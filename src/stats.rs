// SPDX-License-Identifier: AGPL-3.0-only

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

#[derive(Debug, Clone, Copy, Default)]
struct CpuSample {
    idle: u64,
    total: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct GpuSample {
    usage_percent: Option<f64>,
    temperature_c: Option<f64>,
    vram_used_bytes: Option<u64>,
    vram_total_bytes: Option<u64>,
}

#[derive(Debug, Default)]
pub(crate) struct SystemStats {
    previous_cpu: Option<CpuSample>,
    cpu_usage_percent: Option<f64>,
    cpu_temperature_c: Option<f64>,
    gpu_usage_percent: Option<f64>,
    gpu_temperature_c: Option<f64>,
    ram_used_bytes: Option<u64>,
    ram_total_bytes: Option<u64>,
    vram_used_bytes: Option<u64>,
    vram_total_bytes: Option<u64>,
}

impl SystemStats {
    pub(crate) fn refresh(&mut self) {
        if let Some(current) = read_cpu_sample() {
            self.cpu_usage_percent = self
                .previous_cpu
                .and_then(|previous| cpu_usage_between(previous, current));
            self.previous_cpu = Some(current);
        }

        self.cpu_temperature_c = read_cpu_temperature();

        if let Some((used, total)) = read_memory() {
            self.ram_used_bytes = Some(used);
            self.ram_total_bytes = Some(total);
        }

        let gpu = query_nvidia_smi().unwrap_or_else(query_drm_gpu);
        self.gpu_usage_percent = gpu.usage_percent;
        self.gpu_temperature_c = gpu.temperature_c;
        self.vram_used_bytes = gpu.vram_used_bytes;
        self.vram_total_bytes = gpu.vram_total_bytes;
    }

    pub(crate) fn cpu_panel_text(&self) -> String {
        format!(
            "CPU {} {}",
            format_percent(self.cpu_usage_percent),
            format_temperature(self.cpu_temperature_c)
        )
    }

    pub(crate) fn gpu_panel_text(&self) -> String {
        format!(
            "GPU {} {}",
            format_percent(self.gpu_usage_percent),
            format_temperature(self.gpu_temperature_c)
        )
    }

    pub(crate) fn ram_panel_text(&self) -> String {
        format_memory("RAM", self.ram_used_bytes, self.ram_total_bytes)
    }

    pub(crate) fn vram_panel_text(&self) -> String {
        format_memory("VRAM", self.vram_used_bytes, self.vram_total_bytes)
    }
}

fn read_cpu_sample() -> Option<CpuSample> {
    let contents = fs::read_to_string("/proc/stat").ok()?;
    parse_cpu_sample(&contents)
}

fn parse_cpu_sample(contents: &str) -> Option<CpuSample> {
    let line = contents.lines().find(|line| line.starts_with("cpu "))?;
    let values = line
        .split_whitespace()
        .skip(1)
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;

    if values.len() < 4 {
        return None;
    }

    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    // guest and guest_nice are already included in user/nice in /proc/stat.
    let total = values.iter().take(8).copied().sum();
    Some(CpuSample { idle, total })
}

fn cpu_usage_between(previous: CpuSample, current: CpuSample) -> Option<f64> {
    let total_delta = current.total.checked_sub(previous.total)?;
    let idle_delta = current.idle.checked_sub(previous.idle)?;

    if total_delta == 0 {
        return None;
    }

    let busy = total_delta.saturating_sub(idle_delta);
    Some((busy as f64 * 100.0 / total_delta as f64).clamp(0.0, 100.0))
}

fn read_memory() -> Option<(u64, u64)> {
    let contents = fs::read_to_string("/proc/meminfo").ok()?;
    parse_memory(&contents)
}

fn parse_memory(contents: &str) -> Option<(u64, u64)> {
    let mut total_kib = None;
    let mut available_kib = None;

    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("MemTotal:") {
            total_kib = value.split_whitespace().next()?.parse::<u64>().ok();
        } else if let Some(value) = line.strip_prefix("MemAvailable:") {
            available_kib = value.split_whitespace().next()?.parse::<u64>().ok();
        }
    }

    let total = total_kib?.saturating_mul(KIB);
    let available = available_kib?.saturating_mul(KIB);
    Some((total.saturating_sub(available), total))
}

fn read_cpu_temperature() -> Option<f64> {
    let mut preferred = Vec::new();
    let mut labelled = Vec::new();

    if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = fs::read_to_string(path.join("name"))
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();

            let cpu_hwmon = matches!(
                name.as_str(),
                "coretemp" | "k10temp" | "zenpower" | "cpu_thermal" | "x86_pkg_temp"
            );

            for sensor in hwmon_temperature_inputs(&path) {
                let Some(value) = read_temperature_file(&sensor) else {
                    continue;
                };
                let Some(file_name) = sensor.file_name() else {
                    continue;
                };
                let label_path = sensor.with_file_name(
                    file_name.to_string_lossy().replace("_input", "_label"),
                );
                let label = fs::read_to_string(label_path)
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase();

                if cpu_hwmon {
                    preferred.push(value);
                } else if ["package", "tctl", "tdie", "cpu"]
                    .iter()
                    .any(|needle| label.contains(needle))
                {
                    labelled.push(value);
                }
            }
        }
    }

    preferred
        .into_iter()
        .chain(labelled)
        .filter(|temperature| (0.0..=125.0).contains(temperature))
        .max_by(f64::total_cmp)
        .or_else(read_thermal_zone_cpu_temperature)
}

