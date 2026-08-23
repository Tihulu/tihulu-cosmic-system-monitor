// SPDX-License-Identifier: AGPL-3.0-only

use std::{collections::BTreeSet, fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct CpuSample { pub(super) idle: u64, pub(super) total: u64 }

pub(super) fn read_cpu_samples() -> Option<Vec<CpuSample>> {
    parse_cpu_samples(&fs::read_to_string("/proc/stat").ok()?)
}

fn parse_cpu_samples(contents: &str) -> Option<Vec<CpuSample>> {
    let samples: Vec<_> = contents.lines().take_while(|l| l.starts_with("cpu")).filter_map(parse_cpu_line).collect();
    (!samples.is_empty()).then_some(samples)
}

fn parse_cpu_line(line: &str) -> Option<CpuSample> {
    let mut fields = line.split_whitespace();
    if !fields.next()?.starts_with("cpu") { return None; }
    let values = fields.map(str::parse::<u64>).collect::<Result<Vec<_>, _>>().ok()?;
    if values.len() < 4 { return None; }
    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    let total = values.iter().take(8).copied().sum();
    Some(CpuSample { idle, total })
}

pub(super) fn cpu_usage_between(previous: CpuSample, current: CpuSample) -> Option<f64> {
    let total_delta = current.total.checked_sub(previous.total)?;
    let idle_delta = current.idle.checked_sub(previous.idle)?;
    if total_delta == 0 { return None; }
    Some((total_delta.saturating_sub(idle_delta) as f64 * 100.0 / total_delta as f64).clamp(0.0, 100.0))
}

pub(super) fn read_cpu_info() -> Option<(String, usize, Option<usize>)> {
    let contents = fs::read_to_string("/proc/cpuinfo").ok()?;
    let mut model = None;
    let mut logical = 0usize;
    let mut physical_pairs = BTreeSet::new();
    for block in contents.split("\n\n") {
        let (mut package, mut core) = (None, None);
        for line in block.lines() {
            let Some((key, value)) = line.split_once(':') else { continue };
            match key.trim() {
                "processor" => logical += 1,
                "model name" | "Hardware" if model.is_none() => model = Some(value.trim().to_string()),
                "physical id" => package = value.trim().parse::<u32>().ok(),
                "core id" => core = value.trim().parse::<u32>().ok(),
                _ => {}
            }
        }
        if let (Some(p), Some(c)) = (package, core) { physical_pairs.insert((p, c)); }
    }
    Some((model.unwrap_or_else(|| "Linux CPU".into()), logical, (!physical_pairs.is_empty()).then_some(physical_pairs.len())))
}

pub(super) fn read_cpu_frequency_mhz() -> Option<f64> {
    let contents = fs::read_to_string("/proc/cpuinfo").ok()?;
    let values: Vec<f64> = contents.lines().filter_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "cpu MHz").then(|| value.trim().parse::<f64>().ok()).flatten()
    }).collect();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

pub(super) fn read_load_average() -> Option<(f64, f64, f64)> {
    let contents = fs::read_to_string("/proc/loadavg").ok()?;
    let mut f = contents.split_whitespace();
    Some((f.next()?.parse().ok()?, f.next()?.parse().ok()?, f.next()?.parse().ok()?))
}

pub(super) fn read_uptime_seconds() -> Option<f64> {
    fs::read_to_string("/proc/uptime").ok()?.split_whitespace().next()?.parse().ok()
}

pub(super) fn read_cpu_temperature() -> Option<f64> {
    let mut preferred = Vec::new();
    let mut labelled = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = fs::read_to_string(path.join("name")).unwrap_or_default().trim().to_ascii_lowercase();
            let cpu_hwmon = matches!(name.as_str(), "coretemp" | "k10temp" | "zenpower" | "cpu_thermal" | "x86_pkg_temp");
            for sensor in hwmon_temperature_inputs(&path) {
                let Some(value) = read_temperature_file(&sensor) else { continue };
                let Some(file_name) = sensor.file_name() else { continue };
                let label_path = sensor.with_file_name(file_name.to_string_lossy().replace("_input", "_label"));
                let label = fs::read_to_string(label_path).unwrap_or_default().trim().to_ascii_lowercase();
                if cpu_hwmon { preferred.push(value); }
                else if ["package", "tctl", "tdie", "cpu"].iter().any(|n| label.contains(n)) { labelled.push(value); }
            }
        }
    }
    preferred.into_iter().chain(labelled).filter(|t| (0.0..=125.0).contains(t)).max_by(f64::total_cmp).or_else(read_thermal_zone_cpu_temperature)
}

fn hwmon_temperature_inputs(path: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(path) else { return Vec::new() };
    entries.flatten().map(|e| e.path()).filter(|p| p.file_name().map(|n| { let n=n.to_string_lossy(); n.starts_with("temp") && n.ends_with("_input") }).unwrap_or(false)).collect()
}

fn read_thermal_zone_cpu_temperature() -> Option<f64> {
    fs::read_dir("/sys/class/thermal").ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        let zone_type = fs::read_to_string(path.join("type")).ok()?.trim().to_ascii_lowercase();
        ["x86_pkg_temp", "cpu", "cpu-thermal", "soc_thermal"].iter().any(|n| zone_type.contains(n)).then(|| read_temperature_file(&path.join("temp"))).flatten()
    })
}

pub(super) fn read_temperature_file(path: &Path) -> Option<f64> {
    let raw = fs::read_to_string(path).ok()?.trim().parse::<f64>().ok()?;
    let c = if raw.abs() > 1000.0 { raw / 1000.0 } else { raw };
    (c.is_finite() && (-20.0..=150.0).contains(&c)).then_some(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_delta() {
        let a = parse_cpu_samples("cpu  100 20 30 850 10 0 0 0\ncpu0 50 10 15 425 5 0 0 0\n").unwrap();
        let b = parse_cpu_samples("cpu  150 20 50 880 10 0 0 0\ncpu0 70 10 25 440 5 0 0 0\n").unwrap();
        assert!((cpu_usage_between(a[0], b[0]).unwrap() - 70.0).abs() < 0.001);
        assert_eq!(a.len(), 2);
    }
}
