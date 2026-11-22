#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod app;
mod config;
mod pty;
mod terminal_view;
mod theme;
mod ui;
mod work;

use app::TerminalPlus;
use iced::Size;

fn main() -> iced::Result {
    #[cfg(all(target_os = "windows", target_env = "gnu"))]
    if std::env::var_os("WGPU_BACKEND").is_none() {
        std::env::set_var("WGPU_BACKEND", "vulkan");
    }

    iced::application(TerminalPlus::title, TerminalPlus::update, TerminalPlus::view)
        .subscription(TerminalPlus::subscription)
        .theme(TerminalPlus::theme)
        .window_size(Size::new(1280.0, 800.0))
        .run_with(TerminalPlus::new)
}