fn hwmon_temperature_inputs(path: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };

    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| {
                    let name = name.to_string_lossy();
                    name.starts_with("temp") && name.ends_with("_input")
                })
                .unwrap_or(false)
        })
        .collect()
}

fn read_thermal_zone_cpu_temperature() -> Option<f64> {
    let entries = fs::read_dir("/sys/class/thermal").ok()?;
    entries.flatten().find_map(|entry| {
        let path = entry.path();
        let zone_type = fs::read_to_string(path.join("type"))
            .ok()?
            .trim()
            .to_ascii_lowercase();

        if ["x86_pkg_temp", "cpu", "cpu-thermal", "soc_thermal"]
            .iter()
            .any(|needle| zone_type.contains(needle))
        {
            read_temperature_file(&path.join("temp"))
        } else {
            None
        }
    })
}

fn read_temperature_file(path: &Path) -> Option<f64> {
    let raw = fs::read_to_string(path).ok()?.trim().parse::<f64>().ok()?;
    let celsius = if raw.abs() > 1000.0 { raw / 1000.0 } else { raw };
    (celsius.is_finite() && (-20.0..=150.0).contains(&celsius)).then_some(celsius)
}

fn query_nvidia_smi() -> Option<GpuSample> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=utilization.gpu,temperature.gpu,memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_nvidia_smi(&String::from_utf8_lossy(&output.stdout))
}

fn parse_nvidia_smi(contents: &str) -> Option<GpuSample> {
    let line = contents.lines().find(|line| !line.trim().is_empty())?;
    let fields: Vec<&str> = line.split(',').map(str::trim).collect();
    if fields.len() < 4 {
        return None;
    }

    let parse_float = |value: &str| value.parse::<f64>().ok();
    let parse_mib = |value: &str| {
        value
            .parse::<f64>()
            .ok()
            .filter(|value| *value >= 0.0)
            .map(|value| (value * MIB as f64).round() as u64)
    };

    let sample = GpuSample {
        usage_percent: parse_float(fields[0]),
        temperature_c: parse_float(fields[1]),
        vram_used_bytes: parse_mib(fields[2]),
        vram_total_bytes: parse_mib(fields[3]),
    };

    (sample.usage_percent.is_some()
        || sample.temperature_c.is_some()
        || sample.vram_used_bytes.is_some()
        || sample.vram_total_bytes.is_some())
        .then_some(sample)
}

fn query_drm_gpu() -> GpuSample {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return GpuSample::default();
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("card") {
            continue;
        }

        let device = entry.path().join("device");
        if !device.exists() {
            continue;
        }

        let usage_percent = read_number(device.join("gpu_busy_percent"));
        let vram_used_bytes = read_u64(device.join("mem_info_vram_used"));
        let vram_total_bytes = read_u64(device.join("mem_info_vram_total"));
        let temperature_c = read_drm_gpu_temperature(&device);

        if usage_percent.is_some()
            || temperature_c.is_some()
            || vram_used_bytes.is_some()
            || vram_total_bytes.is_some()
        {
            return GpuSample {
                usage_percent,
                temperature_c,
                vram_used_bytes,
                vram_total_bytes,
            };
        }
    }

    GpuSample::default()
}

fn read_drm_gpu_temperature(device: &Path) -> Option<f64> {
    let hwmon_root = device.join("hwmon");
    let entries = fs::read_dir(hwmon_root).ok()?;

    entries.flatten().find_map(|entry| {
        hwmon_temperature_inputs(&entry.path())
            .into_iter()
            .filter_map(|path| read_temperature_file(&path))
            .max_by(f64::total_cmp)
    })
}

fn read_number(path: PathBuf) -> Option<f64> {
    fs::read_to_string(path).ok()?.trim().parse::<f64>().ok()
}

fn read_u64(path: PathBuf) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse::<u64>().ok()
}

fn format_percent(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{:>3.0}%", value.clamp(0.0, 100.0)))
        .unwrap_or_else(|| " --%".to_string())
}

fn format_temperature(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.0}°C"))
        .unwrap_or_else(|| "--°C".to_string())
}

fn format_memory(label: &str, used: Option<u64>, total: Option<u64>) -> String {
    match (used, total) {
        (Some(used), Some(total)) if total > 0 => {
            format!("{label} {:.1}/{:.1}G", used as f64 / GIB, total as f64 / GIB)
        }
        _ => format!("{label} --/--G"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cpu_sample_and_usage() {
        let first = parse_cpu_sample("cpu  100 20 30 850 10 0 0 0 0 0\n").unwrap();
        let second = parse_cpu_sample("cpu  150 20 50 880 10 0 0 0 0 0\n").unwrap();
        let usage = cpu_usage_between(first, second).unwrap();
        assert!((usage - 70.0).abs() < 0.001);
    }

    #[test]
    fn parses_meminfo() {
        let (used, total) = parse_memory(
            "MemTotal:       1000000 kB\nMemFree: 1000 kB\nMemAvailable:    250000 kB\n",
        )
        .unwrap();
        assert_eq!(total, 1_000_000 * KIB);
        assert_eq!(used, 750_000 * KIB);
    }

    #[test]
    fn parses_nvidia_smi_line() {
        let sample = parse_nvidia_smi("42, 57, 2048, 16384\n").unwrap();
        assert_eq!(sample.usage_percent, Some(42.0));
        assert_eq!(sample.temperature_c, Some(57.0));
        assert_eq!(sample.vram_used_bytes, Some(2048 * MIB));
        assert_eq!(sample.vram_total_bytes, Some(16384 * MIB));
    }
}
