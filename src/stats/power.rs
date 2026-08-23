// SPDX-License-Identifier: AGPL-3.0-only

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Default)]
pub(super) struct PowerSnapshot {
    pub(super) supplies: Vec<String>,
    pub(super) sensors: Vec<String>,
}

pub(super) fn read_power_snapshot() -> PowerSnapshot {
    let mut supplies = read_power_supplies();
    supplies.extend(read_smart_psu_hwmon());
    supplies.extend(read_dmi_psu_info());
    if let Some(name) = read_configured_psu_name() {
        supplies.push(format!("Configured PSU: {name}"));
    }
    supplies.sort();
    supplies.dedup();

    PowerSnapshot {
        supplies,
        sensors: read_component_power_sensors(),
    }
}

fn read_power_supplies() -> Vec<String> {
    let Ok(entries) = fs::read_dir("/sys/class/power_supply") else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let kind = read_text(path.join("type")).unwrap_or_else(|| "Unknown".into());
        let online = read_text(path.join("online")).map(|value| value == "1");
        let status = read_text(path.join("status"));
        let manufacturer = read_text(path.join("manufacturer"));
        let model = read_text(path.join("model_name"));
        let power_w = read_micro_value(path.join("power_now")).or_else(|| inferred_power_w(&path));

        let mut parts = vec![format!("{name} ({kind})")];
        if let Some(online) = online {
            parts.push(if online { "online".into() } else { "offline".into() });
        }
        if let Some(status) = status {
            if !status.eq_ignore_ascii_case("unknown") {
                parts.push(status);
            }
        }
        if let Some(power_w) = valid_positive(power_w, 0.5) {
            parts.push(format!("{power_w:.1} W"));
        }
        if manufacturer.is_some() || model.is_some() {
            parts.push(
                [manufacturer, model]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }

        lines.push(parts.join(" · "));
    }

    lines.sort();
    lines
}

fn read_smart_psu_hwmon() -> Vec<String> {
    let Ok(hwmons) = fs::read_dir("/sys/class/hwmon") else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    for hwmon in hwmons.flatten() {
        let path = hwmon.path();
        let chip = read_text(path.join("name"))
            .unwrap_or_else(|| hwmon.file_name().to_string_lossy().into());
        let driver = hwmon_driver_name(&path);
        let labels = collect_sensor_labels(&path);

        if !is_psu_hwmon(&path, &chip, driver.as_deref(), &labels) {
            continue;
        }

        let identity = hwmon_identity(&path, &chip, driver.as_deref());
        lines.push(format!("PSU interface: {identity}"));
        append_psu_sensor_lines(&path, &mut lines);
    }

    lines.sort();
    lines.dedup();
    lines.truncate(40);
    lines
}

fn read_dmi_psu_info() -> Vec<String> {
    let mut lines = read_dmi_type39_sysfs();
    if lines.is_empty() {
        lines = read_dmidecode_type39();
    }
    lines.sort();
    lines.dedup();
    lines
}

fn read_dmi_type39_sysfs() -> Vec<String> {
    let Ok(entries) = fs::read_dir("/sys/firmware/dmi/entries") else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("39-") {
            continue;
        }
        let Ok(raw) = fs::read(entry.path().join("raw")) else {
            continue;
        };
        if let Some(line) = parse_smbios_type39(&raw) {
            lines.push(line);
        }
    }
    lines
}

fn parse_smbios_type39(raw: &[u8]) -> Option<String> {
    if raw.len() < 16 || raw[0] != 39 {
        return None;
    }
    let formatted_len = raw[1] as usize;
    if formatted_len < 16 || formatted_len > raw.len() {
        return None;
    }

    let location = smbios_string(raw, formatted_len, raw[5]);
    let device_name = smbios_string(raw, formatted_len, raw[6]);
    let manufacturer = smbios_string(raw, formatted_len, raw[7]);
    let model = smbios_string(raw, formatted_len, raw[10]);
    let max_power = u16::from_le_bytes([raw[12], raw[13]]);

    let mut identity = [manufacturer, model, device_name, location]
        .into_iter()
        .flatten()
        .filter(|value| !is_placeholder(value))
        .collect::<Vec<_>>();
    identity.dedup();

    if identity.is_empty() && (max_power == 0 || max_power == u16::MAX) {
        return None;
    }

    let mut line = if identity.is_empty() {
        "SMBIOS System Power Supply".to_string()
    } else {
        format!("SMBIOS PSU: {}", identity.join(" · "))
    };
    if max_power != 0 && max_power != u16::MAX {
        line.push_str(&format!(" · {max_power} W max"));
    }
    Some(line)
}

