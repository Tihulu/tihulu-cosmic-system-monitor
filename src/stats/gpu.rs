// SPDX-License-Identifier: AGPL-3.0-only

use std::{fs, path::{Path, PathBuf}, process::Command};
use super::{MIB, cpu::read_temperature_file};

#[derive(Debug, Clone, Default)]
pub(super) struct GpuSample {
    pub(super) usage_percent: Option<f64>,
    pub(super) temperature_c: Option<f64>,
    pub(super) vram_used_bytes: Option<u64>,
    pub(super) vram_total_bytes: Option<u64>,
    pub(super) name: Option<String>,
    pub(super) driver_version: Option<String>,
    pub(super) power_draw_w: Option<f64>,
    pub(super) power_limit_w: Option<f64>,
    pub(super) graphics_clock_mhz: Option<f64>,
    pub(super) memory_clock_mhz: Option<f64>,
}

pub(super) fn query_nvidia_smi() -> Option<GpuSample> {
    let output = Command::new("nvidia-smi").args([
        "--query-gpu=name,driver_version,utilization.gpu,temperature.gpu,memory.used,memory.total,power.draw,power.limit,clocks.current.graphics,clocks.current.memory",
        "--format=csv,noheader,nounits",
    ]).output().ok()?;
    output.status.success().then(|| parse_nvidia_smi(&String::from_utf8_lossy(&output.stdout))).flatten()
}

fn parse_nvidia_smi(contents: &str) -> Option<GpuSample> {
    let fields: Vec<_> = contents.lines().find(|l| !l.trim().is_empty())?.split(',').map(str::trim).collect();
    if fields.len() < 10 { return None; }
    let float = |s: &str| s.parse::<f64>().ok();
    let mib = |s: &str| s.parse::<f64>().ok().filter(|v| *v >= 0.0).map(|v| (v * MIB as f64).round() as u64);
    let text = |s: &str| (!s.is_empty() && s != "[N/A]" && s != "N/A").then(|| s.to_string());
    let sample = GpuSample {
        name: text(fields[0]), driver_version: text(fields[1]),
        usage_percent: float(fields[2]), temperature_c: float(fields[3]),
        vram_used_bytes: mib(fields[4]), vram_total_bytes: mib(fields[5]),
        power_draw_w: float(fields[6]), power_limit_w: float(fields[7]),
        graphics_clock_mhz: float(fields[8]), memory_clock_mhz: float(fields[9]),
    };
    (sample.name.is_some() || sample.usage_percent.is_some() || sample.temperature_c.is_some() || sample.vram_total_bytes.is_some()).then_some(sample)
}

pub(super) fn query_drm_gpu() -> GpuSample {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else { return GpuSample::default() };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("card") { continue; }
        let device = entry.path().join("device");
        if !device.exists() { continue; }
        let usage = read_number(device.join("gpu_busy_percent"));
        let used = read_u64(device.join("mem_info_vram_used"));
        let total = read_u64(device.join("mem_info_vram_total"));
        let temp = read_drm_gpu_temperature(&device);
        if usage.is_some() || used.is_some() || total.is_some() || temp.is_some() {
            return GpuSample { usage_percent: usage, temperature_c: temp, vram_used_bytes: used, vram_total_bytes: total, name: Some("DRM GPU".into()), ..GpuSample::default() };
        }
    }
    GpuSample::default()
}

fn read_drm_gpu_temperature(device: &Path) -> Option<f64> {
    fs::read_dir(device.join("hwmon")).ok()?.flatten().find_map(|entry| {
        hwmon_temperature_inputs(&entry.path()).into_iter().filter_map(|p| read_temperature_file(&p)).max_by(f64::total_cmp)
    })
}

fn hwmon_temperature_inputs(path: &Path) -> Vec<PathBuf> {
    let Ok(entries)=fs::read_dir(path) else { return Vec::new() };
    entries.flatten().map(|e| e.path()).filter(|p| p.file_name().map(|n| { let n=n.to_string_lossy(); n.starts_with("temp") && n.ends_with("_input") }).unwrap_or(false)).collect()
}
fn read_number(path: PathBuf) -> Option<f64> { fs::read_to_string(path).ok()?.trim().parse().ok() }
fn read_u64(path: PathBuf) -> Option<u64> { fs::read_to_string(path).ok()?.trim().parse().ok() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nvidia_line() {
        let s=parse_nvidia_smi("NVIDIA GeForce RTX 5080, 580.173.02, 42, 57, 2048, 16384, 120.5, 360.0, 2400, 14001\n").unwrap();
        assert_eq!(s.name.as_deref(), Some("NVIDIA GeForce RTX 5080"));
        assert_eq!(s.usage_percent, Some(42.0));
        assert_eq!(s.vram_used_bytes, Some(2048*MIB));
        assert_eq!(s.power_draw_w, Some(120.5));
    }
}
