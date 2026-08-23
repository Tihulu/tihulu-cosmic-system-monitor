// SPDX-License-Identifier: AGPL-3.0-only

use std::{env, fs, path::{Path, PathBuf}};

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
        if let Some(power_w) = power_w.filter(|value| value.is_finite() && *value > 0.5) {
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
        if !is_smart_psu_chip(&chip) {
            continue;
        }

        lines.push(format!("Digital PSU interface: {chip}"));

        for index in 1..=8 {
            if let Some(power_w) = read_micro_value(path.join(format!("power{index}_input")))
                .filter(|value| value.is_finite() && *value > 0.5)
            {
                let label = read_text(path.join(format!("power{index}_label")))
                    .unwrap_or_else(|| if index == 1 { "Total power".into() } else { format!("Power {index}") });
                lines.push(format!("{label}: {power_w:.1} W"));
            }
        }

        for index in 0..=8 {
            if let Some(voltage_v) = read_milli_value(path.join(format!("in{index}_input")))
                .filter(|value| value.is_finite() && *value > 0.0)
            {
                let label = read_text(path.join(format!("in{index}_label")))
                    .unwrap_or_else(|| if index == 0 { "AC input".into() } else { format!("Voltage {index}") });
                lines.push(format!("{label}: {voltage_v:.2} V"));
            }
        }

        for index in 1..=8 {
            if let Some(current_a) = read_milli_value(path.join(format!("curr{index}_input")))
                .filter(|value| value.is_finite() && *value > 0.0)
            {
                let label = read_text(path.join(format!("curr{index}_label")))
                    .unwrap_or_else(|| if index == 1 { "Total current".into() } else { format!("Current {index}") });
                lines.push(format!("{label}: {current_a:.2} A"));
            }
        }

        for index in 1..=4 {
            if let Some(rpm) = read_raw_value(path.join(format!("fan{index}_input")))
                .filter(|value| value.is_finite() && *value > 0.0)
            {
                lines.push(format!("PSU fan {index}: {rpm:.0} RPM"));
            }
        }

        for index in 1..=8 {
            if let Some(temp_c) = read_milli_value(path.join(format!("temp{index}_input")))
                .filter(|value| value.is_finite() && *value > -50.0 && *value < 200.0)
            {
                let label = read_text(path.join(format!("temp{index}_label")))
                    .unwrap_or_else(|| format!("Temperature {index}"));
                lines.push(format!("{label}: {temp_c:.1}°C"));
            }
        }
    }

    lines.sort();
    lines.truncate(24);
    lines
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
        if is_smart_psu_chip(&chip) {
            continue;
        }

        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };

        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if !filename.starts_with("power") || !filename.ends_with("_input") {
                continue;
            }

            let stem = filename.trim_end_matches("_input");
            let Some(power_w) = read_micro_value(entry.path())
                .filter(|value| value.is_finite() && *value > 0.5)
            else {
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

fn read_configured_psu_name() -> Option<String> {
    let path = config_path();
    let contents = fs::read_to_string(path).ok()?;
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

fn is_smart_psu_chip(chip: &str) -> bool {
    let chip = chip.to_ascii_lowercase().replace('-', "_");
    chip.contains("corsair_psu")
        || chip.contains("pmbus")
        || chip.contains("crps")
        || chip.contains("dps920")
        || chip.contains("cffps")
        || chip.contains("fsp3y")
        || chip.contains("psu")
}

fn inferred_power_w(path: &Path) -> Option<f64> {
    let voltage_v = read_micro_value(path.join("voltage_now"))?;
    let current_a = read_micro_value(path.join("current_now"))?;
    Some(voltage_v * current_a)
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
    fn recognizes_smart_psu_hwmon_names() {
        assert!(is_smart_psu_chip("corsair_psu"));
        assert!(is_smart_psu_chip("pmbus"));
        assert!(is_smart_psu_chip("dps920ab"));
        assert!(!is_smart_psu_chip("amdgpu"));
    }
}