fn smbios_string(raw: &[u8], formatted_len: usize, index: u8) -> Option<String> {
    if index == 0 || formatted_len >= raw.len() {
        return None;
    }

    let strings = &raw[formatted_len..];
    let mut current = 1u8;
    let mut start = 0usize;
    for end in 0..=strings.len() {
        if end == strings.len() || strings[end] == 0 {
            if current == index {
                let value = String::from_utf8_lossy(&strings[start..end]).trim().to_string();
                return (!value.is_empty()).then_some(value);
            }
            if end == strings.len() || (end + 1 < strings.len() && strings[end + 1] == 0) {
                break;
            }
            current = current.saturating_add(1);
            start = end + 1;
        }
    }
    None
}

fn read_dmidecode_type39() -> Vec<String> {
    let Ok(output) = Command::new("dmidecode").args(["--type", "39"]).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_dmidecode_type39(&String::from_utf8_lossy(&output.stdout))
}

fn parse_dmidecode_type39(contents: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut manufacturer: Option<String> = None;
    let mut model: Option<String> = None;
    let mut name: Option<String> = None;
    let mut location: Option<String> = None;
    let mut max_power: Option<String> = None;

    let flush = |lines: &mut Vec<String>,
                 manufacturer: &mut Option<String>,
                 model: &mut Option<String>,
                 name: &mut Option<String>,
                 location: &mut Option<String>,
                 max_power: &mut Option<String>| {
        let mut parts = [manufacturer.take(), model.take(), name.take(), location.take()]
            .into_iter()
            .flatten()
            .filter(|value| !is_placeholder(value))
            .collect::<Vec<_>>();
        parts.dedup();
        if !parts.is_empty() || max_power.is_some() {
            let mut line = if parts.is_empty() {
                "SMBIOS System Power Supply".to_string()
            } else {
                format!("SMBIOS PSU: {}", parts.join(" · "))
            };
            if let Some(power) = max_power.take() {
                if !is_placeholder(&power) {
                    line.push_str(&format!(" · {power}"));
                }
            }
            lines.push(line);
        }
    };

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("System Power Supply") || trimmed.starts_with("Handle ") {
            flush(
                &mut lines,
                &mut manufacturer,
                &mut model,
                &mut name,
                &mut location,
                &mut max_power,
            );
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim().to_string();
        match key.trim() {
            "Manufacturer" => manufacturer = Some(value),
            "Model Part Number" => model = Some(value),
            "Name" | "Device Name" => name = Some(value),
            "Location" => location = Some(value),
            "Max Power Capacity" => max_power = Some(value),
            _ => {}
        }
    }
    flush(
        &mut lines,
        &mut manufacturer,
        &mut model,
        &mut name,
        &mut location,
        &mut max_power,
    );
    lines
}

fn is_placeholder(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "unknown" | "not specified" | "none" | "n/a" | "to be filled by o.e.m."
    )
}

fn append_psu_sensor_lines(path: &Path, lines: &mut Vec<String>) {
    for index in 1..=16 {
        let input = path.join(format!("power{index}_input"));
        let average = path.join(format!("power{index}_average"));
        let power_w = valid_positive(
            read_micro_value(&input).or_else(|| read_micro_value(&average)),
            0.5,
        );
        if let Some(power_w) = power_w {
            let label = read_text(path.join(format!("power{index}_label"))).unwrap_or_else(|| {
                if index == 1 {
                    "Total/input power".into()
                } else {
                    format!("Power {index}")
                }
            });
            lines.push(format!("{label}: {power_w:.1} W"));
        }

        if let Some(max_w) = valid_positive(
            read_micro_value(path.join(format!("power{index}_rated_max")))
                .or_else(|| read_micro_value(path.join(format!("power{index}_max")))),
            1.0,
        ) {
            let label = read_text(path.join(format!("power{index}_label")))
                .unwrap_or_else(|| format!("Power {index}"));
            lines.push(format!("{label} rated/max: {max_w:.0} W"));
        }
    }

    for index in 0..=16 {
        if let Some(voltage_v) = valid_positive(
            read_milli_value(path.join(format!("in{index}_input"))),
            0.01,
        ) {
            let label = read_text(path.join(format!("in{index}_label"))).unwrap_or_else(|| {
                if index == 0 {
                    "AC/input voltage".into()
                } else {
                    format!("Voltage {index}")
                }
            });
            lines.push(format!("{label}: {voltage_v:.2} V"));
        }
    }

    for index in 1..=16 {
        if let Some(current_a) = valid_positive(
            read_milli_value(path.join(format!("curr{index}_input"))),
            0.001,
        ) {
            let label = read_text(path.join(format!("curr{index}_label"))).unwrap_or_else(|| {
                if index == 1 {
                    "Total/input current".into()
                } else {
                    format!("Current {index}")
                }
            });
            lines.push(format!("{label}: {current_a:.2} A"));
        }
    }

    for index in 1..=8 {
        if let Some(rpm) = valid_positive(
            read_raw_value(path.join(format!("fan{index}_input"))),
            1.0,
        ) {
            let label = read_text(path.join(format!("fan{index}_label")))
                .unwrap_or_else(|| format!("PSU fan {index}"));
            lines.push(format!("{label}: {rpm:.0} RPM"));
        }
    }

    for index in 1..=16 {
        if let Some(temp_c) = read_milli_value(path.join(format!("temp{index}_input")))
            .filter(|value| value.is_finite() && *value > -50.0 && *value < 200.0)
        {
            let label = read_text(path.join(format!("temp{index}_label")))
                .unwrap_or_else(|| format!("Temperature {index}"));
            lines.push(format!("{label}: {temp_c:.1}°C"));
        }
    }
}

