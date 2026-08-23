// SPDX-License-Identifier: AGPL-3.0-only

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{MIB, cpu::read_temperature_file};

#[derive(Debug, Clone, Default)]
pub(super) struct GpuSpecs {
    pub(super) architecture: Option<String>,
    pub(super) codename: Option<String>,
    pub(super) sm_count: Option<u32>,
    pub(super) cuda_cores: Option<u32>,
    pub(super) tensor_cores: Option<u32>,
    pub(super) tensor_generation: Option<String>,
    pub(super) rt_cores: Option<u32>,
    pub(super) rt_generation: Option<String>,
    pub(super) gpcs: Option<u32>,
    pub(super) tpcs: Option<u32>,
    pub(super) texture_units: Option<u32>,
    pub(super) rops: Option<u32>,
    pub(super) nvenc: Option<String>,
    pub(super) nvdec: Option<String>,
    pub(super) compute_capability: Option<String>,
    pub(super) memory_type: Option<String>,
    pub(super) memory_bus_bits: Option<u32>,
}

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
    pub(super) specs: Option<GpuSpecs>,
}

pub(super) fn query_nvidia_smi() -> Option<GpuSample> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,driver_version,utilization.gpu,temperature.gpu,memory.used,memory.total,power.draw,power.limit,clocks.current.graphics,clocks.current.memory",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| parse_nvidia_smi(&String::from_utf8_lossy(&output.stdout)))
        .flatten()
}

fn parse_nvidia_smi(contents: &str) -> Option<GpuSample> {
    let fields: Vec<_> = contents
        .lines()
        .find(|line| !line.trim().is_empty())?
        .split(',')
        .map(str::trim)
        .collect();
    if fields.len() < 10 {
        return None;
    }

    let float = |value: &str| value.parse::<f64>().ok();
    let mib = |value: &str| {
        value
            .parse::<f64>()
            .ok()
            .filter(|value| *value >= 0.0)
            .map(|value| (value * MIB as f64).round() as u64)
    };
    let text = |value: &str| {
        (!value.is_empty() && value != "[N/A]" && value != "N/A").then(|| value.to_string())
    };

    let name = text(fields[0]);
    let specs = name.as_deref().and_then(nvidia_model_specs);
    let sample = GpuSample {
        name,
        driver_version: text(fields[1]),
        usage_percent: float(fields[2]),
        temperature_c: float(fields[3]),
        vram_used_bytes: mib(fields[4]),
        vram_total_bytes: mib(fields[5]),
        power_draw_w: float(fields[6]),
        power_limit_w: float(fields[7]),
        graphics_clock_mhz: float(fields[8]),
        memory_clock_mhz: float(fields[9]),
        specs,
    };

    (sample.name.is_some()
        || sample.usage_percent.is_some()
        || sample.temperature_c.is_some()
        || sample.vram_total_bytes.is_some())
    .then_some(sample)
}

fn nvidia_model_specs(name: &str) -> Option<GpuSpecs> {
    let normalized = name.to_ascii_lowercase();

    if normalized.contains("geforce rtx 5080") && !normalized.contains("laptop") {
        return Some(GpuSpecs {
            architecture: Some("NVIDIA Blackwell".into()),
            codename: Some("GB203".into()),
            sm_count: Some(84),
            cuda_cores: Some(10_752),
            tensor_cores: Some(336),
            tensor_generation: Some("5th gen".into()),
            rt_cores: Some(84),
            rt_generation: Some("4th gen".into()),
            gpcs: Some(7),
            tpcs: Some(42),
            texture_units: Some(336),
            rops: Some(112),
            nvenc: Some("2 × 9th-gen NVENC".into()),
            nvdec: Some("2 × 6th-gen NVDEC".into()),
            compute_capability: Some("12.0".into()),
            memory_type: Some("GDDR7".into()),
            memory_bus_bits: Some(256),
        });
    }

    None
}

pub(super) fn query_drm_gpu() -> GpuSample {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return GpuSample::default();
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("card") {
            continue;
        }
        let device = entry.path().join("device");
        if !device.exists() {
            continue;
        }
        let usage = read_number(device.join("gpu_busy_percent"));
        let used = read_u64(device.join("mem_info_vram_used"));
        let total = read_u64(device.join("mem_info_vram_total"));
        let temp = read_drm_gpu_temperature(&device);
        if usage.is_some() || used.is_some() || total.is_some() || temp.is_some() {
            return GpuSample {
                usage_percent: usage,
                temperature_c: temp,
                vram_used_bytes: used,
                vram_total_bytes: total,
                name: Some("DRM GPU".into()),
                ..GpuSample::default()
            };
        }
    }
    GpuSample::default()
}

fn read_drm_gpu_temperature(device: &Path) -> Option<f64> {
    fs::read_dir(device.join("hwmon"))
        .ok()?
        .flatten()
        .find_map(|entry| {
            hwmon_temperature_inputs(&entry.path())
                .into_iter()
                .filter_map(|path| read_temperature_file(&path))
                .max_by(f64::total_cmp)
        })
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

fn read_number(path: PathBuf) -> Option<f64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_u64(path: PathBuf) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvidia_line() {
        let sample = parse_nvidia_smi(
            "NVIDIA GeForce RTX 5080, 580.173.02, 42, 57, 2048, 16384, 120.5, 360.0, 2400, 14001\n",
        )
        .unwrap();
        assert_eq!(sample.name.as_deref(), Some("NVIDIA GeForce RTX 5080"));
        assert_eq!(sample.usage_percent, Some(42.0));
        assert_eq!(sample.vram_used_bytes, Some(2048 * MIB));
        assert_eq!(sample.power_draw_w, Some(120.5));
        let specs = sample.specs.unwrap();
        assert_eq!(specs.sm_count, Some(84));
        assert_eq!(specs.cuda_cores, Some(10_752));
        assert_eq!(specs.tensor_cores, Some(336));
        assert_eq!(specs.rt_cores, Some(84));
    }
}
