// SPDX-License-Identifier: AGPL-3.0-only

use std::time::Duration;

use cosmic::iced::{Length, widget::mouse_area, window::Id};

use crate::{
    config::{PanelConfig, PanelMetric},
    stats::SystemStats,
};

const SYSTEM_MONITOR_APP_ID: &str = "io.github.tihulu.SystemMonitor";
const POPUP_WIDTH: f32 = 308.0;
const POPUP_HEIGHT: f32 = 540.0;
const GRAPH_HEIGHT: f32 = 52.0;

pub(crate) fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<SystemMonitor>(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopupKind {
    Dashboard,
    PanelSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DashboardTab {
    Overview,
    Cpu,
    Gpu,
    Memory,
    Network,
    Psu,
}

struct SystemMonitor {
    core: cosmic::app::Core,
    popup: Option<Id>,
    popup_kind: PopupKind,
    dashboard_tab: DashboardTab,
    panel_config: PanelConfig,
    stats: SystemStats,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    ToggleDashboard,
    OpenPanelSettings,
    SetDashboardTab(DashboardTab),
    TogglePanelMetric(PanelMetric),
    PopupClosed(Id),
}

impl SystemMonitor {
    fn toggle_popup(&mut self, kind: PopupKind) -> cosmic::app::Task<Message> {
        if let Some(id) = self.popup {
            if self.popup_kind == kind {
                self.popup = None;
                return cosmic::iced::platform_specific::shell::commands::popup::destroy_popup(id);
            }

            self.popup_kind = kind;
            self.stats.refresh();
            return cosmic::task::none();
        }

        self.stats.refresh();
        self.popup_kind = kind;
        let new_id = Id::unique();
        self.popup = Some(new_id);

        let popup_settings = self.core.applet.get_popup_settings(
            self.core.main_window_id().unwrap(),
            new_id,
            None,
            None,
            None,
        );

        cosmic::iced::platform_specific::shell::commands::popup::get_popup(popup_settings)
    }

    fn panel_settings_view(&self) -> cosmic::Element<'_, Message> {
        let settings = cosmic::widget::column::with_capacity(9)
            .push(cosmic::widget::text("Panel display").size(20))
            .push(cosmic::widget::text::caption(
                "Choose which live metrics are shown in the COSMIC panel.",
            ))
            .push(panel_metric_button(
                "CPU usage + temperature",
                PanelMetric::Cpu,
                self.panel_config.is_visible(PanelMetric::Cpu),
            ))
            .push(panel_metric_button(
                "GPU usage + temperature",
                PanelMetric::Gpu,
                self.panel_config.is_visible(PanelMetric::Gpu),
            ))
            .push(panel_metric_button(
                "RAM used / total",
                PanelMetric::Ram,
                self.panel_config.is_visible(PanelMetric::Ram),
            ))
            .push(panel_metric_button(
                "Swap used / total",
                PanelMetric::Swap,
                self.panel_config.is_visible(PanelMetric::Swap),
            ))
            .push(panel_metric_button(
                "VRAM used / total",
                PanelMetric::Vram,
                self.panel_config.is_visible(PanelMetric::Vram),
            ))
            .push(panel_metric_button(
                "Network download / upload",
                PanelMetric::Network,
                self.panel_config.is_visible(PanelMetric::Network),
            ))
            .push(cosmic::widget::text::caption(
                "Left-click the applet for the dashboard. Right-click again to close.",
            ))
            .spacing(8)
            .width(Length::Fixed(POPUP_WIDTH));

        self.core.applet.popup_container(settings).into()
    }

    fn dashboard_header(&self) -> cosmic::Element<'_, Message> {
        cosmic::widget::column::with_capacity(4)
            .push(cosmic::widget::text("Tihulu System Monitor").size(20))
            .push(cosmic::widget::text::caption(
                "Live hardware dashboard · 1 s refresh · last 60 samples",
            ))
            .push(
                cosmic::widget::row::with_capacity(3)
                    .push(tab_button("Overview", DashboardTab::Overview, self.dashboard_tab))
                    .push(tab_button("CPU", DashboardTab::Cpu, self.dashboard_tab))
                    .push(tab_button("GPU", DashboardTab::Gpu, self.dashboard_tab))
                    .spacing(4),
            )
            .push(
                cosmic::widget::row::with_capacity(3)
                    .push(tab_button("Memory", DashboardTab::Memory, self.dashboard_tab))
                    .push(tab_button("Network", DashboardTab::Network, self.dashboard_tab))
                    .push(tab_button("PSU", DashboardTab::Psu, self.dashboard_tab))
                    .spacing(4),
            )
            .spacing(6)
            .width(Length::Fill)
            .into()
    }

    fn overview_tab(&self) -> cosmic::Element<'_, Message> {
        cosmic::widget::column::with_capacity(7)
            .push(
                cosmic::widget::row::with_capacity(2)
                    .push(summary_cell(
                        "CPU",
                        format!(
                            "{}  {}",
                            self.stats.cpu_usage_text(),
                            self.stats.cpu_temperature_text()
                        ),
                    ))
                    .push(summary_cell(
                        "GPU",
                        format!(
                            "{}  {}",
                            self.stats.gpu_usage_text(),
                            self.stats.gpu_temperature_text()
                        ),
                    ))
                    .spacing(12),
            )
            .push(
                cosmic::widget::row::with_capacity(2)
                    .push(summary_cell("RAM", self.stats.ram_percent_text()))
                    .push(summary_cell("Swap", self.stats.swap_percent_text()))
                    .spacing(12),
            )
            .push(
                cosmic::widget::row::with_capacity(2)
                    .push(summary_cell("VRAM", self.stats.vram_percent_text()))
                    .push(summary_cell(
                        "Network",
                        format!(
                            "↓ {}  ↑ {}",
                            self.stats.network_download_text(),
                            self.stats.network_upload_text()
                        ),
                    ))
                    .spacing(12),
            )
            .push(section_title("60-second history"))
            .push(graph_card(
                "CPU usage",
                self.stats.cpu_usage_text(),
                self.stats.cpu_usage_graph(),
            ))
            .push(graph_card(
                "GPU usage",
                self.stats.gpu_usage_text(),
                self.stats.gpu_usage_graph(),
            ))
            .spacing(8)
            .into()
    }

    fn cpu_tab(&self) -> cosmic::Element<'_, Message> {
        let mut cores = cosmic::widget::column::with_capacity(self.stats.core_usage().len() + 1)
            .push(section_title("Per-core CPU usage"))
            .spacing(3);
        for (index, usage) in self.stats.core_usage().iter().copied().enumerate() {
            cores = cores.push(cosmic::widget::text::caption(SystemStats::core_usage_line(
                index, usage,
            )));
        }

        cosmic::widget::column::with_capacity(11)
            .push(metric_block("Model", self.stats.cpu_model_text()))
            .push(metric_block("Cores", self.stats.cpu_topology_text()))
            .push(metric_block("Average clock", self.stats.cpu_frequency_text()))
            .push(metric_block(
                "Load avg 1 / 5 / 15",
                self.stats.load_average_text(),
            ))
            .push(metric_block("Uptime", self.stats.uptime_text()))
            .push(graph_card(
                "CPU usage",
                self.stats.cpu_usage_text(),
                self.stats.cpu_usage_graph(),
            ))
            .push(graph_card(
                "CPU temperature",
                self.stats.cpu_temperature_text(),
                self.stats.cpu_temperature_graph(),
            ))
            .push(cores)
            .spacing(8)
            .into()
    }

    fn gpu_tab(&self) -> cosmic::Element<'_, Message> {
        cosmic::widget::column::with_capacity(18)
            .push(metric_block("Model", self.stats.gpu_name_text()))
            .push(metric_block("Driver", self.stats.gpu_driver_text()))
            .push(metric_block(
                "Architecture",
                self.stats.gpu_architecture_text(),
            ))
            .push(metric_block(
                "SM / CUDA cores",
                self.stats.gpu_compute_units_text(),
            ))
            .push(metric_block(
                "Tensor / RT cores",
                self.stats.gpu_tensor_rt_text(),
            ))
            .push(metric_block(
                "GPC / TPC partitions",
                self.stats.gpu_partition_text(),
            ))
            .push(metric_block(
                "Texture / raster units",
                self.stats.gpu_raster_text(),
            ))
            .push(metric_block("Media engines", self.stats.gpu_media_text()))
            .push(metric_block("CUDA", self.stats.gpu_cuda_text()))
            .push(metric_block(
                "Memory interface",
                self.stats.gpu_memory_bus_text(),
            ))
            .push(metric_block("Power", self.stats.gpu_power_text()))
            .push(metric_block("Clocks", self.stats.gpu_clocks_text()))
            .push(metric_block("VRAM", self.stats.vram_usage_text()))
            .push(graph_card(
                "GPU usage",
                self.stats.gpu_usage_text(),
                self.stats.gpu_usage_graph(),
            ))
            .push(graph_card(
                "GPU temperature",
                self.stats.gpu_temperature_text(),
                self.stats.gpu_temperature_graph(),
            ))
            .spacing(7)
            .into()
    }

    fn memory_tab(&self) -> cosmic::Element<'_, Message> {
        cosmic::widget::column::with_capacity(10)
            .push(metric_block("RAM", self.stats.ram_usage_text()))
            .push(graph_card(
                "RAM",
                self.stats.ram_percent_text(),
                self.stats.ram_graph(),
            ))
            .push(metric_block("Swap", self.stats.swap_usage_text()))
            .push(graph_card(
                "Swap",
                self.stats.swap_percent_text(),
                self.stats.swap_graph(),
            ))
            .push(metric_block("VRAM", self.stats.vram_usage_text()))
            .push(graph_card(
                "VRAM",
                self.stats.vram_percent_text(),
                self.stats.vram_graph(),
            ))
            .spacing(8)
            .into()
    }

    fn network_tab(&self) -> cosmic::Element<'_, Message> {
        cosmic::widget::column::with_capacity(8)
            .push(metric_block(
                "Interfaces",
                self.stats.network_interfaces_text(),
            ))
            .push(metric_block(
                "Download",
                self.stats.network_download_text(),
            ))
            .push(metric_block("Upload", self.stats.network_upload_text()))
            .push(graph_card(
                "Network download",
                self.stats.network_download_text(),
                self.stats.network_download_graph(),
            ))
            .push(graph_card(
                "Network upload",
                self.stats.network_upload_text(),
                self.stats.network_upload_graph(),
            ))
            .spacing(8)
            .into()
    }

    fn psu_tab(&self) -> cosmic::Element<'_, Message> {
        let supply_lines = self.stats.power_supply_lines();
        let sensor_lines = self.stats.power_sensor_lines();

        let mut content = cosmic::widget::column::with_capacity(
            supply_lines.len() + sensor_lines.len() + 6,
        )
        .push(section_title("PSU / power"))
        .push(metric_block(
            "GPU board power",
            self.stats.gpu_power_text(),
        ));

        if supply_lines.is_empty() {
            content = content.push(metric_block(
                "PSU / AC telemetry",
                "Not exposed by this hardware/driver".into(),
            ));
        } else {
            content = content.push(section_title("Power supplies exposed by Linux"));
            for line in supply_lines {
                content = content.push(metric_block("Supply", line.clone()));
            }
        }

        if sensor_lines.is_empty() {
            content = content.push(metric_block(
                "hwmon power sensors",
                "No additional power sensors exposed".into(),
            ));
        } else {
            content = content.push(section_title("Other reported power sensors"));
            for line in sensor_lines {
                content = content.push(metric_block("Sensor", line.clone()));
            }
        }

        content
            .push(cosmic::widget::text::caption(
                "Desktop ATX PSUs usually do not expose model, rated wattage, rail current, efficiency or total wall draw to Linux. hwmon readings above are individual device sensors, not guaranteed PSU total power.",
            ))
            .spacing(8)
            .into()
    }

    fn dashboard_view(&self) -> cosmic::Element<'_, Message> {
        let body = match self.dashboard_tab {
            DashboardTab::Overview => self.overview_tab(),
            DashboardTab::Cpu => self.cpu_tab(),
            DashboardTab::Gpu => self.gpu_tab(),
            DashboardTab::Memory => self.memory_tab(),
            DashboardTab::Network => self.network_tab(),
            DashboardTab::Psu => self.psu_tab(),
        };

        let dashboard = cosmic::widget::column::with_capacity(2)
            .push(self.dashboard_header())
            .push(body)
            .spacing(10)
            .width(Length::Fill);

        let scroll = cosmic::widget::scrollable(dashboard)
            .height(Length::Fixed(POPUP_HEIGHT))
            .width(Length::Fixed(POPUP_WIDTH));

        self.core.applet.popup_container(scroll).into()
    }
}