fn read_component_power_sensors() -> Vec<String> {
    let Ok(hwmons) = fs::read_dir("/sys/class/hwmon") else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    for hwmon in hwmons.flatten() {
        let path = hwmon.path();
        let chip = read_text(path.join("name"))
            .unwrap_or_else(|| hwmon.file_name().to_string_lossy().into());
        let driver = hwmon_driver_name(&path);
        let labels = collect_sensor_labels(&path);
        if is_psu_hwmon(&path, &chip, driver.as_deref(), &labels) {
            continue;
        }

        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };

        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if !filename.starts_with("power")
                || !(filename.ends_with("_input") || filename.ends_with("_average"))
            {
                continue;
            }

            let stem = filename
                .trim_end_matches("_input")
                .trim_end_matches("_average");
            let Some(power_w) = valid_positive(read_micro_value(entry.path()), 0.5) else {
                continue;
            };
            let label = read_text(path.join(format!("{stem}_label")))
                .unwrap_or_else(|| stem.to_string());
            lines.push(format!("{chip} / {label}: {power_w:.1} W"));
        }
    }

    lines.sort();
    lines.dedup();
    lines.truncate(12);
    lines
}

fn is_psu_hwmon(path: &Path, chip: &str, driver: Option<&str>, labels: &[String]) -> bool {
    if is_known_psu_identifier(chip) || driver.is_some_and(is_known_psu_identifier) {
        return true;
    }

    let joined_labels = labels.join(" ").to_ascii_lowercase();
    let psu_label_hint = [
        "psu",
        "power supply",
        "pin",
        "pout",
        "vin",
        "vout",
        "iin",
        "iout",
        "ac input",
        "input power",
        "output power",
        "total power",
        "12v",
        "5v",
        "3.3v",
    ]
    .iter()
    .any(|needle| joined_labels.contains(needle));

    let has_power = has_sensor(path, "power", &["_input", "_average"]);
    let has_voltage = has_sensor(path, "in", &["_input"]);
    let has_current = has_sensor(path, "curr", &["_input"]);
    let has_fan = has_sensor(path, "fan", &["_input"]);
    let has_temp = has_sensor(path, "temp", &["_input"]);

    has_power && psu_label_hint && has_voltage && has_current && (has_fan || has_temp)
}

fn is_known_psu_identifier(value: &str) -> bool {
    let value = normalize_identifier(value);
    [
        "corsair_psu",
        "crps185",
        "dps920ab",
        "ibm_cffps",
        "inspur_ipsps1",
        "ipsps1",
        "acbel_fsg032",
        "fsg032",
        "pfe1100",
        "pfe3000",
        "bel_pfe",
        "bpa_rs600",
        "fsp3y",
        "fsp_3y",
        "hac300s",
        "d1u74t",
        "lineage_pem",
        "cffps",
        "common_redundant_power_supply",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

fn normalize_identifier(value: &str) -> String {
    value.to_ascii_lowercase().replace(['-', ' ', '/'], "_")
}

fn collect_sensor_labels(path: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };

    let mut labels = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.ends_with("_label")
                .then(|| read_text(entry.path()))
                .flatten()
        })
        .collect::<Vec<_>>();
    labels.sort();
    labels.dedup();
    labels
}

fn has_sensor(path: &Path, prefix: &str, suffixes: &[&str]) -> bool {
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };

    entries.flatten().any(|entry| {
        let name = entry.file_name().to_string_lossy().to_string();
        name.starts_with(prefix) && suffixes.iter().any(|suffix| name.ends_with(suffix))
    })
}

