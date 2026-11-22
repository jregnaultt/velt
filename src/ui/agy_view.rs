use crate::ai::antigravity::{ChatMessageItem, ChatRole, ConversationSummary, SdkStatus};
use crate::theme;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgyTab {
    Conversations,
    NativeChat,
}

#[derive(Debug, Clone)]
pub enum AgyMessage {
    SelectTab(AgyTab),
    SearchChanged(String),
    SelectConversation(String),
    OpenProjectTerminal(PathBuf),
    Refresh,
    ChatInputChanged(String),
    SendChat,
    InstallSdk,
    ClearChat,
    Close,
}

#[allow(clippy::too_many_arguments)]
pub fn view_agy_hub<'a>(
    tab: AgyTab,
    conversations: &'a [ConversationSummary],
    selected_conv_id: Option<&'a str>,
    conv_messages: &'a [ChatMessageItem],
    search_query: &'a str,
    sdk_status: &'a SdkStatus,
    chat_messages: &'a [ChatMessageItem],
    chat_input: &'a str,
    chat_input_id: text_input::Id,
    chat_busy: bool,
    loading: bool,
) -> Element<'a, AgyMessage> {
    let header = row![
        column![
            text("Antigravity Hub").size(18).color(theme::TEXT_PRIMARY),
            text("Antigravity 2.0 live sessions & Native SDK Chat")
                .size(12)
                .color(theme::TEXT_MUTED),
        ]
        .spacing(3),
        Space::with_width(Length::Fill),
        button(text(if loading { "Scanning..." } else { "Refresh" }).size(12))
            .padding([6, 12])
            .style(theme::card_button_style(false))
            .on_press(AgyMessage::Refresh),
        Space::with_width(8),
        button(text("esc").size(12).color(theme::TEXT_MUTED))
            .padding([6, 10])
            .style(theme::card_button_style(false))
            .on_press(AgyMessage::Close),
    ]
    .align_y(Alignment::Center);

    let tabs_row = row![
        button(text(format!("2.0 Conversations ({})", conversations.len())).size(12))
            .padding([7, 14])
            .style(theme::card_button_style(tab == AgyTab::Conversations))
            .on_press(AgyMessage::SelectTab(AgyTab::Conversations)),
        button(text("Native Chat (SDK)").size(12))
            .padding([7, 14])
            .style(theme::card_button_style(tab == AgyTab::NativeChat))
            .on_press(AgyMessage::SelectTab(AgyTab::NativeChat)),
    ]
    .spacing(8);

    let sticky_top = container(
        column![header, tabs_row]
            .spacing(12)
            .padding([16, 24]),
    )
    .width(Length::Fill)
    .style(theme::top_bar_style);

    let body: Element<'a, AgyMessage> = match tab {
        AgyTab::Conversations => view_conversations_tab(
            conversations,
            selected_conv_id,
            conv_messages,
            search_query,
        ),
        AgyTab::NativeChat => view_native_chat_tab(
            sdk_status,
            chat_messages,
            chat_input,
            chat_input_id,
            chat_busy,
        ),
    };

    container(
        column![sticky_top, body]
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(theme::app_bg)
    .into()
}

