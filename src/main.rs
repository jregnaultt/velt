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

const VELT_ICON_RGBA: &[u8] = include_bytes!("../assets/branding/velt-icon-64.rgba");

fn main() -> iced::Result {
    #[cfg(all(target_os = "windows", target_env = "gnu"))]
    if std::env::var_os("WGPU_BACKEND").is_none() {
        std::env::set_var("WGPU_BACKEND", "vulkan");
    }

    let icon = iced::window::icon::from_rgba(VELT_ICON_RGBA.to_vec(), 64, 64).ok();

    iced::application(TerminalPlus::title, TerminalPlus::update, TerminalPlus::view)
        .subscription(TerminalPlus::subscription)
        .theme(TerminalPlus::theme)
        .window(iced::window::Settings {
            size: Size::new(1280.0, 800.0),
            icon,
            ..Default::default()
        })
        .run_with(TerminalPlus::new)
}
