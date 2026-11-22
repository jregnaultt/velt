use crate::config::TerminalTheme;
use iced::widget::{button, container, scrollable, text_input};
use iced::{Background, Border, Color, Font, Theme};

pub const BG: Color = Color::from_rgb(0x0A as f32 / 255.0, 0x0A as f32 / 255.0, 0x0B as f32 / 255.0);
pub const SIDEBAR_BG: Color =
    Color::from_rgb(0x11 as f32 / 255.0, 0x11 as f32 / 255.0, 0x13 as f32 / 255.0);
pub const CARD_BG: Color =
    Color::from_rgb(0x16 as f32 / 255.0, 0x16 as f32 / 255.0, 0x19 as f32 / 255.0);
pub const CARD_HOVER_BG: Color =
    Color::from_rgb(0x1D as f32 / 255.0, 0x1D as f32 / 255.0, 0x22 as f32 / 255.0);
pub const BORDER: Color =
    Color::from_rgb(0x22 as f32 / 255.0, 0x22 as f32 / 255.0, 0x26 as f32 / 255.0);
pub const BORDER_ACTIVE: Color =
    Color::from_rgb(0x3F as f32 / 255.0, 0x3F as f32 / 255.0, 0x46 as f32 / 255.0);
pub const TEXT_PRIMARY: Color =
    Color::from_rgb(0xE4 as f32 / 255.0, 0xE4 as f32 / 255.0, 0xE7 as f32 / 255.0);
pub const TEXT_MUTED: Color =
    Color::from_rgb(0x71 as f32 / 255.0, 0x71 as f32 / 255.0, 0x7A as f32 / 255.0);
pub const GREEN: Color =
    Color::from_rgb(0x22 as f32 / 255.0, 0xC5 as f32 / 255.0, 0x5E as f32 / 255.0);
pub const AMBER: Color =
    Color::from_rgb(0xF5 as f32 / 255.0, 0x9E as f32 / 255.0, 0x0B as f32 / 255.0);
pub const BLUE: Color =
    Color::from_rgb(0x3B as f32 / 255.0, 0x82 as f32 / 255.0, 0xF6 as f32 / 255.0);
pub const RED: Color =
    Color::from_rgb(0xEF as f32 / 255.0, 0x44 as f32 / 255.0, 0x44 as f32 / 255.0);
pub const ORANGE: Color =
    Color::from_rgb(0xEA as f32 / 255.0, 0x58 as f32 / 255.0, 0x0C as f32 / 255.0);
#[allow(dead_code)]
pub const PURPLE: Color =
    Color::from_rgb(0x8B as f32 / 255.0, 0x5C as f32 / 255.0, 0xF6 as f32 / 255.0);

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub fg: Color,
    pub cursor: Color,
    pub selection: Color,
    pub red: Color,
    pub green: Color,
    pub amber: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
}

pub fn terminal_palette(t: TerminalTheme) -> Palette {
    match t {
        TerminalTheme::MidtermDark => Palette {
            bg: BG,
            fg: TEXT_PRIMARY,
            cursor: TEXT_PRIMARY,
            selection: Color::from_rgba8(0x3B, 0x82, 0xF6, 0.32),
            red: RED,
            green: GREEN,
            amber: AMBER,
            blue: BLUE,
            magenta: Color::from_rgb8(0xA8, 0x55, 0xF7),
            cyan: Color::from_rgb8(0x06, 0xB6, 0xD4),
        },
        TerminalTheme::TokyoNight => Palette {
            bg: Color::from_rgb8(0x1A, 0x1B, 0x26),
            fg: Color::from_rgb8(0xC0, 0xCA, 0xF5),
            cursor: Color::from_rgb8(0x7A, 0xA2, 0xF7),
            selection: Color::from_rgba8(0x33, 0x46, 0x7C, 0.55),
            red: Color::from_rgb8(0xF7, 0x76, 0x8E),
            green: Color::from_rgb8(0x9E, 0xCE, 0x6A),
            amber: Color::from_rgb8(0xE0, 0xAF, 0x68),
            blue: Color::from_rgb8(0x7A, 0xA2, 0xF7),
            magenta: Color::from_rgb8(0xBB, 0x9A, 0xF7),
            cyan: Color::from_rgb8(0x7D, 0xCF, 0xFF),
        },
        TerminalTheme::CatppuccinMocha => Palette {
            bg: Color::from_rgb8(0x1E, 0x1E, 0x2E),
            fg: Color::from_rgb8(0xCD, 0xD6, 0xF4),
            cursor: Color::from_rgb8(0xF5, 0xE0, 0xDC),
            selection: Color::from_rgba8(0x58, 0x5B, 0x70, 0.55),
            red: Color::from_rgb8(0xF3, 0x8B, 0xA8),
            green: Color::from_rgb8(0xA6, 0xE3, 0xA1),
            amber: Color::from_rgb8(0xF9, 0xE2, 0xAF),
            blue: Color::from_rgb8(0x89, 0xB4, 0xFA),
            magenta: Color::from_rgb8(0xCB, 0xA6, 0xF7),
            cyan: Color::from_rgb8(0x94, 0xE2, 0xD5),
        },
    }
}

