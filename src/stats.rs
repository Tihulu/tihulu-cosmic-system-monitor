// SPDX-License-Identifier: AGPL-3.0-only

mod cpu;
mod display;
mod gpu;
mod memory;
mod network;
mod power;

use std::collections::VecDeque;

use cpu::{
    CpuSample, cpu_usage_between, read_cpu_frequency_mhz, read_cpu_info, read_cpu_samples,
    read_cpu_temperature, read_load_average, read_uptime_seconds,
};
use display::{
    format_memory, format_memory_long, format_percent, format_rate, format_temperature,
    format_uptime, line_svg_auto, line_svg_fixed, percent_of, push_optional, usage_bar,
};
use gpu::{GpuSample, GpuSpecs, query_drm_gpu, query_nvidia_smi};
use memory::read_memory;
use network::{NetworkSample, read_network_totals};
use power::read_power_snapshot;

pub(super) const KIB: u64 = 1024;
pub(super) const MIB: u64 = 1024 * KIB;
pub(super) const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
pub(super) const HISTORY_LEN: usize = 60;

#[derive(Debug, Default)]
pub(crate) struct SystemStats {
    previous_cpu: Option<CpuSample>,
    previous_cores: Vec<CpuSample>,
    previous_network: Option<NetworkSample>,

    cpu_usage_percent: Option<f64>,
    cpu_temperature_c: Option<f64>,
    core_usage_percent: Vec<f64>,
    cpu_frequency_mhz: Option<f64>,
    cpu_model: Option<String>,
    cpu_logical_cores: Option<usize>,
    cpu_physical_cores: Option<usize>,
    load_average: Option<(f64, f64, f64)>,
    uptime_seconds: Option<f64>,

    gpu_usage_percent: Option<f64>,
    gpu_temperature_c: Option<f64>,
    vram_used_bytes: Option<u64>,
    vram_total_bytes: Option<u64>,
    gpu_name: Option<String>,
    gpu_driver_version: Option<String>,
    gpu_power_draw_w: Option<f64>,
    gpu_power_limit_w: Option<f64>,
    gpu_graphics_clock_mhz: Option<f64>,
    gpu_memory_clock_mhz: Option<f64>,
    gpu_specs: Option<GpuSpecs>,

    ram_used_bytes: Option<u64>,
    ram_total_bytes: Option<u64>,
    swap_used_bytes: Option<u64>,
    swap_total_bytes: Option<u64>,

    network_rx_bytes_per_sec: Option<f64>,
    network_tx_bytes_per_sec: Option<f64>,
    network_interfaces: Vec<String>,

    power_supply_lines: Vec<String>,
    power_sensor_lines: Vec<String>,

    cpu_usage_history: VecDeque<f64>,
    cpu_temp_history: VecDeque<f64>,
    gpu_usage_history: VecDeque<f64>,
    gpu_temp_history: VecDeque<f64>,
    ram_history: VecDeque<f64>,
    swap_history: VecDeque<f64>,
    vram_history: VecDeque<f64>,
    network_rx_history: VecDeque<f64>,
    network_tx_history: VecDeque<f64>,
}

impl SystemStats {
    pub(crate) fn refresh(&mut self) {
        self.refresh_cpu();
        self.cpu_temperature_c = read_cpu_temperature();
        self.cpu_frequency_mhz = read_cpu_frequency_mhz();
        self.load_average = read_load_average();
        self.uptime_seconds = read_uptime_seconds();
        self.ensure_cpu_info();

        if let Some(memory) = read_memory() {
            self.ram_used_bytes = Some(memory.ram_used_bytes);
            self.ram_total_bytes = Some(memory.ram_total_bytes);
            self.swap_used_bytes = Some(memory.swap_used_bytes);
            self.swap_total_bytes = Some(memory.swap_total_bytes);
        }

        self.apply_gpu(query_nvidia_smi().unwrap_or_else(query_drm_gpu));
        self.refresh_network();

        let power = read_power_snapshot();
        self.power_supply_lines = power.supplies;
        self.power_sensor_lines = power.sensors;

        self.record_history();
    }

