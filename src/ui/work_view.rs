use crate::config::IntegrationsConfig;
use crate::theme;
use crate::work::client::{JiraIssue, PullRequestItem, WorkData};
use crate::work::git_remote::GitRemote;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Color, Element, Length};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkTab {
    Jira,
    Bitbucket,
    GitHub,
    Connections,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JiraFilter {
    All,
    Todo,
    InProgress,
    Done,
}

#[derive(Debug, Clone)]
pub enum WorkMessage {
    SelectTab(WorkTab),
    SelectJiraFilter(JiraFilter),
    Refresh,
    StartJiraBranch(String, String),
    CheckoutPr(String),
    OpenUrl(String),
    SetJiraDomain(String),
    SetJiraEmail(String),
    SetJiraToken(String),
    SetJiraClientId(String),
    SetJiraClientSecret(String),
    SetBitbucketWorkspace(String),
    SetBitbucketUser(String),
    SetBitbucketPass(String),
    SetGithubToken(String),
    SetGithubUser(String),
    SetGithubClientId(String),
    SetGithubClientSecret(String),
    ImportGhCli,
    ImportGitGcmGithub,
    ImportGitGcmBitbucket,
    StartAtlassianOAuth,
    StartGithubOAuth,
    SaveAndSync,
    Close,
}

#[allow(clippy::too_many_arguments)]
pub fn view_work_hub<'a>(
    tab: WorkTab,
    jira_filter: JiraFilter,
    data: &'a WorkData,
    cfg: &'a IntegrationsConfig,
    detected_remote: Option<&'a GitRemote>,
    auth_status: Option<&'a str>,
    loading: bool,
) -> Element<'a, WorkMessage> {
    let remote_badge = match detected_remote {
        Some(r) => r.label(),
        None => "No Git remote in active folder".to_string(),
    };

    let header = row![
        column![
            text("Work Hub").size(18).color(theme::TEXT_PRIMARY),
            text(remote_badge).size(12).color(theme::BLUE),
        ]
        .spacing(3),
        Space::with_width(Length::Fill),
        button(text(if loading { "Syncing..." } else { "Refresh" }).size(12))
            .padding([6, 12])
            .style(theme::card_button_style(false))
            .on_press(WorkMessage::Refresh),
        Space::with_width(8),
        button(text("esc").size(12).color(theme::TEXT_MUTED))
            .padding([6, 10])
            .style(theme::card_button_style(false))
            .on_press(WorkMessage::Close),
    ]
    .align_y(Alignment::Center);

    let tabs_row = row![
        tab_pill(
            format!("Jira ({})", data.jira_issues.len()),
            tab == WorkTab::Jira,
            WorkMessage::SelectTab(WorkTab::Jira),
        ),
        tab_pill(
            format!("Bitbucket ({})", data.bitbucket_prs.len()),
            tab == WorkTab::Bitbucket,
            WorkMessage::SelectTab(WorkTab::Bitbucket),
        ),
        tab_pill(
            format!("GitHub ({})", data.github_prs.len()),
            tab == WorkTab::GitHub,
            WorkMessage::SelectTab(WorkTab::GitHub),
        ),
        tab_pill(
            "Connections".to_string(),
            tab == WorkTab::Connections,
            WorkMessage::SelectTab(WorkTab::Connections),
        ),
    ]
    .spacing(8);

    let mut sticky_col = column![header, tabs_row].spacing(12);
    if let Some(msg) = auth_status {
        let lower = msg.to_lowercase();
        let status_color = if lower.contains("error")
            || lower.contains("failed")
            || lower.contains("first")
            || lower.contains("not ")
            || lower.contains("no ")
            || lower.contains("timed out")
        {
            theme::AMBER
        } else if lower.contains("connected") || lower.contains("imported") {
            theme::GREEN
        } else {
            theme::BLUE
        };
        sticky_col = sticky_col.push(
            container(text(msg).size(12).color(status_color))
                .width(Length::Fill)
                .padding([8, 12])
                .style(theme::card_style),
        );
    }

    let sticky_top = container(sticky_col.padding([16, 24]))
        .width(Length::Fill)
        .style(theme::top_bar_style);

    let content: Element<'a, WorkMessage> = match tab {
        WorkTab::Jira => view_jira_tab(data, jira_filter),
        WorkTab::Bitbucket => view_prs_tab(
            "BITBUCKET PULL REQUESTS",
            &data.bitbucket_status,
            &data.bitbucket_prs,
            theme::BLUE,
        ),
        WorkTab::GitHub => view_prs_tab(
            "GITHUB PULL REQUESTS",
            &data.github_status,
            &data.github_prs,
            theme::GREEN,
        ),
        WorkTab::Connections => view_connections_tab(cfg),
    };

    let body = scrollable(container(content).padding([18, 24]))
        .height(Length::Fill)
        .style(theme::dark_scrollable_style);

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