fn view_conversations_tab<'a>(
    conversations: &'a [ConversationSummary],
    selected_conv_id: Option<&'a str>,
    conv_messages: &'a [ChatMessageItem],
    search_query: &'a str,
) -> Element<'a, AgyMessage> {
    let q_lower = search_query.trim().to_lowercase();
    let mut left_list = column![].spacing(6);

    let mut filtered_count = 0usize;
    for conv in conversations {
        if !q_lower.is_empty()
            && !conv.title.to_lowercase().contains(&q_lower)
            && !conv.last_model.to_lowercase().contains(&q_lower)
        {
            continue;
        }
        filtered_count += 1;
        let is_selected = selected_conv_id == Some(conv.id.as_str());
        let dot_color = if conv.is_live {
            theme::AMBER
        } else {
            theme::BLUE
        };
        let status_label = if conv.is_live { "Live" } else { "Saved" };

        let item_btn = button(
            column![
                row![
                    text("o").size(8).color(dot_color),
                    text(&conv.title).size(12).color(theme::TEXT_PRIMARY),
                    Space::with_width(Length::Fill),
                    text(&conv.last_updated_label)
                        .size(10)
                        .color(theme::TEXT_MUTED),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
                row![
                    text(status_label).size(10).color(dot_color),
                    text(format!("{} turns", conv.turn_count))
                        .size(10)
                        .color(theme::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    text(&conv.last_model).size(10).color(theme::TEXT_MUTED),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(4),
        )
        .width(Length::Fill)
        .padding([8, 10])
        .style(theme::card_button_style(is_selected))
        .on_press(AgyMessage::SelectConversation(conv.id.clone()));

        left_list = left_list.push(item_btn);
    }

    if filtered_count == 0 {
        left_list = left_list.push(
            container(
                text("No Antigravity 2.0 conversations match.")
                    .size(12)
                    .color(theme::TEXT_MUTED),
            )
            .padding(12),
        );
    }

    let left_panel = container(
        column![
            text_input("Filter conversations...", search_query)
                .size(12)
                .padding([8, 10])
                .style(theme::search_input_style)
                .on_input(AgyMessage::SearchChanged),
            scrollable(left_list)
                .height(Length::Fill)
                .style(theme::dark_scrollable_style),
        ]
        .spacing(10)
        .padding(14),
    )
    .width(290)
    .height(Length::Fill)
    .style(theme::sidebar_style);

    let selected_summary = selected_conv_id
        .and_then(|id| conversations.iter().find(|c| c.id == id))
        .or_else(|| conversations.first());

    let right_panel: Element<'a, AgyMessage> = if let Some(summary) = selected_summary {
        let mut header_bar = row![
            column![
                text(&summary.title).size(14).color(theme::TEXT_PRIMARY),
                text(format!(
                    "{} · {} turns · {}",
                    summary.last_model, summary.turn_count, summary.last_updated_label
                ))
                .size(11)
                .color(theme::TEXT_MUTED),
            ]
            .spacing(3),
            Space::with_width(Length::Fill),
        ]
        .align_y(Alignment::Center);

        if let Some(proj_path) = &summary.workspace_hint {
            header_bar = header_bar.push(
                button(
                    text("Open Terminal in Project")
                        .size(11)
                        .color(theme::TEXT_PRIMARY),
                )
                .padding([6, 12])
                .style(theme::card_button_style(true))
                .on_press(AgyMessage::OpenProjectTerminal(proj_path.clone())),
            );
        }

        let mut timeline = column![].spacing(10);
        for msg in conv_messages {
            timeline = timeline.push(view_chat_card(msg));
        }

        column![
            container(header_bar.padding([12, 18]))
                .width(Length::Fill)
                .style(theme::top_bar_style),
            scrollable(container(timeline).padding([16, 20]))
                .height(Length::Fill)
                .style(theme::dark_scrollable_style),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        container(
            text("Select an Antigravity 2.0 conversation on the left to inspect it.")
                .size(13)
                .color(theme::TEXT_MUTED),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    };

    row![left_panel, right_panel]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_native_chat_tab<'a>(
    sdk_status: &'a SdkStatus,
    chat_messages: &'a [ChatMessageItem],
    chat_input: &'a str,
    chat_input_id: text_input::Id,
    chat_busy: bool,
) -> Element<'a, AgyMessage> {
    let status_banner: Element<'a, AgyMessage> = match sdk_status {
        SdkStatus::Ready(py) => container(
            row![
                text("o").size(9).color(theme::GREEN),
                text(format!("Antigravity Python SDK ready ({py})"))
                    .size(12)
                    .color(theme::TEXT_PRIMARY),
                Space::with_width(Length::Fill),
                button(text("Clear Chat").size(11).color(theme::TEXT_MUTED))
                    .padding([5, 10])
                    .style(theme::card_button_style(false))
                    .on_press(AgyMessage::ClearChat),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .padding([10, 14]),
        )
        .width(Length::Fill)
        .style(theme::card_style)
        .into(),
        SdkStatus::MissingSdk(py) => container(
            row![
                text("o").size(9).color(theme::AMBER),
                text(format!(
                    "Python ({py}) detected, but `google-antigravity` SDK is not installed yet."
                ))
                .size(12)
                .color(theme::TEXT_PRIMARY),
                Space::with_width(Length::Fill),
                button(text("Install SDK in Terminal").size(11).color(theme::TEXT_PRIMARY))
                    .padding([6, 12])
                    .style(theme::card_button_style(true))
                    .on_press(AgyMessage::InstallSdk),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .padding([10, 14]),
        )
        .width(Length::Fill)
        .style(theme::card_style)
        .into(),
        SdkStatus::MissingPython => container(
            row![
                text("o").size(9).color(theme::RED),
                text("Python 3 was not found in PATH. Install Python 3 to use the Antigravity SDK chat.")
                    .size(12)
                    .color(theme::TEXT_PRIMARY),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .padding([10, 14]),
        )
        .width(Length::Fill)
        .style(theme::card_style)
        .into(),
    };

    let mut feed = column![].spacing(10);
    if chat_messages.is_empty() {
        feed = feed.push(
            container(
                column![
                    text("Native Antigravity Chat inside Velt")
                        .size(14)
                        .color(theme::TEXT_PRIMARY),
                    text("Powered by the official `google-antigravity` SDK while keeping your Antigravity 2.0 desktop sessions untouched.")
                        .size(12)
                        .color(theme::TEXT_MUTED),
                ]
                .spacing(6)
                .padding(18),
            )
            .width(Length::Fill)
            .style(theme::card_style),
        );
    } else {
        for msg in chat_messages {
            feed = feed.push(view_chat_card(msg));
        }
    }

    let send_label = if chat_busy { "Thinking..." } else { "Send" };
    let mut prompt_input = text_input("Ask Antigravity (runs in current workspace)...", chat_input)
        .id(chat_input_id)
        .size(13)
        .padding([10, 12])
        .style(theme::search_input_style)
        .on_input(AgyMessage::ChatInputChanged);
    if !chat_busy {
        prompt_input = prompt_input.on_submit(AgyMessage::SendChat);
    }

    let mut send_btn = button(text(send_label).size(12).color(theme::TEXT_PRIMARY))
        .padding([10, 18])
        .style(theme::card_button_style(true));
    if !chat_busy {
        send_btn = send_btn.on_press(AgyMessage::SendChat);
    }

    column![
        container(status_banner).padding([14, 24]),
        scrollable(container(feed).padding([4, 24]))
            .height(Length::Fill)
            .style(theme::dark_scrollable_style),
        container(
            row![prompt_input, send_btn]
                .spacing(10)
                .align_y(Alignment::Center)
                .padding([12, 24]),
        )
        .width(Length::Fill)
        .style(theme::top_bar_style),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn view_chat_card<'a>(msg: &'a ChatMessageItem) -> Element<'a, AgyMessage> {
    let (badge, accent) = match msg.role {
        ChatRole::User => ("You", theme::GREEN),
        ChatRole::Assistant => (
            msg.model.as_deref().unwrap_or("Antigravity"),
            theme::BLUE,
        ),
    };

    let mut card_col = column![
        row![
            text("o").size(8).color(accent),
            text(badge).size(11).color(accent),
            Space::with_width(Length::Fill),
            text(&msg.timestamp).size(10).color(theme::TEXT_MUTED),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(6);

    if !msg.tools.is_empty() {
        let mut tools_row = row![].spacing(6);
        for t in msg.tools.iter().take(6) {
            tools_row = tools_row.push(
                container(text(t).size(10).color(theme::AMBER))
                    .padding([2, 6])
                    .style(theme::card_style),
            );
        }
        card_col = card_col.push(tools_row);
    }

    if let Some(th) = &msg.thinking {
        card_col = card_col.push(
            container(text(th).size(11).color(theme::TEXT_MUTED))
                .padding([6, 10])
                .width(Length::Fill)
                .style(theme::app_bg),
        );
    }

    if !msg.content.is_empty() {
        card_col = card_col.push(text(&msg.content).size(12).color(theme::TEXT_PRIMARY));
    }

    container(card_col.padding(14))
        .width(Length::Fill)
        .style(theme::card_style)
        .into()
}