pub fn detect_terminal_font() -> Font {
    let local_fonts = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .map(|p| p.join("Microsoft").join("Windows").join("Fonts"));
    let sys_fonts = std::path::PathBuf::from(r"C:\Windows\Fonts");

    let has_font = |file: &str| {
        sys_fonts.join(file).is_file()
            || local_fonts
                .as_ref()
                .map(|d| d.join(file).is_file())
                .unwrap_or(false)
    };

    if has_font("CascadiaMono.ttf") || has_font("CascadiaCode.ttf") {
        Font::with_name("Cascadia Mono")
    } else if has_font("JetBrainsMono-Regular.ttf") {
        Font::with_name("JetBrains Mono")
    } else if has_font("consola.ttf") {
        Font::with_name("Consolas")
    } else {
        Font::MONOSPACE
    }
}

pub fn app_bg(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG)),
        text_color: Some(TEXT_PRIMARY),
        ..Default::default()
    }
}

pub fn sidebar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SIDEBAR_BG)),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn top_bar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SIDEBAR_BG)),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn card_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(CARD_BG)),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

pub fn pane_container_style(focused: bool) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(BG)),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            color: if focused { BORDER_ACTIVE } else { BORDER },
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

#[allow(dead_code)]
pub fn pill_badge_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(CARD_BG)),
        text_color: Some(TEXT_PRIMARY),
        border: Border {
            color: BORDER_ACTIVE,
            width: 1.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    }
}

pub fn divider_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BORDER)),
        ..Default::default()
    }
}

pub fn ghost_button_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let bg = if active {
            CARD_HOVER_BG
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => CARD_BG,
                _ => Color::TRANSPARENT,
            }
        };
        let border_color = if active {
            BORDER_ACTIVE
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => BORDER,
                _ => Color::TRANSPARENT,
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: if active { TEXT_PRIMARY } else { TEXT_MUTED },
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    }
}

pub fn card_button_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => CARD_HOVER_BG,
            _ => {
                if active {
                    CARD_HOVER_BG
                } else {
                    CARD_BG
                }
            }
        };
        let border_color = if active {
            BORDER_ACTIVE
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => BORDER_ACTIVE,
                _ => BORDER,
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: TEXT_PRIMARY,
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    }
}

pub fn tab_button_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let bg = if active {
            BG
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => CARD_BG,
                _ => Color::TRANSPARENT,
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: if active { TEXT_PRIMARY } else { TEXT_MUTED },
            border: Border {
                color: if active { BORDER } else { Color::TRANSPARENT },
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        }
    }
}

pub fn filter_pill_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let bg = if active {
            CARD_HOVER_BG
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => CARD_BG,
                _ => Color::TRANSPARENT,
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: if active { TEXT_PRIMARY } else { TEXT_MUTED },
            border: Border {
                color: if active { BORDER_ACTIVE } else { BORDER },
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        }
    }
}

pub fn search_input_style(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused => BORDER_ACTIVE,
        text_input::Status::Hovered => BORDER_ACTIVE,
        _ => BORDER,
    };
    text_input::Style {
        background: Background::Color(BG),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 6.0.into(),
        },
        icon: TEXT_MUTED,
        placeholder: TEXT_MUTED,
        value: TEXT_PRIMARY,
        selection: Color {
            a: 0.35,
            ..BLUE
        },
    }
}

pub fn dark_scrollable_style(_theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let scroller_color = match status {
        scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => BORDER_ACTIVE,
        _ => BORDER,
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scrollable::Scroller {
                color: scroller_color,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
            },
        },
        horizontal_rail: scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scrollable::Scroller {
                color: scroller_color,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
            },
        },
        gap: None,
    }
}