fn hwmon_driver_name(path: &Path) -> Option<String> {
    let target = fs::read_link(path.join("device/driver")).ok()?;
    target.file_name()?.to_str().map(ToOwned::to_owned)
}

fn hwmon_identity(path: &Path, chip: &str, driver: Option<&str>) -> String {
    let mut values = Vec::new();
    if let Ok(mut current) = fs::canonicalize(path.join("device")) {
        for _ in 0..6 {
            for key in ["manufacturer", "product", "model", "model_name"] {
                if let Some(value) = read_text(current.join(key)) {
                    if !values.contains(&value) {
                        values.push(value);
                    }
                }
            }
            let Some(parent) = current.parent() else {
                break;
            };
            current = parent.to_path_buf();
        }
    }

    if values.is_empty() {
        let driver = driver.filter(|value| !value.eq_ignore_ascii_case(chip));
        match driver {
            Some(driver) => format!("{chip} [{driver}]"),
            None => chip.to_string(),
        }
    } else {
        let mut identity = values.join(" ");
        if let Some(driver) = driver {
            identity.push_str(&format!(" [{driver}]"));
        }
        identity
    }
}

fn read_configured_psu_name() -> Option<String> {
    let contents = fs::read_to_string(config_path()).ok()?;
    contents.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "psu_name" && !value.trim().is_empty()).then(|| value.trim().to_string())
    })
}

fn config_path() -> PathBuf {
    if let Some(base) = env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(base).join("tihulu-cosmic-system-monitor/panel.conf");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".config/tihulu-cosmic-system-monitor/panel.conf");
    }
    PathBuf::from("/tmp/tihulu-cosmic-system-monitor-panel.conf")
}

fn inferred_power_w(path: &Path) -> Option<f64> {
    let voltage_v = read_micro_value(path.join("voltage_now"))?;
    let current_a = read_micro_value(path.join("current_now"))?;
    Some(voltage_v * current_a)
}

fn valid_positive(value: Option<f64>, minimum: f64) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value >= minimum)
}

fn read_micro_value(path: impl AsRef<Path>) -> Option<f64> {
    Some(read_raw_value(path)? / 1_000_000.0)
}

fn read_milli_value(path: impl AsRef<Path>) -> Option<f64> {
    Some(read_raw_value(path)? / 1_000.0)
}

fn read_raw_value(path: impl AsRef<Path>) -> Option<f64> {
    fs::read_to_string(path).ok()?.trim().parse::<f64>().ok()
}

fn read_text(path: impl AsRef<Path>) -> Option<String> {
    let value = fs::read_to_string(path).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_common_linux_psu_drivers() {
        for name in [
            "corsair_psu",
            "crps185",
            "dps920ab",
            "ibm-cffps",
            "inspur-ipsps1",
            "acbel-fsg032",
            "pfe1100",
            "bpa-rs600",
            "fsp3y",
            "hac300s",
        ] {
            assert!(is_known_psu_identifier(name), "{name}");
        }
        assert!(!is_known_psu_identifier("amdgpu"));
        assert!(!is_known_psu_identifier("coretemp"));
    }

    #[test]
    fn normalizes_driver_names() {
        assert_eq!(normalize_identifier("IBM CFFPS"), "ibm_cffps");
        assert_eq!(normalize_identifier("BPA-RS600"), "bpa_rs600");
    }

    #[test]
    fn parses_smbios_type39_identity() {
        let mut raw = vec![39, 0x16, 0, 0, 0, 1, 2, 3, 0, 0, 4, 0];
        raw.extend_from_slice(&1000u16.to_le_bytes());
        raw.resize(0x16, 0);
        raw.extend_from_slice(b"Rear PSU\0System PSU\0be quiet!\0Straight Power 12 1000W\0\0");
        let line = parse_smbios_type39(&raw).unwrap();
        assert!(line.contains("be quiet!"));
        assert!(line.contains("Straight Power 12 1000W"));
        assert!(line.contains("1000 W max"));
    }

    #[test]
    fn parses_dmidecode_type39_identity() {
        let text = "System Power Supply\n\tLocation: PSU Bay\n\tName: Main PSU\n\tManufacturer: be quiet!\n\tModel Part Number: Dark Power 13 1000W\n\tMax Power Capacity: 1000 W\n";
        let lines = parse_dmidecode_type39(text);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("be quiet!"));
        assert!(lines[0].contains("Dark Power 13 1000W"));
    }
}
