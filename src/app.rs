// SPDX-License-Identifier: AGPL-3.0-only

use std::time::Duration;

use crate::stats::SystemStats;

const SYSTEM_MONITOR_APP_ID: &str = "io.github.tihulu.SystemMonitor";

pub(crate) fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<SystemMonitor>(())
}

struct SystemMonitor {
    core: cosmic::app::Core,
    stats: SystemStats,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
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

        (Self { core, stats }, cosmic::task::none())
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

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::Tick => self.stats.refresh(),
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
            .on_press_down(Message::Tick);

        cosmic::widget::autosize::autosize(button, cosmic::widget::Id::unique()).into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}
