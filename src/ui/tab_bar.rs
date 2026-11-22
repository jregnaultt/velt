use crate::pty::{SessionId, SessionStatus, TerminalSession};
use crate::theme;
use iced::widget::{button, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length};

#[allow(clippy::too_many_arguments)]
pub fn view_tab_bar<'a, M: 'a + Clone>(
    sessions: &'a [TerminalSession],
    active_session: Option<SessionId>,
    sidebar_visible: bool,
    split_active: bool,
    show_usage: bool,
    show_settings: bool,
    show_palette: bool,
    show_work: bool,
    show_agy: bool,
    on_toggle_sidebar: M,
    on_select_session: fn(SessionId) -> M,
    on_close_session: fn(SessionId) -> M,
    on_new_session: M,
    on_toggle_split: M,
    on_toggle_agy: M,
    on_toggle_usage: M,
    on_toggle_work: M,
    on_toggle_settings: M,
    on_toggle_palette: M,
) -> Element<'a, M> {
    let sidebar_btn = button(text(if sidebar_visible { "[=]" } else { "[ ]" }).size(12))
        .padding([5, 8])
        .style(theme::ghost_button_style(sidebar_visible))
        .on_press(on_toggle_sidebar);

    let mut tabs_row = row![].spacing(4).align_y(Alignment::Center);
    let active_tabs: Vec<&TerminalSession> = sessions.iter().filter(|s| !s.archived).collect();
    let show_close = active_tabs.len() > 1;

    for session in active_tabs {
        let is_active = Some(session.id) == active_session
            && !show_usage
            && !show_settings
            && !show_palette
            && !show_work
            && !show_agy;
        let dot_color = match session.status() {
            SessionStatus::Working => theme::AMBER,
            SessionStatus::Waiting => theme::BLUE,
            SessionStatus::Done => theme::GREEN,
        };

        let title_str = if session.title.chars().count() > 22 {
            let truncated: String = session.title.chars().take(21).collect();
            format!("{truncated}...")
        } else {
            session.title.clone()
        };

        let tab_label = button(
            row![
                text("o").size(10).color(dot_color),
                text(title_str).size(12),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([5, 8])
        .style(theme::tab_button_style(is_active))
        .on_press(on_select_session(session.id));

        let tab_group = if show_close {
            let close_btn = button(text("x").size(12).color(theme::TEXT_MUTED))
                .padding([3, 6])
                .style(theme::ghost_button_style(false))
                .on_press(on_close_session(session.id));
            container(row![tab_label, close_btn].align_y(Alignment::Center))
                .style(theme::card_style)
        } else {
            container(row![tab_label].align_y(Alignment::Center)).style(theme::card_style)
        };

        tabs_row = tabs_row.push(tab_group);
    }

    let tabs_scroll = scrollable(tabs_row)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(0).scroller_width(0),
        ))
        .width(Length::Fill);

    let new_tab_btn = button(text("+").size(15))
        .padding([3, 9])
        .style(theme::ghost_button_style(false))
        .on_press(on_new_session);

    let cmd_btn = button(text("Cmd").size(12))
        .padding([5, 10])
        .style(theme::ghost_button_style(show_palette))
        .on_press(on_toggle_palette);

    let split_btn = button(text("|| Split").size(12))
        .padding([5, 10])
        .style(theme::ghost_button_style(split_active))
        .on_press(on_toggle_split);

    let agy_btn = button(text("AGY").size(12))
        .padding([5, 10])
        .style(theme::ghost_button_style(show_agy))
        .on_press(on_toggle_agy);

    let work_btn = button(text("Work").size(12))
        .padding([5, 10])
        .style(theme::ghost_button_style(show_work))
        .on_press(on_toggle_work);

    let usage_btn = button(text("Usage").size(12))
        .padding([5, 10])
        .style(theme::ghost_button_style(show_usage))
        .on_press(on_toggle_usage);

    let settings_btn = button(text("Options").size(12))
        .padding([5, 10])
        .style(theme::ghost_button_style(show_settings))
        .on_press(on_toggle_settings);

    container(
        row![
            sidebar_btn,
            tabs_scroll,
            new_tab_btn,
            Space::with_width(4),
            cmd_btn,
            split_btn,
            agy_btn,
            work_btn,
            usage_btn,
            settings_btn,
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .padding([0, 10]),
    )
    .width(Length::Fill)
    .height(38)
    .align_y(Alignment::Center)
    .style(theme::top_bar_style)
    .into()
}