impl cosmic::Application for SystemMonitor {
    type Flags = ();
    type Message = Message;
    type Executor = cosmic::SingleThreadExecutor;

    const APP_ID: &'static str = SYSTEM_MONITOR_APP_ID;

    fn init(
        core: cosmic::app::Core,
        _flags: Self::Flags,
    ) -> (Self, cosmic::app::Task<Self::Message>) {
        let mut stats = SystemStats::default();
        stats.refresh();

        (
            Self {
                core,
                popup: None,
                popup_kind: PopupKind::Dashboard,
                dashboard_tab: DashboardTab::Overview,
                panel_config: PanelConfig::load(),
                stats,
            },
            cosmic::task::none(),
        )
    }

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        cosmic::iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)
    }

    fn on_close_requested(&self, id: Id) -> Option<Self::Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::Tick => self.stats.refresh(),
            Message::ToggleDashboard => return self.toggle_popup(PopupKind::Dashboard),
            Message::OpenPanelSettings => return self.toggle_popup(PopupKind::PanelSettings),
            Message::SetDashboardTab(tab) => self.dashboard_tab = tab,
            Message::TogglePanelMetric(metric) => {
                self.panel_config.toggle(metric);
                self.panel_config.save();
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
        }

        cosmic::task::none()
    }

    fn view(&self) -> cosmic::Element<'_, Self::Message> {
        let mut content = cosmic::widget::row::with_capacity(6).spacing(12);

        if self.panel_config.show_cpu {
            content = content.push(cosmic::widget::text::body(self.stats.cpu_panel_text()));
        }
        if self.panel_config.show_gpu {
            content = content.push(cosmic::widget::text::body(self.stats.gpu_panel_text()));
        }
        if self.panel_config.show_ram {
            content = content.push(cosmic::widget::text::body(self.stats.ram_panel_text()));
        }
        if self.panel_config.show_swap {
            content = content.push(cosmic::widget::text::body(self.stats.swap_panel_text()));
        }
        if self.panel_config.show_vram {
            content = content.push(cosmic::widget::text::body(self.stats.vram_panel_text()));
        }
        if self.panel_config.show_network {
            content = content.push(cosmic::widget::text::body(self.stats.network_panel_text()));
        }
        if self.panel_config.visible_count() == 0 {
            content = content.push(cosmic::widget::text::body("SYS"));
        }

        let button = cosmic::widget::button::custom(content)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press_down(Message::ToggleDashboard);

        let interactive = mouse_area(button).on_right_release(Message::OpenPanelSettings);

        cosmic::widget::autosize::autosize(interactive, cosmic::widget::Id::unique()).into()
    }

    fn view_window(&self, _id: Id) -> cosmic::Element<'_, Self::Message> {
        match self.popup_kind {
            PopupKind::Dashboard => self.dashboard_view(),
            PopupKind::PanelSettings => self.panel_settings_view(),
        }
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

