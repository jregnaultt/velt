use crate::ai::scanner::DetectedAgent;
use crate::pty::{SessionId, SessionStatus, TerminalSession};
use crate::theme;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarFilter {
    All,
    Waiting,
    Working,
}

#[allow(clippy::too_many_arguments)]
pub fn view_sidebar<'a, M: 'a + Clone>(
    sessions: &'a [TerminalSession],
    active_session: Option<SessionId>,
    filter: SidebarFilter,
    search_query: &'a str,
    search_input_id: text_input::Id,
    detected_agents: &'a [DetectedAgent],
    on_new_session: M,
    on_search_change: fn(String) -> M,
    on_filter_change: fn(SidebarFilter) -> M,
    on_select_session: fn(SessionId) -> M,
    on_open_usage: M,
) -> Element<'a, M> {
    let new_btn = button(
        row![
            text("+ New session").size(12).color(theme::TEXT_PRIMARY),
            Space::with_width(Length::Fill),
            text("Ctrl+T").size(11).color(theme::TEXT_MUTED),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([8, 10])
    .style(theme::card_button_style(false))
    .on_press(on_new_session);

    let search_box = text_input("Search agents   /", search_query)
        .id(search_input_id)
        .size(12)
        .padding([7, 10])
        .style(theme::search_input_style)
        .on_input(on_search_change);

    let active_sessions: Vec<&TerminalSession> =
        sessions.iter().filter(|s| !s.archived).collect();
    let total_count = active_sessions.len();
    let working_count = active_sessions
        .iter()
        .filter(|s| s.status() == SessionStatus::Working)
        .count();
    let archived_count = sessions.iter().filter(|s| s.archived).count();

    let filters_row = row![
        button(text(format!("All {total_count}")).size(11))
            .padding([4, 9])
            .style(theme::filter_pill_style(filter == SidebarFilter::All))
            .on_press(on_filter_change(SidebarFilter::All)),
        button(text("Waiting").size(11))
            .padding([4, 9])
            .style(theme::filter_pill_style(filter == SidebarFilter::Waiting))
            .on_press(on_filter_change(SidebarFilter::Waiting)),
        button(text(format!("Working {working_count}")).size(11))
            .padding([4, 9])
            .style(theme::filter_pill_style(filter == SidebarFilter::Working))
            .on_press(on_filter_change(SidebarFilter::Working)),
    ]
    .spacing(6);

    let query_lower = search_query.trim().to_lowercase();
    let mut grouped: BTreeMap<String, Vec<&TerminalSession>> = BTreeMap::new();

    for s in active_sessions {
        let st = s.status();
        let filter_ok = match filter {
            SidebarFilter::All => true,
            SidebarFilter::Waiting => st == SessionStatus::Waiting,
            SidebarFilter::Working => st == SessionStatus::Working,
        };
        if !filter_ok {
            continue;
        }
        if !query_lower.is_empty() {
            let matches_title = s.title.to_lowercase().contains(&query_lower);
            let matches_proj = s.project_name.to_lowercase().contains(&query_lower);
            if !matches_title && !matches_proj {
                continue;
            }
        }
        grouped
            .entry(s.project_name.clone())
            .or_default()
            .push(s);
    }

    let mut list_col = column![].spacing(10);

    if grouped.is_empty() {
        list_col = list_col.push(
            container(text("No matching sessions").size(12).color(theme::TEXT_MUTED))
                .padding([12, 6]),
        );
    } else {
        for (project, items) in grouped {
            let mut group_col = column![
                text(project).size(11).color(theme::TEXT_MUTED),
            ]
            .spacing(4);

            for session in items {
                let is_selected = Some(session.id) == active_session;
                let st = session.status();
                let (dot_color, status_text, status_color) = match st {
                    SessionStatus::Working => (theme::AMBER, "Working", theme::TEXT_MUTED),
                    SessionStatus::Waiting => (theme::BLUE, "Waiting", theme::TEXT_MUTED),
                    SessionStatus::Done => (theme::GREEN, "Done", theme::GREEN),
                };

                let row_btn = button(
                    column![
                        row![
                            text("o").size(9).color(dot_color),
                            text(&session.title).size(12).color(theme::TEXT_PRIMARY),
                            Space::with_width(Length::Fill),
                            text(session.elapsed_label())
                                .size(11)
                                .color(theme::TEXT_MUTED),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                        row![
                            Space::with_width(14),
                            text(status_text).size(11).color(status_color),
                        ],
                    ]
                    .spacing(3),
                )
                .width(Length::Fill)
                .padding([7, 8])
                .style(theme::ghost_button_style(is_selected))
                .on_press(on_select_session(session.id));

                group_col = group_col.push(row_btn);
            }

            list_col = list_col.push(group_col);
        }
    }

    let sessions_scroll = scrollable(list_col)
        .height(Length::Fill)
        .style(theme::dark_scrollable_style);

    // Detected local AI tools strip
    let installed_ais: Vec<&DetectedAgent> =
        detected_agents.iter().filter(|a| a.installed).collect();

    let mut ai_badges = row![].spacing(6).align_y(Alignment::Center);
    if installed_ais.is_empty() {
        ai_badges = ai_badges.push(text("Scanning AIs...").size(11).color(theme::TEXT_MUTED));
    } else {
        for agent in installed_ais.iter().take(4) {
            let color = match agent.short_name {
                "AGY" => theme::BLUE,
                "Codex" => theme::GREEN,
                "Claude" => theme::ORANGE,
                _ => theme::AMBER,
            };
            ai_badges = ai_badges.push(
                container(
                    row![
                        text("o").size(8).color(color),
                        text(agent.short_name).size(10).color(theme::TEXT_PRIMARY),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                )
                .padding([3, 7])
                .style(theme::card_style),
            );
        }
    }

    let ai_strip = button(
        column![
            row![
                text("AI Agents").size(11).color(theme::TEXT_MUTED),
                Space::with_width(Length::Fill),
                text(format!("{} found", installed_ais.len()))
                    .size(10)
                    .color(theme::GREEN),
            ]
            .align_y(Alignment::Center),
            ai_badges,
        ]
        .spacing(6),
    )
    .width(Length::Fill)
    .padding([6, 8])
    .style(theme::ghost_button_style(false))
    .on_press(on_open_usage);

    let footer = container(
        row![
            text("Archived").size(12).color(theme::TEXT_MUTED),
            Space::with_width(Length::Fill),
            text(format!("{archived_count}"))
                .size(12)
                .color(theme::TEXT_MUTED),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([6, 8]);

    container(
        column![
            new_btn,
            search_box,
            filters_row,
            container(Space::with_height(1))
                .width(Length::Fill)
                .style(theme::divider_style),
            sessions_scroll,
            container(Space::with_height(1))
                .width(Length::Fill)
                .style(theme::divider_style),
            ai_strip,
            footer,
        ]
        .spacing(10)
        .padding([12, 10]),
    )
    .width(240)
    .height(Length::Fill)
    .style(theme::sidebar_style)
    .into()
}
