// SPDX-License-Identifier: AGPL-3.0-only

mod app;
mod config;
mod stats;

fn main() -> cosmic::iced::Result {
    app::run()
}
