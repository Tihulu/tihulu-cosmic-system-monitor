// SPDX-License-Identifier: AGPL-3.0-only

use std::time::Duration;

use cosmic::iced::{Length, window::Id};

use crate::stats::SystemStats;

const SYSTEM_MONITOR_APP_ID: &str = "io.github.tihulu.SystemMonitor";
const POPUP_WIDTH: f32 = 330.0;
const POPUP_HEIGHT: f32 = 560.0;

pub(crate) fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<SystemMonitor>(())
}

struct SystemMonitor {
    core: cosmic::app::Core,
    popup: Option<Id>,
    stats: SystemStats,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    TogglePopup,
    PopupClosed(Id),
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
            Message::TogglePopup => {
                if let Some(id) = self.popup.take() {
                    return cosmic::iced::platform_specific::shell::commands::popup::destroy_popup(
                        id,
                    );
                }

                self.stats.refresh();
                let new_id = Id::unique();
                self.popup = Some(new_id);

                let popup_settings = self.core.applet.get_popup_settings(
                    self.core.main_window_id().unwrap(),
                    new_id,
                    None,
                    None,
                    None,
                );

                return cosmic::iced::platform_specific::shell::commands::popup::get_popup(
                    popup_settings,
                );
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
        let content = cosmic::widget::row::with_capacity(4)
            .push(cosmic::widget::text::body(self.stats.cpu_panel_text()))
            .push(cosmic::widget::text::body(self.stats.gpu_panel_text()))
            .push(cosmic::widget::text::body(self.stats.ram_panel_text()))
            .push(cosmic::widget::text::body(self.stats.vram_panel_text()))
            .spacing(12);

        let button = cosmic::widget::button::custom(content)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press_down(Message::TogglePopup);

        cosmic::widget::autosize::autosize(button, cosmic::widget::Id::unique()).into()
    }

    fn view_window(&self, _id: Id) -> cosmic::Element<'_, Self::Message> {
        let summary = cosmic::widget::column::with_capacity(4)
            .push(cosmic::widget::text("Tihulu System Monitor").size(20))
            .push(cosmic::widget::text::caption(
                "Live hardware dashboard · 1 s refresh · last 60 samples",
            ))
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
                    .spacing(16),
            )
            .push(
                cosmic::widget::row::with_capacity(2)
                    .push(summary_cell("RAM", self.stats.ram_percent_text()))
                    .push(summary_cell("VRAM", self.stats.vram_percent_text()))
                    .spacing(16),
            )
            .spacing(8);

        let history = cosmic::widget::column::with_capacity(9)
            .push(section_title("60-second history"))
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
            .push(graph_card(
                "RAM",
                self.stats.ram_percent_text(),
                self.stats.ram_graph(),
            ))
            .push(graph_card(
                "VRAM",
                self.stats.vram_percent_text(),
                self.stats.vram_graph(),
            ))
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
            .spacing(10);

        let cpu_details = cosmic::widget::column::with_capacity(8)
            .push(section_title("CPU"))
            .push(metric_row("Model", self.stats.cpu_model_text()))
            .push(metric_row("Cores", self.stats.cpu_topology_text()))
            .push(metric_row("Average clock", self.stats.cpu_frequency_text()))
            .push(metric_row("Load avg 1 / 5 / 15", self.stats.load_average_text()))
            .push(metric_row("Uptime", self.stats.uptime_text()))
            .push(metric_row("Usage", self.stats.cpu_usage_text()))
            .push(metric_row(
                "Temperature",
                self.stats.cpu_temperature_text(),
            ))
            .spacing(4);

        let gpu_details = cosmic::widget::column::with_capacity(8)
            .push(section_title("GPU"))
            .push(metric_row("Model", self.stats.gpu_name_text()))
            .push(metric_row("Driver", self.stats.gpu_driver_text()))
            .push(metric_row("Usage", self.stats.gpu_usage_text()))
            .push(metric_row(
                "Temperature",
                self.stats.gpu_temperature_text(),
            ))
            .push(metric_row("VRAM", self.stats.vram_usage_text()))
            .push(metric_row("Power", self.stats.gpu_power_text()))
            .push(metric_row("Clocks", self.stats.gpu_clocks_text()))
            .spacing(4);

        let memory_network = cosmic::widget::column::with_capacity(7)
            .push(section_title("Memory & network"))
            .push(metric_row("RAM", self.stats.ram_usage_text()))
            .push(metric_row("Swap", self.stats.swap_usage_text()))
            .push(metric_row("VRAM", self.stats.vram_usage_text()))
            .push(metric_row(
                "Interfaces",
                self.stats.network_interfaces_text(),
            ))
            .push(metric_row(
                "Download",
                self.stats.network_download_text(),
            ))
            .push(metric_row("Upload", self.stats.network_upload_text()))
            .spacing(4);

        let mut core_column =
            cosmic::widget::column::with_capacity(self.stats.core_usage().len() + 1)
                .push(section_title("Per-core CPU usage"))
                .spacing(3);
        for (index, usage) in self.stats.core_usage().iter().copied().enumerate() {
            core_column = core_column.push(cosmic::widget::text::caption(
                SystemStats::core_usage_line(index, usage),
            ));
        }

        let dashboard = cosmic::widget::column::with_capacity(11)
            .push(summary)
            .push(cosmic::widget::text::caption(" "))
            .push(history)
            .push(cosmic::widget::text::caption(" "))
            .push(cpu_details)
            .push(cosmic::widget::text::caption(" "))
            .push(gpu_details)
            .push(cosmic::widget::text::caption(" "))
            .push(memory_network)
            .push(cosmic::widget::text::caption(" "))
            .push(core_column)
            .spacing(10)
            .width(Length::Fill);

        let scroll = cosmic::widget::scrollable(dashboard)
            .height(Length::Fixed(POPUP_HEIGHT))
            .width(Length::Fixed(POPUP_WIDTH));

        self.core.applet.popup_container(scroll).into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

fn summary_cell<'a>(label: &'a str, value: String) -> cosmic::Element<'a, Message> {
    cosmic::widget::column::with_capacity(2)
        .push(cosmic::widget::text::caption(label))
        .push(cosmic::widget::text::body(value))
        .spacing(2)
        .width(Length::FillPortion(1))
        .into()
}

fn section_title<'a>(title: &'a str) -> cosmic::Element<'a, Message> {
    cosmic::widget::text(title).size(16).into()
}

fn metric_row<'a>(label: &'a str, value: String) -> cosmic::Element<'a, Message> {
    cosmic::widget::row::with_capacity(3)
        .push(cosmic::widget::text::body(label))
        .push(cosmic::widget::space::horizontal())
        .push(cosmic::widget::text::body(value))
        .spacing(8)
        .width(Length::Fill)
        .into()
}

fn graph_card<'a>(label: &'a str, value: String, svg: String) -> cosmic::Element<'a, Message> {
    let icon = cosmic::widget::icon::from_svg_bytes(svg.into_bytes()).symbolic(true);
    cosmic::widget::column::with_capacity(2)
        .push(metric_row(label, value))
        .push(
            icon.icon()
                .height(Length::Fixed(72.0))
                .width(Length::Fill),
        )
        .spacing(2)
        .width(Length::Fill)
        .into()
}