    fn refresh_cpu(&mut self) {
        let Some(samples) = read_cpu_samples() else {
            return;
        };
        let Some((&current, cores)) = samples.split_first() else {
            return;
        };

        self.cpu_usage_percent = self
            .previous_cpu
            .and_then(|previous| cpu_usage_between(previous, current));
        self.previous_cpu = Some(current);

        self.core_usage_percent = if self.previous_cores.len() == cores.len() {
            self.previous_cores
                .iter()
                .copied()
                .zip(cores.iter().copied())
                .map(|(previous, current)| cpu_usage_between(previous, current).unwrap_or(0.0))
                .collect()
        } else {
            vec![0.0; cores.len()]
        };
        self.previous_cores = cores.to_vec();
    }

    fn ensure_cpu_info(&mut self) {
        if self.cpu_model.is_some() {
            return;
        }
        if let Some((model, logical, physical)) = read_cpu_info() {
            self.cpu_model = Some(model);
            self.cpu_logical_cores = Some(logical);
            self.cpu_physical_cores = physical;
        }
    }

    fn apply_gpu(&mut self, gpu: GpuSample) {
        self.gpu_usage_percent = gpu.usage_percent;
        self.gpu_temperature_c = gpu.temperature_c;
        self.vram_used_bytes = gpu.vram_used_bytes;
        self.vram_total_bytes = gpu.vram_total_bytes;
        if gpu.name.is_some() {
            self.gpu_name = gpu.name;
        }
        if gpu.driver_version.is_some() {
            self.gpu_driver_version = gpu.driver_version;
        }
        if gpu.specs.is_some() {
            self.gpu_specs = gpu.specs;
        }
        self.gpu_power_draw_w = gpu.power_draw_w;
        self.gpu_power_limit_w = gpu.power_limit_w;
        self.gpu_graphics_clock_mhz = gpu.graphics_clock_mhz;
        self.gpu_memory_clock_mhz = gpu.memory_clock_mhz;
    }

    fn refresh_network(&mut self) {
        let Some((rx_bytes, tx_bytes, interfaces)) = read_network_totals() else {
            return;
        };
        self.network_interfaces = interfaces;
        let current = NetworkSample::now(rx_bytes, tx_bytes);
        if let Some(previous) = &self.previous_network {
            if let Some((rx, tx)) = current.rates_since(previous) {
                self.network_rx_bytes_per_sec = Some(rx);
                self.network_tx_bytes_per_sec = Some(tx);
            }
        }
        self.previous_network = Some(current);
    }