fn tab_button<'a>(
    label: &'a str,
    tab: DashboardTab,
    active: DashboardTab,
) -> cosmic::Element<'a, Message> {
    let marker = if tab == active { "●" } else { "○" };
    cosmic::widget::button::custom(cosmic::widget::text::caption(format!("{marker} {label}")))
        .on_press(Message::SetDashboardTab(tab))
        .width(Length::FillPortion(1))
        .into()
}

fn panel_metric_button<'a>(
    label: &'a str,
    metric: PanelMetric,
    visible: bool,
) -> cosmic::Element<'a, Message> {
    let marker = if visible { "✓" } else { "○" };
    let content = cosmic::widget::row::with_capacity(3)
        .push(cosmic::widget::text::body(marker))
        .push(cosmic::widget::text::body(label))
        .push(cosmic::widget::space::horizontal())
        .spacing(8)
        .width(Length::Fill);

    cosmic::widget::button::custom(content)
        .on_press(Message::TogglePanelMetric(metric))
        .width(Length::Fill)
        .into()
}

fn summary_cell<'a>(label: &'a str, value: String) -> cosmic::Element<'a, Message> {
    cosmic::widget::column::with_capacity(2)
        .push(cosmic::widget::text::caption(label))
        .push(cosmic::widget::text::body(value).width(Length::Fill))
        .spacing(2)
        .width(Length::FillPortion(1))
        .into()
}

fn section_title<'a>(title: &'a str) -> cosmic::Element<'a, Message> {
    cosmic::widget::text(title).size(16).into()
}

fn metric_block<'a>(label: &'a str, value: String) -> cosmic::Element<'a, Message> {
    cosmic::widget::column::with_capacity(2)
        .push(cosmic::widget::text::caption(label))
        .push(cosmic::widget::text::body(value).width(Length::Fill))
        .spacing(1)
        .width(Length::Fill)
        .into()
}

fn graph_card<'a>(label: &'a str, value: String, svg: String) -> cosmic::Element<'a, Message> {
    let icon = cosmic::widget::icon::from_svg_bytes(svg.into_bytes()).symbolic(true);
    cosmic::widget::column::with_capacity(2)
        .push(metric_block(label, value))
        .push(
            icon.icon()
                .height(Length::Fixed(GRAPH_HEIGHT))
                .width(Length::Fill),
        )
        .spacing(2)
        .width(Length::Fill)
        .into()
}
