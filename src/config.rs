// SPDX-License-Identifier: AGPL-3.0-only

use std::{env, fs, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelMetric {
    Cpu,
    Gpu,
    Ram,
    Swap,
    Vram,
    Network,
}

#[derive(Debug, Clone)]
pub(crate) struct PanelConfig {
    pub(crate) show_cpu: bool,
    pub(crate) show_gpu: bool,
    pub(crate) show_ram: bool,
    pub(crate) show_swap: bool,
    pub(crate) show_vram: bool,
    pub(crate) show_network: bool,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            show_cpu: true,
            show_gpu: true,
            show_ram: true,
            show_swap: true,
            show_vram: true,
            show_network: false,
        }
    }
}

impl PanelConfig {
    pub(crate) fn load() -> Self {
        let mut config = Self::default();
        let Ok(contents) = fs::read_to_string(config_path()) else {
            return config;
        };

        for line in contents.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let enabled = matches!(value.trim(), "1" | "true" | "yes" | "on");
            match key.trim() {
                "cpu" => config.show_cpu = enabled,
                "gpu" => config.show_gpu = enabled,
                "ram" => config.show_ram = enabled,
                "swap" => config.show_swap = enabled,
                "vram" => config.show_vram = enabled,
                "network" => config.show_network = enabled,
                _ => {}
            }
        }

        config
    }

    pub(crate) fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let contents = format!(
            "cpu={}\ngpu={}\nram={}\nswap={}\nvram={}\nnetwork={}\n",
            self.show_cpu,
            self.show_gpu,
            self.show_ram,
            self.show_swap,
            self.show_vram,
            self.show_network,
        );
        let _ = fs::write(path, contents);
    }

    pub(crate) fn toggle(&mut self, metric: PanelMetric) {
        let value = match metric {
            PanelMetric::Cpu => &mut self.show_cpu,
            PanelMetric::Gpu => &mut self.show_gpu,
            PanelMetric::Ram => &mut self.show_ram,
            PanelMetric::Swap => &mut self.show_swap,
            PanelMetric::Vram => &mut self.show_vram,
            PanelMetric::Network => &mut self.show_network,
        };
        *value = !*value;
    }

    pub(crate) fn is_visible(&self, metric: PanelMetric) -> bool {
        match metric {
            PanelMetric::Cpu => self.show_cpu,
            PanelMetric::Gpu => self.show_gpu,
            PanelMetric::Ram => self.show_ram,
            PanelMetric::Swap => self.show_swap,
            PanelMetric::Vram => self.show_vram,
            PanelMetric::Network => self.show_network,
        }
    }

    pub(crate) fn visible_count(&self) -> usize {
        [
            self.show_cpu,
            self.show_gpu,
            self.show_ram,
            self.show_swap,
            self.show_vram,
            self.show_network,
        ]
        .into_iter()
        .filter(|visible| *visible)
        .count()
    }
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
