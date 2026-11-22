use crate::config::{AppConfig, ShellKind, TerminalTheme};
use crate::theme;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length};

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    SelectShell(ShellKind),
    CustomShellChanged(String),
    SelectFontSize(f32),
    SelectTheme(TerminalTheme),
    OpenWithSelectedShell,
    Close,
}

pub fn view_settings<'a>(config: &'a AppConfig) -> Element<'a, SettingsMessage> {
    let header = row![
        column![
            text("Options & Terminal Preferences")
                .size(18)
                .color(theme::TEXT_PRIMARY),
            text("Choose default shell, color theme & font size")
                .size(12)
                .color(theme::TEXT_MUTED),
        ]
        .spacing(3),
        Space::with_width(Length::Fill),
        button(text("esc").size(12).color(theme::TEXT_MUTED))
            .padding([5, 10])
            .style(theme::card_button_style(false))
            .on_press(SettingsMessage::Close),
    ]
    .align_y(Alignment::Center);

    let mut shells_col = column![].spacing(8);
    for shell in ShellKind::ALL {
        let is_selected = config.default_shell == shell;
        let available = shell.is_available(&config.custom_shell_path);
        let (cmd, args) = shell.resolve_command(&config.custom_shell_path);
        let subtitle = if args.is_empty() {
            cmd
        } else {
            format!("{} {}", cmd, args.join(" "))
        };

        let status_label = if is_selected {
            ("Active default", theme::BLUE)
        } else if available {
            ("Detected", theme::GREEN)
        } else {
            ("Not found", theme::TEXT_MUTED)
        };

        let card = button(
            row![
                column![
                    text(shell.label()).size(13).color(theme::TEXT_PRIMARY),
                    text(subtitle).size(11).color(theme::TEXT_MUTED),
                ]
                .spacing(3),
                Space::with_width(Length::Fill),
                text(status_label.0).size(11).color(status_label.1),
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .padding([10, 14])
        .style(theme::card_button_style(is_selected))
        .on_press(SettingsMessage::SelectShell(shell));

        shells_col = shells_col.push(card);
    }

    let custom_input: Element<'a, SettingsMessage> = if config.default_shell == ShellKind::Custom {
        column![
            text("Custom executable path")
                .size(11)
                .color(theme::TEXT_MUTED),
            text_input(r"C:\path\to\your-shell.exe", &config.custom_shell_path)
                .size(12)
                .padding([8, 10])
                .style(theme::search_input_style)
                .on_input(SettingsMessage::CustomShellChanged),
        ]
        .spacing(6)
        .into()
    } else {
        Space::with_height(0).into()
    };

    let font_sizes: [f32; 4] = [12.0, 13.5, 15.0, 16.5];
    let mut font_row = row![].spacing(8);
    for size in font_sizes {
        let active = (config.font_size - size).abs() < 0.1;
        font_row = font_row.push(
            button(text(format!("{size:.1} px")).size(12))
                .padding([6, 12])
                .style(theme::card_button_style(active))
                .on_press(SettingsMessage::SelectFontSize(size)),
        );
    }

    let mut themes_row = row![].spacing(8);
    for t in TerminalTheme::ALL {
        let active = config.terminal_theme == t;
        themes_row = themes_row.push(
            button(text(t.label()).size(12))
                .padding([7, 14])
                .style(theme::card_button_style(active))
                .on_press(SettingsMessage::SelectTheme(t)),
        );
    }

    let config_path_str = AppConfig::config_path().display().to_string();

    let body = scrollable(
        column![
            header,
            container(
                column![
                    text("DEFAULT TERMINAL SHELL")
                        .size(11)
                        .color(theme::TEXT_MUTED),
                    shells_col,
                    custom_input,
                    row![
                        Space::with_width(Length::Fill),
                        button(
                            text("Open new session with selected shell")
                                .size(12)
                                .color(theme::TEXT_PRIMARY),
                        )
                        .padding([8, 14])
                        .style(theme::card_button_style(true))
                        .on_press(SettingsMessage::OpenWithSelectedShell),
                    ],
                ]
                .spacing(12)
                .padding(16),
            )
            .width(Length::Fill)
            .style(theme::card_style),
            container(
                column![
                    text("COLOR THEME").size(11).color(theme::TEXT_MUTED),
                    themes_row,
                    Space::with_height(4),
                    text("TERMINAL FONT SIZE").size(11).color(theme::TEXT_MUTED),
                    font_row,
                    text(format!("Saved automatically to {config_path_str}"))
                        .size(11)
                        .color(theme::TEXT_MUTED),
                ]
                .spacing(10)
                .padding(16),
            )
            .width(Length::Fill)
            .style(theme::card_style),
            container(
                row![
                    column![
                        text("REMOTE WORK PCs")
                            .size(11)
                            .color(theme::TEXT_MUTED),
                        text("Remote device sync & encrypted terminal sessions")
                            .size(12)
                            .color(theme::TEXT_MUTED),
                    ]
                    .spacing(4),
                    Space::with_width(Length::Fill),
                    container(text("Coming soon").size(11).color(theme::AMBER))
                        .padding([4, 10])
                        .style(theme::card_style),
                ]
                .align_y(Alignment::Center)
                .padding(16),
            )
            .width(Length::Fill)
            .style(theme::card_style),
        ]
        .spacing(16)
        .padding(24),
    )
    .height(Length::Fill)
    .style(theme::dark_scrollable_style);

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::app_bg)
        .into()
}
