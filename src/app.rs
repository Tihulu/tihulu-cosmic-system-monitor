// SPDX-License-Identifier: AGPL-3.0-only

use std::time::Duration;

use cosmic::iced::window::Id;

use crate::stats::SystemStats;

const SYSTEM_MONITOR_APP_ID: &str = "io.github.tihulu.SystemMonitor";

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
        let details = cosmic::widget::list_column()
            .add(cosmic::widget::settings::item(
                "CPU usage",
                cosmic::widget::text::body(self.stats.cpu_usage_text()),
            ))
            .add(cosmic::widget::settings::item(
                "CPU temperature",
                cosmic::widget::text::body(self.stats.cpu_temperature_text()),
            ))
            .add(cosmic::widget::settings::item(
                "GPU usage",
                cosmic::widget::text::body(self.stats.gpu_usage_text()),
            ))
            .add(cosmic::widget::settings::item(
                "GPU temperature",
                cosmic::widget::text::body(self.stats.gpu_temperature_text()),
            ))
            .add(cosmic::widget::settings::item(
                "RAM",
                cosmic::widget::text::body(self.stats.ram_usage_text()),
            ))
            .add(cosmic::widget::settings::item(
                "VRAM",
                cosmic::widget::text::body(self.stats.vram_usage_text()),
            ));

        self.core.applet.popup_container(details).into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}
