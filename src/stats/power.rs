// SPDX-License-Identifier: AGPL-3.0-only

use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default)]
pub(super) struct PowerSnapshot {
    pub(super) supplies: Vec<String>,
    pub(super) sensors: Vec<String>,
}

pub(super) fn read_power_snapshot() -> PowerSnapshot {
    let mut supplies = read_power_supplies();
    supplies.extend(read_smart_psu_hwmon());
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

fn append_psu_sensor_lines(path: &Path, lines: &mut Vec<String>) {
    for index in 1..=16 {
        let input = path.join(format!("power{index}_input"));
        let average = path.join(format!("power{index}_average"));
        let power_w = valid_positive(
            read_micro_value(&input).or_else(|| read_micro_value(&average)),
            0.5,
        );
        if let Some(power_w) = power_w {
            let label = read_text(path.join(format!("power{index}_label")))
                .unwrap_or_else(|| if index == 1 { "Total/input power".into() } else { format!("Power {index}") });
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
            let label = read_text(path.join(format!("in{index}_label")))
                .unwrap_or_else(|| if index == 0 { "AC/input voltage".into() } else { format!("Voltage {index}") });
            lines.push(format!("{label}: {voltage_v:.2} V"));
        }
    }

    for index in 1..=16 {
        if let Some(current_a) = valid_positive(
            read_milli_value(path.join(format!("curr{index}_input"))),
            0.001,
        ) {
            let label = read_text(path.join(format!("curr{index}_label")))
                .unwrap_or_else(|| if index == 1 { "Total/input current".into() } else { format!("Current {index}") });
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
        "psu", "power supply", "pin", "pout", "vin", "vout", "iin", "iout",
        "ac input", "input power", "output power", "total power", "12v", "5v", "3.3v",
    ]
    .iter()
    .any(|needle| joined_labels.contains(needle));

    let has_power = has_sensor(path, "power", &["_input", "_average"]);
    let has_voltage = has_sensor(path, "in", &["_input"]);
    let has_current = has_sensor(path, "curr", &["_input"]);
    let has_fan = has_sensor(path, "fan", &["_input"]);
    let has_temp = has_sensor(path, "temp", &["_input"]);

    has_power
        && psu_label_hint
        && has_voltage
        && has_current
        && (has_fan || has_temp)
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
    value
        .to_ascii_lowercase()
        .replace(['-', ' ', '/'], "_")
}

fn collect_sensor_labels(path: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };

    let mut labels = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.ends_with("_label").then(|| read_text(entry.path())).flatten()
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
}
