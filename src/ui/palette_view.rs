use crate::config::ShellKind;
use crate::theme;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length, Padding};

#[derive(Debug, Clone)]
pub enum PaletteAction {
    NewShell(ShellKind),
    SplitHorizontal,
    SplitVertical,
    CloseSplit,
    ToggleSidebar,
    OpenUsageToday,
    OpenUsage30d,
    OpenAgyHub,
    OpenWorkHub,
    OpenSettings,
    FindInTerminal,
    RunCommand(&'static str),
}

struct PaletteItem {
    title: &'static str,
    hint: &'static str,
    category: &'static str,
    action: PaletteAction,
}

pub fn view_palette<'a, M: 'a + Clone>(
    query: &'a str,
    input_id: text_input::Id,
    on_query_change: fn(String) -> M,
    on_run_action: fn(PaletteAction) -> M,
    on_close: M,
) -> Element<'a, M> {
    let items = [
        PaletteItem {
            title: "New session: Git Bash",
            hint: "bash",
            category: "Shell",
            action: PaletteAction::NewShell(ShellKind::GitBash),
        },
        PaletteItem {
            title: "New session: WSL",
            hint: "wsl",
            category: "Shell",
            action: PaletteAction::NewShell(ShellKind::Wsl),
        },
        PaletteItem {
            title: "New session: Windows PowerShell",
            hint: "powershell",
            category: "Shell",
            action: PaletteAction::NewShell(ShellKind::PowerShell),
        },
        PaletteItem {
            title: "New session: Command Prompt (CMD)",
            hint: "cmd",
            category: "Shell",
            action: PaletteAction::NewShell(ShellKind::Cmd),
        },
        PaletteItem {
            title: "Split Pane: Side-by-Side (Horizontal)",
            hint: "Ctrl+\\",
            category: "Layout",
            action: PaletteAction::SplitHorizontal,
        },
        PaletteItem {
            title: "Split Pane: Top / Bottom (Vertical)",
            hint: "2 rows",
            category: "Layout",
            action: PaletteAction::SplitVertical,
        },
        PaletteItem {
            title: "Close Split Pane",
            hint: "1 pane",
            category: "Layout",
            action: PaletteAction::CloseSplit,
        },
        PaletteItem {
            title: "Toggle Sidebar",
            hint: "Ctrl+B",
            category: "Layout",
            action: PaletteAction::ToggleSidebar,
        },
        PaletteItem {
            title: "Agent Usage: Today (Hourly)",
            hint: "Stats",
            category: "AI",
            action: PaletteAction::OpenUsageToday,
        },
        PaletteItem {
            title: "Agent Usage: Last 30 days",
            hint: "Ctrl+U",
            category: "AI",
            action: PaletteAction::OpenUsage30d,
        },
        PaletteItem {
            title: "Antigravity Hub: 2.0 Conversations & SDK Chat",
            hint: "Ctrl+Shift+A",
            category: "AI",
            action: PaletteAction::OpenAgyHub,
        },
        PaletteItem {
            title: "Work Hub: Jira, Bitbucket & GitHub",
            hint: "Ctrl+J",
            category: "Work",
            action: PaletteAction::OpenWorkHub,
        },
        PaletteItem {
            title: "Options & Themes",
            hint: "Ctrl+,",
            category: "Config",
            action: PaletteAction::OpenSettings,
        },
        PaletteItem {
            title: "Find in Terminal",
            hint: "Ctrl+F",
            category: "Term",
            action: PaletteAction::FindInTerminal,
        },
        PaletteItem {
            title: "Run: git status",
            hint: "Enter",
            category: "Git",
            action: PaletteAction::RunCommand("git status\r"),
        },
        PaletteItem {
            title: "Run: git log --oneline -n 15",
            hint: "Enter",
            category: "Git",
            action: PaletteAction::RunCommand("git log --oneline -n 15\r"),
        },
        PaletteItem {
            title: "Run: clear",
            hint: "Enter",
            category: "Term",
            action: PaletteAction::RunCommand("clear\r"),
        },
    ];

    let q_lower = query.trim().to_lowercase();
    let mut list_col = column![].spacing(4);
    let mut match_count = 0usize;

    for item in items {
        if !q_lower.is_empty()
            && !item.title.to_lowercase().contains(&q_lower)
            && !item.category.to_lowercase().contains(&q_lower)
        {
            continue;
        }
        match_count += 1;
        let btn = button(
            row![
                container(text(item.category).size(10).color(theme::BLUE))
                    .padding([2, 6])
                    .style(theme::card_style),
                text(item.title).size(12).color(theme::TEXT_PRIMARY),
                Space::with_width(Length::Fill),
                text(item.hint).size(11).color(theme::TEXT_MUTED),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([8, 12])
        .style(theme::ghost_button_style(false))
        .on_press(on_run_action(item.action));

        list_col = list_col.push(btn);
    }

    if match_count == 0 {
        list_col = list_col.push(
            container(text("No matching commands").size(12).color(theme::TEXT_MUTED))
                .padding([12, 10]),
        );
    }

    let palette_card = container(
        column![
            row![
                text("Command Palette").size(14).color(theme::TEXT_PRIMARY),
                Space::with_width(Length::Fill),
                button(text("esc").size(11).color(theme::TEXT_MUTED))
                    .padding([4, 8])
                    .style(theme::card_button_style(false))
                    .on_press(on_close),
            ]
            .align_y(Alignment::Center),
            text_input("Type a command, shell, or layout...", query)
                .id(input_id)
                .size(13)
                .padding([9, 12])
                .style(theme::search_input_style)
                .on_input(on_query_change),
            scrollable(list_col)
                .height(340)
                .style(theme::dark_scrollable_style),
        ]
        .spacing(10)
        .padding(16),
    )
    .width(560)
    .style(theme::card_style);

    container(palette_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .padding(Padding {
            top: 48.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        })
        .style(theme::app_bg)
        .into()
}