fn tab_pill<'a>(label: String, active: bool, msg: WorkMessage) -> Element<'a, WorkMessage> {
    button(text(label).size(12))
        .padding([7, 14])
        .style(theme::card_button_style(active))
        .on_press(msg)
        .into()
}

fn view_jira_tab<'a>(data: &'a WorkData, filter: JiraFilter) -> Element<'a, WorkMessage> {
    let filter_row = row![
        text(&data.jira_status).size(12).color(theme::TEXT_MUTED),
        Space::with_width(Length::Fill),
        tab_pill(
            "All".to_string(),
            filter == JiraFilter::All,
            WorkMessage::SelectJiraFilter(JiraFilter::All),
        ),
        tab_pill(
            "To Do".to_string(),
            filter == JiraFilter::Todo,
            WorkMessage::SelectJiraFilter(JiraFilter::Todo),
        ),
        tab_pill(
            "In Progress".to_string(),
            filter == JiraFilter::InProgress,
            WorkMessage::SelectJiraFilter(JiraFilter::InProgress),
        ),
        tab_pill(
            "Done".to_string(),
            filter == JiraFilter::Done,
            WorkMessage::SelectJiraFilter(JiraFilter::Done),
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let mut list = column![filter_row].spacing(10);

    let filtered: Vec<&JiraIssue> = data
        .jira_issues
        .iter()
        .filter(|issue| match filter {
            JiraFilter::All => true,
            JiraFilter::Todo => issue.status_category == "new",
            JiraFilter::InProgress => issue.status_category == "indeterminate",
            JiraFilter::Done => issue.status_category == "done",
        })
        .collect();

    if filtered.is_empty() {
        list = list.push(
            container(
                column![
                    text("No Jira issues to show")
                        .size(13)
                        .color(theme::TEXT_PRIMARY),
                    text("Connect Jira via OAuth 2.0 or API Token in the Connections tab.")
                        .size(12)
                        .color(theme::TEXT_MUTED),
                ]
                .spacing(6)
                .padding(16),
            )
            .width(Length::Fill)
            .style(theme::card_style),
        );
    } else {
        for issue in filtered {
            let status_color = match issue.status_category.as_str() {
                "done" => theme::GREEN,
                "indeterminate" => theme::BLUE,
                _ => theme::AMBER,
            };
            let card = container(
                row![
                    column![
                        row![
                            container(text(&issue.key).size(11).color(theme::BLUE))
                                .padding([2, 7])
                                .style(theme::card_style),
                            container(text(&issue.status).size(11).color(status_color))
                                .padding([2, 7])
                                .style(theme::card_style),
                            text(&issue.issue_type)
                                .size(11)
                                .color(theme::TEXT_MUTED),
                            text(&issue.priority)
                                .size(11)
                                .color(theme::TEXT_MUTED),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                        text(&issue.summary)
                            .size(13)
                            .color(theme::TEXT_PRIMARY),
                    ]
                    .spacing(6)
                    .width(Length::Fill),
                    row![
                        button(text("Start Branch").size(11).color(theme::TEXT_PRIMARY))
                            .padding([6, 10])
                            .style(theme::card_button_style(true))
                            .on_press(WorkMessage::StartJiraBranch(
                                issue.key.clone(),
                                issue.summary.clone(),
                            )),
                        button(text("Open").size(11).color(theme::TEXT_MUTED))
                            .padding([6, 10])
                            .style(theme::card_button_style(false))
                            .on_press(WorkMessage::OpenUrl(issue.url.clone())),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding(14),
            )
            .width(Length::Fill)
            .style(theme::card_style);

            list = list.push(card);
        }
    }

    list.into()
}

fn view_prs_tab<'a>(
    heading: &'a str,
    status: &'a str,
    prs: &'a [PullRequestItem],
    accent: Color,
) -> Element<'a, WorkMessage> {
    let mut list = column![
        row![
            text(heading).size(11).color(theme::TEXT_MUTED),
            Space::with_width(Length::Fill),
            text(status).size(12).color(theme::TEXT_MUTED),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(10);

    if prs.is_empty() {
        list = list.push(
            container(
                column![
                    text("No open Pull Requests")
                        .size(13)
                        .color(theme::TEXT_PRIMARY),
                    text(status).size(12).color(theme::TEXT_MUTED),
                ]
                .spacing(6)
                .padding(16),
            )
            .width(Length::Fill)
            .style(theme::card_style),
        );
    } else {
        for pr in prs {
            let branch_info = if pr.source_branch.is_empty() {
                pr.repo.clone()
            } else {
                format!("{}  |  {} -> {}", pr.repo, pr.source_branch, pr.target_branch)
            };

            let mut actions = row![].spacing(6).align_y(Alignment::Center);
            if !pr.source_branch.is_empty() {
                actions = actions.push(
                    button(text("Checkout PR").size(11).color(theme::TEXT_PRIMARY))
                        .padding([6, 10])
                        .style(theme::card_button_style(true))
                        .on_press(WorkMessage::CheckoutPr(pr.source_branch.clone())),
                );
            }
            if !pr.url.is_empty() {
                actions = actions.push(
                    button(text("Open").size(11).color(theme::TEXT_MUTED))
                        .padding([6, 10])
                        .style(theme::card_button_style(false))
                        .on_press(WorkMessage::OpenUrl(pr.url.clone())),
                );
            }

            let card = container(
                row![
                    column![
                        row![
                            container(text(format!("#{}", pr.id)).size(11).color(accent))
                                .padding([2, 7])
                                .style(theme::card_style),
                            text(&pr.state).size(11).color(theme::GREEN),
                            text(&pr.author).size(11).color(theme::TEXT_MUTED),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                        text(&pr.title).size(13).color(theme::TEXT_PRIMARY),
                        text(branch_info).size(11).color(theme::TEXT_MUTED),
                    ]
                    .spacing(5)
                    .width(Length::Fill),
                    actions,
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding(14),
            )
            .width(Length::Fill)
            .style(theme::card_style);

            list = list.push(card);
        }
    }

    list.into()
}

fn view_connections_tab<'a>(cfg: &'a IntegrationsConfig) -> Element<'a, WorkMessage> {
    let mut col = column![
        text("Use 1-click auto-import (gh CLI / Git Credential Manager), Browser OAuth 2.0, or API tokens — saved locally in ~/.velt/config.json.")
            .size(12)
            .color(theme::TEXT_MUTED),
    ]
    .spacing(14);

    // GitHub Card
    let github_card = container(
        column![
            row![
                text("GITHUB").size(11).color(theme::GREEN),
                Space::with_width(Length::Fill),
                button(text("Import from gh CLI").size(11))
                    .padding([5, 10])
                    .style(theme::card_button_style(true))
                    .on_press(WorkMessage::ImportGhCli),
                button(text("Import from Git (GCM)").size(11))
                    .padding([5, 10])
                    .style(theme::card_button_style(false))
                    .on_press(WorkMessage::ImportGitGcmGithub),
                button(text("Open Tokens Page").size(11).color(theme::TEXT_MUTED))
                    .padding([5, 10])
                    .style(theme::card_button_style(false))
                    .on_press(WorkMessage::OpenUrl(
                        "https://github.com/settings/tokens/new".to_string(),
                    )),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            row![
                labeled_input(
                    "GitHub Username (optional)",
                    "Regnault",
                    &cfg.github_username,
                    false,
                    WorkMessage::SetGithubUser,
                ),
                labeled_input(
                    "Personal Access Token (auto-filled by Import)",
                    "ghp_... or gho_...",
                    &cfg.github_token,
                    true,
                    WorkMessage::SetGithubToken,
                ),
            ]
            .spacing(10),
            row![
                labeled_input(
                    "OAuth 2.0 Client ID (callback: http://127.0.0.1:48272/callback)",
                    "Ov23li...",
                    &cfg.github_oauth_client_id,
                    false,
                    WorkMessage::SetGithubClientId,
                ),
                labeled_input(
                    "OAuth 2.0 Client Secret",
                    "secret...",
                    &cfg.github_oauth_client_secret,
                    true,
                    WorkMessage::SetGithubClientSecret,
                ),
            ]
            .spacing(10),
            row![
                Space::with_width(Length::Fill),
                button(text("Sign in with GitHub (OAuth 2.0)").size(11))
                    .padding([6, 12])
                    .style(theme::card_button_style(false))
                    .on_press(WorkMessage::StartGithubOAuth),
            ],
        ]
        .spacing(10)
        .padding(16),
    )
    .width(Length::Fill)
    .style(theme::card_style);

    // Bitbucket Card
    let bitbucket_card = container(
        column![
            row![
                text("BITBUCKET CLOUD").size(11).color(theme::BLUE),
                Space::with_width(Length::Fill),
                button(text("Import from Git (GCM)").size(11))
                    .padding([5, 10])
                    .style(theme::card_button_style(true))
                    .on_press(WorkMessage::ImportGitGcmBitbucket),
                button(text("Open App Passwords").size(11).color(theme::TEXT_MUTED))
                    .padding([5, 10])
                    .style(theme::card_button_style(false))
                    .on_press(WorkMessage::OpenUrl(
                        "https://bitbucket.org/account/settings/app-passwords/".to_string(),
                    )),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            row![
                labeled_input(
                    "Workspace slug (optional if inside repo)",
                    "my-workspace",
                    &cfg.bitbucket_workspace,
                    false,
                    WorkMessage::SetBitbucketWorkspace,
                ),
                labeled_input(
                    "Bitbucket Username",
                    "your-username",
                    &cfg.bitbucket_username,
                    false,
                    WorkMessage::SetBitbucketUser,
                ),
                labeled_input(
                    "App Password / OAuth Token",
                    "ATBB...",
                    &cfg.bitbucket_app_password,
                    true,
                    WorkMessage::SetBitbucketPass,
                ),
            ]
            .spacing(10),
        ]
        .spacing(10)
        .padding(16),
    )
    .width(Length::Fill)
    .style(theme::card_style);

    // Jira Card
    let jira_oauth_badge = if !cfg.jira_oauth_access_token.trim().is_empty()
        && !cfg.jira_cloud_id.trim().is_empty()
    {
        "Connected via OAuth 2.0"
    } else {
        "OAuth 2.0 or API Token"
    };

    let jira_card = container(
        column![
            row![
                text("JIRA (ATLASSIAN CLOUD)").size(11).color(theme::BLUE),
                Space::with_width(Length::Fill),
                text(jira_oauth_badge).size(11).color(theme::GREEN),
                Space::with_width(8),
                button(text("Open API Tokens").size(11).color(theme::TEXT_MUTED))
                    .padding([5, 10])
                    .style(theme::card_button_style(false))
                    .on_press(WorkMessage::OpenUrl(
                        "https://id.atlassian.com/manage-profile/security/api-tokens".to_string(),
                    )),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            row![
                labeled_input(
                    "OAuth 2.0 Client ID (callback: http://127.0.0.1:48271/callback)",
                    "Atlassian OAuth Client ID",
                    &cfg.jira_oauth_client_id,
                    false,
                    WorkMessage::SetJiraClientId,
                ),
                labeled_input(
                    "OAuth 2.0 Client Secret",
                    "Atlassian OAuth Secret",
                    &cfg.jira_oauth_client_secret,
                    true,
                    WorkMessage::SetJiraClientSecret,
                ),
            ]
            .spacing(10),
            row![
                button(text("Sign in with Atlassian (OAuth 2.0)").size(11))
                    .padding([6, 12])
                    .style(theme::card_button_style(true))
                    .on_press(WorkMessage::StartAtlassianOAuth),
                button(text("Open Atlassian Dev Console").size(11).color(theme::TEXT_MUTED))
                    .padding([6, 10])
                    .style(theme::card_button_style(false))
                    .on_press(WorkMessage::OpenUrl(
                        "https://developer.atlassian.com/console/myapps/".to_string(),
                    )),
            ]
            .spacing(8),
            text("Or connect directly with API Token:")
                .size(11)
                .color(theme::TEXT_MUTED),
            row![
                labeled_input(
                    "Atlassian Domain",
                    "your-company.atlassian.net",
                    &cfg.jira_domain,
                    false,
                    WorkMessage::SetJiraDomain,
                ),
                labeled_input(
                    "Atlassian Email",
                    "you@company.com",
                    &cfg.jira_email,
                    false,
                    WorkMessage::SetJiraEmail,
                ),
                labeled_input(
                    "Jira API Token",
                    "ATATT3x...",
                    &cfg.jira_api_token,
                    true,
                    WorkMessage::SetJiraToken,
                ),
            ]
            .spacing(10),
        ]
        .spacing(10)
        .padding(16),
    )
    .width(Length::Fill)
    .style(theme::card_style);

    col = col
        .push(github_card)
        .push(bitbucket_card)
        .push(jira_card)
        .push(row![
            Space::with_width(Length::Fill),
            button(
                text("Save & Sync All")
                    .size(12)
                    .color(theme::TEXT_PRIMARY),
            )
            .padding([9, 18])
            .style(theme::card_button_style(true))
            .on_press(WorkMessage::SaveAndSync),
        ]);

    col.into()
}

fn labeled_input<'a>(
    label: &'a str,
    placeholder: &'a str,
    value: &'a str,
    secure: bool,
    on_input: fn(String) -> WorkMessage,
) -> Element<'a, WorkMessage> {
    let mut input = text_input(placeholder, value)
        .size(12)
        .padding([8, 10])
        .style(theme::search_input_style)
        .on_input(on_input);
    if secure {
        input = input.secure(true);
    }
    column![text(label).size(11).color(theme::TEXT_MUTED), input]
        .width(Length::Fill)
        .spacing(4)
        .into()
}