    fn record_history(&mut self) {
        push_optional(&mut self.cpu_usage_history, self.cpu_usage_percent);
        push_optional(&mut self.cpu_temp_history, self.cpu_temperature_c);
        push_optional(&mut self.gpu_usage_history, self.gpu_usage_percent);
        push_optional(&mut self.gpu_temp_history, self.gpu_temperature_c);

        let ram_percent = self.ram_percent();
        let swap_percent = self.swap_percent();
        let vram_percent = self.vram_percent();
        push_optional(&mut self.ram_history, ram_percent);
        push_optional(&mut self.swap_history, swap_percent);
        push_optional(&mut self.vram_history, vram_percent);
        push_optional(
            &mut self.network_rx_history,
            self.network_rx_bytes_per_sec.map(|value| value / MIB as f64),
        );
        push_optional(
            &mut self.network_tx_history,
            self.network_tx_bytes_per_sec.map(|value| value / MIB as f64),
        );
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

    pub(crate) fn swap_panel_text(&self) -> String {
        format_memory("SWAP", self.swap_used_bytes, self.swap_total_bytes)
    }

    pub(crate) fn vram_panel_text(&self) -> String {
        format_memory("VRAM", self.vram_used_bytes, self.vram_total_bytes)
    }

    pub(crate) fn network_panel_text(&self) -> String {
        format!(
            "NET ↓{} ↑{}",
            self.network_download_text(),
            self.network_upload_text()
        )
    }

    pub(crate) fn cpu_usage_text(&self) -> String {
        format_percent(self.cpu_usage_percent).trim().to_string()
    }

    pub(crate) fn cpu_temperature_text(&self) -> String {
        format_temperature(self.cpu_temperature_c)
    }

    pub(crate) fn gpu_usage_text(&self) -> String {
        format_percent(self.gpu_usage_percent).trim().to_string()
    }

    pub(crate) fn gpu_temperature_text(&self) -> String {
        format_temperature(self.gpu_temperature_c)
    }

    pub(crate) fn ram_usage_text(&self) -> String {
        format_memory_long(self.ram_used_bytes, self.ram_total_bytes)
    }

    pub(crate) fn swap_usage_text(&self) -> String {
        format_memory_long(self.swap_used_bytes, self.swap_total_bytes)
    }

    pub(crate) fn vram_usage_text(&self) -> String {
        format_memory_long(self.vram_used_bytes, self.vram_total_bytes)
    }

    pub(crate) fn ram_percent_text(&self) -> String {
        format_percent(self.ram_percent()).trim().to_string()
    }

    pub(crate) fn swap_percent_text(&self) -> String {
        format_percent(self.swap_percent()).trim().to_string()
    }

    pub(crate) fn vram_percent_text(&self) -> String {
        format_percent(self.vram_percent()).trim().to_string()
    }

    pub(crate) fn cpu_model_text(&self) -> String {
        self.cpu_model
            .clone()
            .unwrap_or_else(|| "Unknown CPU".into())
    }

    pub(crate) fn cpu_topology_text(&self) -> String {
        match (self.cpu_physical_cores, self.cpu_logical_cores) {
            (Some(physical), Some(logical)) => format!("{physical} physical / {logical} logical"),
            (None, Some(logical)) => format!("{logical} logical"),
            _ => "--".into(),
        }
    }

    pub(crate) fn cpu_frequency_text(&self) -> String {
        self.cpu_frequency_mhz
            .map(|mhz| {
                if mhz >= 1000.0 {
                    format!("{:.2} GHz avg", mhz / 1000.0)
                } else {
                    format!("{mhz:.0} MHz avg")
                }
            })
            .unwrap_or_else(|| "--".into())
    }

    pub(crate) fn load_average_text(&self) -> String {
        self.load_average
            .map(|(one, five, fifteen)| format!("{one:.2} / {five:.2} / {fifteen:.2}"))
            .unwrap_or_else(|| "--".into())
    }

    pub(crate) fn uptime_text(&self) -> String {
        self.uptime_seconds
            .map(format_uptime)
            .unwrap_or_else(|| "--".into())
    }

    pub(crate) fn gpu_name_text(&self) -> String {
        self.gpu_name
            .clone()
            .unwrap_or_else(|| "Unknown GPU".into())
    }

    pub(crate) fn gpu_driver_text(&self) -> String {
        self.gpu_driver_version
            .clone()
            .unwrap_or_else(|| "--".into())
    }

    pub(crate) fn gpu_power_text(&self) -> String {
        match (self.gpu_power_draw_w, self.gpu_power_limit_w) {
            (Some(draw), Some(limit)) => format!("{draw:.1} / {limit:.0} W"),
            (Some(draw), None) => format!("{draw:.1} W"),
            _ => "--".into(),
        }
    }

    pub(crate) fn gpu_clocks_text(&self) -> String {
        match (self.gpu_graphics_clock_mhz, self.gpu_memory_clock_mhz) {
            (Some(graphics), Some(memory)) => {
                format!("core {graphics:.0} MHz / memory {memory:.0} MHz")
            }
            (Some(graphics), None) => format!("core {graphics:.0} MHz"),
            _ => "--".into(),
        }
    }

    pub(crate) fn gpu_architecture_text(&self) -> String {
        self.gpu_specs
            .as_ref()
            .map(|specs| match (&specs.architecture, &specs.codename) {
                (Some(architecture), Some(codename)) => format!("{architecture} ({codename})"),
                (Some(architecture), None) => architecture.clone(),
                _ => "Not exposed by driver for this model".into(),
            })
            .unwrap_or_else(|| "Not exposed by driver for this model".into())
    }

    pub(crate) fn gpu_compute_units_text(&self) -> String {
        self.gpu_specs
            .as_ref()
            .and_then(|specs| Some(format!("{} SM · {} CUDA cores", specs.sm_count?, specs.cuda_cores?)))
            .unwrap_or_else(|| "Not exposed by driver for this model".into())
    }

    pub(crate) fn gpu_tensor_rt_text(&self) -> String {
        let Some(specs) = self.gpu_specs.as_ref() else {
            return "Not exposed by driver for this model".into();
        };
        match (specs.tensor_cores, specs.rt_cores) {
            (Some(tensor), Some(rt)) => format!(
                "{tensor} Tensor ({}) · {rt} RT ({})",
                specs.tensor_generation.as_deref().unwrap_or("generation unknown"),
                specs.rt_generation.as_deref().unwrap_or("generation unknown")
            ),
            _ => "Not exposed by driver for this model".into(),
        }
    }

    pub(crate) fn gpu_partition_text(&self) -> String {
        self.gpu_specs
            .as_ref()
            .and_then(|specs| Some(format!("{} GPC · {} TPC", specs.gpcs?, specs.tpcs?)))
            .unwrap_or_else(|| "--".into())
    }

    pub(crate) fn gpu_raster_text(&self) -> String {
        self.gpu_specs
            .as_ref()
            .and_then(|specs| {
                Some(format!(
                    "{} texture units · {} ROPs",
                    specs.texture_units?, specs.rops?
                ))
            })
            .unwrap_or_else(|| "--".into())
    }

    pub(crate) fn gpu_media_text(&self) -> String {
        self.gpu_specs
            .as_ref()
            .map(|specs| {
                [specs.nvenc.clone(), specs.nvdec.clone()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "--".into())
    }

    pub(crate) fn gpu_cuda_text(&self) -> String {
        self.gpu_specs
            .as_ref()
            .and_then(|specs| specs.compute_capability.as_ref())
            .map(|capability| format!("CUDA compute capability {capability}"))
            .unwrap_or_else(|| "--".into())
    }

    pub(crate) fn gpu_memory_bus_text(&self) -> String {
        self.gpu_specs
            .as_ref()
            .map(|specs| match (&specs.memory_type, specs.memory_bus_bits) {
                (Some(memory_type), Some(bits)) => format!("{memory_type} · {bits}-bit bus"),
                (Some(memory_type), None) => memory_type.clone(),
                _ => "--".into(),
            })
            .unwrap_or_else(|| "--".into())
    }

    pub(crate) fn network_download_text(&self) -> String {
        format_rate(self.network_rx_bytes_per_sec)
    }

    pub(crate) fn network_upload_text(&self) -> String {
        format_rate(self.network_tx_bytes_per_sec)
    }

    pub(crate) fn network_interfaces_text(&self) -> String {
        if self.network_interfaces.is_empty() {
            "--".into()
        } else {
            self.network_interfaces.join(", ")
        }
    }

    pub(crate) fn power_supply_lines(&self) -> &[String] {
        &self.power_supply_lines
    }

    pub(crate) fn power_sensor_lines(&self) -> &[String] {
        &self.power_sensor_lines
    }

    pub(crate) fn cpu_usage_graph(&self) -> String {
        line_svg_fixed(&self.cpu_usage_history, 100.0)
    }

    pub(crate) fn cpu_temperature_graph(&self) -> String {
        line_svg_auto(&self.cpu_temp_history)
    }

    pub(crate) fn gpu_usage_graph(&self) -> String {
        line_svg_fixed(&self.gpu_usage_history, 100.0)
    }

    pub(crate) fn gpu_temperature_graph(&self) -> String {
        line_svg_auto(&self.gpu_temp_history)
    }

    pub(crate) fn ram_graph(&self) -> String {
        line_svg_fixed(&self.ram_history, 100.0)
    }

    pub(crate) fn swap_graph(&self) -> String {
        line_svg_fixed(&self.swap_history, 100.0)
    }

    pub(crate) fn vram_graph(&self) -> String {
        line_svg_fixed(&self.vram_history, 100.0)
    }

    pub(crate) fn network_download_graph(&self) -> String {
        line_svg_auto(&self.network_rx_history)
    }

    pub(crate) fn network_upload_graph(&self) -> String {
        line_svg_auto(&self.network_tx_history)
    }

    pub(crate) fn core_usage(&self) -> &[f64] {
        &self.core_usage_percent
    }

    pub(crate) fn core_usage_line(index: usize, usage: f64) -> String {
        format!(
            "Core {index:02}  {usage:>5.1}%  {}",
            usage_bar(usage, 12)
        )
    }

    fn ram_percent(&self) -> Option<f64> {
        percent_of(self.ram_used_bytes, self.ram_total_bytes)
    }

    fn swap_percent(&self) -> Option<f64> {
        percent_of(self.swap_used_bytes, self.swap_total_bytes)
    }

    fn vram_percent(&self) -> Option<f64> {
        percent_of(self.vram_used_bytes, self.vram_total_bytes)
    }
}
