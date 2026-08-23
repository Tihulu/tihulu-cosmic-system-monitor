// SPDX-License-Identifier: AGPL-3.0-only

use std::{fs, path::Path};

#[derive(Debug, Clone, Default)]
pub(super) struct PowerSnapshot {
    pub(super) supplies: Vec<String>,
    pub(super) sensors: Vec<String>,
}

pub(super) fn read_power_snapshot() -> PowerSnapshot {
    PowerSnapshot {
        supplies: read_power_supplies(),
        sensors: read_hwmon_power_sensors(),
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
        let power_w = read_micro_value(path.join("power_now"))
            .or_else(|| inferred_power_w(&path));

        let mut parts = Vec::new();
        parts.push(format!("{name} ({kind})"));
        if let Some(online) = online {
            parts.push(if online { "online".into() } else { "offline".into() });
        }
        if let Some(status) = status {
            if !status.eq_ignore_ascii_case("unknown") {
                parts.push(status);
            }
        }
        if let Some(power_w) = power_w {
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

fn read_hwmon_power_sensors() -> Vec<String> {
    let Ok(hwmons) = fs::read_dir("/sys/class/hwmon") else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    for hwmon in hwmons.flatten() {
        let path = hwmon.path();
        let chip = read_text(path.join("name")).unwrap_or_else(|| hwmon.file_name().to_string_lossy().into());
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };

        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if !filename.starts_with("power") || !filename.ends_with("_input") {
                continue;
            }

            let stem = filename.trim_end_matches("_input");
            let Some(power_w) = read_micro_value(entry.path()) else {
                continue;
            };
            let label = read_text(path.join(format!("{stem}_label")))
                .unwrap_or_else(|| stem.to_string());
            lines.push(format!("{chip} / {label}: {power_w:.1} W"));
        }
    }

    lines.sort();
    lines.truncate(12);
    lines
}

fn inferred_power_w(path: &Path) -> Option<f64> {
    let voltage_v = read_micro_value(path.join("voltage_now"))?;
    let current_a = read_micro_value(path.join("current_now"))?;
    Some(voltage_v * current_a)
}

fn read_micro_value(path: impl AsRef<Path>) -> Option<f64> {
    let raw = fs::read_to_string(path).ok()?.trim().parse::<f64>().ok()?;
    Some(raw / 1_000_000.0)
}

fn read_text(path: impl AsRef<Path>) -> Option<String> {
    let value = fs::read_to_string(path).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}
