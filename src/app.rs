use crate::ai::antigravity::{
    check_sdk_status, list_antigravity_conversations, load_conversation_messages,
    send_sdk_chat_prompt, ChatMessageItem, ChatRole, ConversationSummary, SdkStatus,
};
use crate::ai::scanner::{scan_installed_agents, DetectedAgent};
use crate::ai::usage::{load_all_usage_events, UsageEvent, UsageRange, UsageReport};
use crate::config::{AppConfig, ShellKind};
use crate::pty::{SessionId, TerminalSession};
use crate::terminal_view::{view_terminal_pane, SearchOverlay, TerminalMessage};
use crate::theme::{self, detect_terminal_font};
use crate::ui::agy_view::{view_agy_hub, AgyMessage, AgyTab};
use crate::ui::palette_view::{view_palette, PaletteAction};
use crate::ui::settings_view::{view_settings, SettingsMessage};
use crate::ui::sidebar::{view_sidebar, SidebarFilter};
use crate::ui::tab_bar::view_tab_bar;
use crate::ui::usage_view::view_usage_dashboard;
use crate::ui::work_view::{view_work_hub, JiraFilter, WorkMessage, WorkTab};
use crate::work::auth::{
    import_git_credential_manager, import_github_cli_token, run_atlassian_oauth, run_github_oauth,
};
use crate::work::client::{fetch_all_work_data, open_in_browser, slugify_branch, WorkData};
use crate::work::git_remote::{detect_git_remote, GitRemote};
use iced::clipboard;
use iced::event::{self, Status};
use iced::keyboard::{self, key::Named, Key, Modifiers};
use iced::widget::canvas::Cache;
use iced::widget::{column, container, row, text, text_input, Space};
use iced::{Element, Event, Font, Length, Subscription, Task, Theme};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveOverlay {
    None,
    Usage,
    Settings,
    Palette,
    Work,
    Agy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    NewSession,
    SelectSession(SessionId),
    CloseSession(SessionId),
    ToggleSidebar,
    ToggleSplit,
    ToggleUsage,
    ToggleSettings,
    TogglePalette,
    ToggleWork,
    ToggleAgy,
    CloseOverlay,
    UsageRangeChanged(UsageRange),
    ToggleAgentFilter(String),
    RefreshAiData,
    AiDataLoaded(Vec<DetectedAgent>, Vec<UsageEvent>),
    Agy(AgyMessage),
    AgyConversationsLoaded(Vec<ConversationSummary>, SdkStatus),
    AgyMessagesLoaded(String, Vec<ChatMessageItem>),
    AgySdkReply(Result<ChatMessageItem, String>),
    Work(WorkMessage),
    WorkDataLoaded(Option<GitRemote>, WorkData),
    AuthFlowCompleted(AuthOutcome),
    Settings(SettingsMessage),
    FilterChanged(SidebarFilter),
    SearchChanged(String),
    PaletteQueryChanged(String),
    RunPaletteAction(PaletteAction),
    Terminal(TerminalMessage),
    JumpToBottom(usize),
    ModifiersChanged(Modifiers),
    ToggleTermSearch,
    TermSearchChanged(String),
    TermSearchNext,
    TermSearchPrev,
    CloseTermSearch,
    KeyboardEvent(Event),
    PasteClipboard(Option<String>),
}

#[derive(Debug, Clone)]
pub enum AuthOutcome {
    GithubToken(String, String),
    BitbucketCreds(String, String),
    AtlassianOAuth {
        access_token: String,
        cloud_id: String,
        domain: String,
    },
    Error(String),
}

pub struct TerminalPlus {
    config: AppConfig,
    terminal_font: Font,
    sessions: Vec<TerminalSession>,
    next_session_id: u64,
    panes: Vec<SessionId>,
    split_direction: SplitDirection,
    focused_pane: usize,
    ctrl_held: bool,
    term_search_open: bool,
    term_search_query: String,
    term_search_input_id: text_input::Id,
    term_search_index: usize,
    term_search_matches: Vec<(usize, usize, usize)>,
    pane_selections: HashMap<usize, ((usize, usize), (usize, usize))>,
    status_toast: Option<(String, Instant)>,
    sidebar_visible: bool,
    sidebar_filter: SidebarFilter,
    search_query: String,
    search_input_id: text_input::Id,
    palette_query: String,
    palette_input_id: text_input::Id,
    overlay: ActiveOverlay,
    usage_range: UsageRange,
    selected_agent: Option<String>,
    detected_agents: Vec<DetectedAgent>,
    usage_events: Vec<UsageEvent>,
    usage_report: UsageReport,
    ai_loading: bool,
    work_tab: WorkTab,
    jira_filter: JiraFilter,
    detected_remote: Option<GitRemote>,
    work_data: WorkData,
    work_loading: bool,
    auth_status: Option<String>,
    agy_tab: AgyTab,
    agy_conversations: Vec<ConversationSummary>,
    agy_selected_conv: Option<String>,
    agy_conv_messages: Vec<ChatMessageItem>,
    agy_search: String,
    agy_sdk_status: SdkStatus,
    agy_chat_messages: Vec<ChatMessageItem>,
    agy_chat_input: String,
    agy_chat_input_id: text_input::Id,
    agy_chat_busy: bool,
    agy_loading: bool,
    canvas_caches: Vec<Cache>,
    last_dirty_versions: HashMap<SessionId, u64>,
    last_context_refresh: Instant,
    default_cwd: PathBuf,
}

impl TerminalPlus {
    pub fn new() -> (Self, Task<Message>) {
        let config = AppConfig::load();
        let default_cwd = std::env::current_dir()
            .ok()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));

        let initial_report = UsageReport::build(&[], &[], UsageRange::Days30, None);

        let initial_remote = detect_git_remote(&default_cwd);
        let initial_tab = match &initial_remote {
            Some(GitRemote::GitHub { .. }) => WorkTab::GitHub,
            Some(GitRemote::Bitbucket { .. }) => WorkTab::Bitbucket,
            _ => WorkTab::Jira,
        };

        let mut app = Self {
            config,
            terminal_font: detect_terminal_font(),
            sessions: Vec::new(),
            next_session_id: 1,
            panes: Vec::new(),
            split_direction: SplitDirection::Horizontal,
            focused_pane: 0,
            ctrl_held: false,
            term_search_open: false,
            term_search_query: String::new(),
            term_search_input_id: text_input::Id::unique(),
            term_search_index: 0,
            term_search_matches: Vec::new(),
            pane_selections: HashMap::new(),
            status_toast: None,
            sidebar_visible: true,
            sidebar_filter: SidebarFilter::All,
            search_query: String::new(),
            search_input_id: text_input::Id::unique(),
            palette_query: String::new(),
            palette_input_id: text_input::Id::unique(),
            overlay: ActiveOverlay::None,
            usage_range: UsageRange::Days30,
            selected_agent: None,
            detected_agents: Vec::new(),
            usage_events: Vec::new(),
            usage_report: initial_report,
            ai_loading: true,
            work_tab: initial_tab,
            jira_filter: JiraFilter::All,
            detected_remote: initial_remote,
            work_data: WorkData::default(),
            work_loading: false,
            auth_status: None,
            agy_tab: AgyTab::Conversations,
            agy_conversations: Vec::new(),
            agy_selected_conv: None,
            agy_conv_messages: Vec::new(),
            agy_search: String::new(),
            agy_sdk_status: SdkStatus::MissingPython,
            agy_chat_messages: Vec::new(),
            agy_chat_input: String::new(),
            agy_chat_input_id: text_input::Id::unique(),
            agy_chat_busy: false,
            agy_loading: false,
            canvas_caches: vec![Cache::new(), Cache::new()],
            last_dirty_versions: HashMap::new(),
            last_context_refresh: Instant::now(),
            default_cwd,
        };

        let shell = app.config.default_shell;
        app.spawn_session_with_shell(shell);

        let load_task = Task::perform(
            tokio::task::spawn_blocking(|| {
                let agents = scan_installed_agents();
                let events = load_all_usage_events();
                (agents, events)
            }),
            |res| match res {
                Ok((agents, events)) => Message::AiDataLoaded(agents, events),
                Err(_) => Message::AiDataLoaded(Vec::new(), Vec::new()),
            },
        );
        let work_task = app.spawn_work_fetch_task();

        (app, Task::batch([load_task, work_task]))
    }

    fn spawn_work_fetch_task(&mut self) -> Task<Message> {
        self.work_loading = true;
        let cwd = self
            .active_session_id()
            .and_then(|id| self.session_by_id(id))
            .map(|s| s.cwd.clone())
            .unwrap_or_else(|| self.default_cwd.clone());
        let integrations = self.config.integrations.clone();
        Task::perform(
            tokio::task::spawn_blocking(move || {
                let remote = detect_git_remote(&cwd);
                let data = fetch_all_work_data(&integrations, remote.as_ref());
                (remote, data)
            }),
            |res| match res {
                Ok((remote, data)) => Message::WorkDataLoaded(remote, data),
                Err(_) => Message::WorkDataLoaded(None, WorkData::default()),
            },
        )
    }

    pub fn title(&self) -> String {
        "Velt".to_string()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn rebuild_report(&mut self) {
        self.usage_report = UsageReport::build(
            &self.usage_events,
            &self.detected_agents,
            self.usage_range,
            self.selected_agent.as_deref(),
        );
    }

    fn next_tab_number(&self, prefix: &str) -> u64 {
        let mut n = 1u64;
        loop {
            let candidate = format!("{prefix} {n}");
            let taken = self
                .sessions
                .iter()
                .any(|s| !s.archived && s.title == candidate);
            if !taken {
                return n;
            }
            n += 1;
        }
    }

    fn spawn_session_with_shell(&mut self, shell: ShellKind) {
        self.spawn_session_in_dir(shell, self.default_cwd.clone(), None);
    }

    fn spawn_session_in_dir(
        &mut self,
        shell: ShellKind,
        cwd: PathBuf,
        initial_cmd: Option<&str>,
    ) {
        let id = SessionId(self.next_session_id);
        self.next_session_id += 1;
        let (shell_cmd, shell_args) = shell.resolve_command(&self.config.custom_shell_path);
        let prefix = shell.short_label();
        let tab_num = self.next_tab_number(prefix);
        let title = format!("{prefix} {tab_num}");
        self.spawn_custom_session(id, title, prefix.to_string(), &shell_cmd, &shell_args, cwd);
        if let Some(cmd) = initial_cmd {
            if let Some(s) = self.session_by_id(id) {
                s.write_bytes(cmd.as_bytes());
            }
        }
    }

    fn spawn_agy_refresh_task(&mut self) -> Task<Message> {
        self.agy_loading = true;
        Task::perform(
            tokio::task::spawn_blocking(|| {
                let convs = list_antigravity_conversations();
                let sdk = check_sdk_status();
                (convs, sdk)
            }),
            |res| match res {
                Ok((convs, sdk)) => Message::AgyConversationsLoaded(convs, sdk),
                Err(_) => {
                    Message::AgyConversationsLoaded(Vec::new(), SdkStatus::MissingPython)
                }
            },
        )
    }

    fn spawn_custom_session(
        &mut self,
        id: SessionId,
        title: String,
        shell_label: String,
        shell_cmd: &str,
        shell_args: &[String],
        cwd: PathBuf,
    ) {
        match TerminalSession::spawn(
            id,
            title,
            shell_label,
            shell_cmd,
            shell_args,
            cwd,
            100,
            30,
        ) {
            Ok(session) => {
                self.last_dirty_versions
                    .insert(id, session.dirty_version());
                self.sessions.push(session);
                if self.panes.is_empty() {
                    self.panes.push(id);
                    self.focused_pane = 0;
                } else {
                    let target = self.focused_pane.min(self.panes.len() - 1);
                    self.panes[target] = id;
                    self.focused_pane = target;
                    self.pane_selections.remove(&target);
                }
                self.clear_all_caches();
            }
            Err(err) => {
                eprintln!("Failed to spawn session: {err}");
            }
        }
    }

    fn active_session_id(&self) -> Option<SessionId> {
        self.panes.get(self.focused_pane).copied()
    }

    fn session_by_id(&self, id: SessionId) -> Option<&TerminalSession> {
        self.sessions.iter().find(|s| s.id == id)
    }

    fn clear_all_caches(&mut self) {
        for cache in &self.canvas_caches {
            cache.clear();
        }
    }

    fn clear_pane_cache(&mut self, idx: usize) {
        if let Some(cache) = self.canvas_caches.get(idx) {
            cache.clear();
        }
    }

    fn refresh_term_search_matches(&mut self) {
        if !self.term_search_open {
            self.term_search_matches.clear();
            return;
        }
        self.term_search_matches = self
            .active_session_id()
            .and_then(|id| self.session_by_id(id))
            .map(|s| s.find_visible_matches(&self.term_search_query))
            .unwrap_or_default();
        if self.term_search_matches.is_empty() {
            self.term_search_index = 0;
        } else if self.term_search_index >= self.term_search_matches.len() {
            self.term_search_index = 0;
        }
    }

    fn ensure_split(&mut self, dir: SplitDirection) {
        self.split_direction = dir;
        if self.panes.len() >= 2 {
            self.clear_all_caches();
            return;
        }
        let current_id = self.active_session_id();
        let other_active = self
            .sessions
            .iter()
            .find(|s| !s.archived && Some(s.id) != current_id)
            .map(|s| s.id);

        if let Some(other_id) = other_active {
            self.panes.push(other_id);
            self.focused_pane = 1;
            self.clear_all_caches();
        } else {
            let id = SessionId(self.next_session_id);
            self.next_session_id += 1;
            let shell = self.config.default_shell;
            let (shell_cmd, shell_args) = shell.resolve_command(&self.config.custom_shell_path);
            let prefix = shell.short_label();
            let tab_num = self.next_tab_number(prefix);
            let title = format!("{prefix} {tab_num}");
            if let Ok(session) = TerminalSession::spawn(
                id,
                title,
                shell.short_label(),
                &shell_cmd,
                &shell_args,
                self.default_cwd.clone(),
                80,
                30,
            ) {
                self.last_dirty_versions
                    .insert(id, session.dirty_version());
                self.sessions.push(session);
                self.panes.push(id);
                self.focused_pane = 1;
                self.clear_all_caches();
            }
        }
    }

    fn close_split(&mut self) {
        if self.panes.len() >= 2 {
            let keep = self
                .panes
                .get(self.focused_pane)
                .copied()
                .unwrap_or(self.panes[0]);
            self.panes = vec![keep];
            self.focused_pane = 0;
            self.pane_selections.remove(&1);
            self.clear_all_caches();
        }
    }

    fn toggle_split(&mut self) {
        if self.panes.len() >= 2 {
            self.close_split();
        } else {
            self.ensure_split(self.split_direction);
        }
    }

    fn archive_session(&mut self, id: SessionId) {
        let active_count = self.sessions.iter().filter(|s| !s.archived).count();
        if active_count <= 1 {
            return;
        }

        let mut discard_empty = false;
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id) {
            session.send_exit();
            if session.has_user_input() {
                session.archived = true;
            } else {
                discard_empty = true;
            }
        }
        if discard_empty {
            self.sessions.retain(|s| s.id != id);
            self.last_dirty_versions.remove(&id);
        }

        let replacement = self
            .sessions
            .iter()
            .find(|s| !s.archived && !self.panes.contains(&s.id))
            .map(|s| s.id)
            .or_else(|| self.sessions.iter().find(|s| !s.archived).map(|s| s.id));

        if let Some(rep) = replacement {
            for pane_id in &mut self.panes {
                if *pane_id == id {
                    *pane_id = rep;
                }
            }
            if self.panes.len() == 2 && self.panes[0] == self.panes[1] {
                self.panes.truncate(1);
                self.focused_pane = 0;
            }
            self.clear_all_caches();
        }
    }

    fn copy_pane_selection(&mut self, pane_idx: usize) -> Option<Task<Message>> {
        let (a, b) = self.pane_selections.get(&pane_idx).copied()?;
        if a == b {
            return None;
        }
        let session_id = self.panes.get(pane_idx).copied()?;
        let session = self.session_by_id(session_id)?;
        let selected = session.extract_text_selection(a, b);
        if selected.is_empty() {
            return None;
        }
        self.pane_selections.remove(&pane_idx);
        self.status_toast = Some(("Copied!".to_string(), Instant::now()));
        self.clear_pane_cache(pane_idx);
        Some(clipboard::write(selected))
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                if let Some((_, t)) = self.status_toast {
                    if t.elapsed() >= Duration::from_secs(2) {
                        self.status_toast = None;
                    }
                }
                if self.last_context_refresh.elapsed() >= Duration::from_secs(3) {
                    self.last_context_refresh = Instant::now();
                    for session in &mut self.sessions {
                        if !session.archived {
                            session.refresh_context();
                        }
                    }
                }
                let mut refresh_search = false;
                for (pane_idx, pane_id) in self.panes.iter().enumerate() {
                    if let Some(session) = self.session_by_id(*pane_id) {
                        let v = session.dirty_version();
                        let prev = self.last_dirty_versions.get(pane_id).copied().unwrap_or(0);
                        if v != prev {
                            self.last_dirty_versions.insert(*pane_id, v);
                            if let Some(cache) = self.canvas_caches.get(pane_idx) {
                                cache.clear();
                            }
                            if self.term_search_open && pane_idx == self.focused_pane {
                                refresh_search = true;
                            }
                        }
                    }
                }
                if refresh_search {
                    self.refresh_term_search_matches();
                }
                Task::none()
            }
            Message::NewSession => {
                self.overlay = ActiveOverlay::None;
                let shell = self.config.default_shell;
                self.spawn_session_with_shell(shell);
                Task::none()
            }
            Message::SelectSession(id) => {
                self.overlay = ActiveOverlay::None;
                if let Some(existing_pane) = self.panes.iter().position(|&p| p == id) {
                    self.focused_pane = existing_pane;
                } else if self.panes.is_empty() {
                    self.panes.push(id);
                    self.focused_pane = 0;
                } else {
                    let target = self.focused_pane.min(self.panes.len() - 1);
                    self.panes[target] = id;
                    self.pane_selections.remove(&target);
                }
                self.clear_all_caches();
                Task::none()
            }
            Message::CloseSession(id) => {
                self.archive_session(id);
                Task::none()
            }
            Message::ToggleSidebar => {
                self.sidebar_visible = !self.sidebar_visible;
                self.clear_all_caches();
                Task::none()
            }
            Message::ToggleSplit => {
                self.overlay = ActiveOverlay::None;
                self.toggle_split();
                Task::none()
            }
            Message::ToggleUsage => {
                self.overlay = if self.overlay == ActiveOverlay::Usage {
                    ActiveOverlay::None
                } else {
                    ActiveOverlay::Usage
                };
                Task::none()
            }
            Message::ToggleSettings => {
                self.overlay = if self.overlay == ActiveOverlay::Settings {
                    ActiveOverlay::None
                } else {
                    ActiveOverlay::Settings
                };
                Task::none()
            }
            Message::TogglePalette => {
                if self.overlay == ActiveOverlay::Palette {
                    self.overlay = ActiveOverlay::None;
                    Task::none()
                } else {
                    self.overlay = ActiveOverlay::Palette;
                    self.palette_query.clear();
                    text_input::focus(self.palette_input_id.clone())
                }
            }
            Message::ToggleWork => {
                if self.overlay == ActiveOverlay::Work {
                    self.overlay = ActiveOverlay::None;
                    Task::none()
                } else {
                    self.overlay = ActiveOverlay::Work;
                    self.spawn_work_fetch_task()
                }
            }
            Message::ToggleAgy => {
                if self.overlay == ActiveOverlay::Agy {
                    self.overlay = ActiveOverlay::None;
                    Task::none()
                } else {
                    self.overlay = ActiveOverlay::Agy;
                    self.spawn_agy_refresh_task()
                }
            }
            Message::CloseOverlay => {
                self.overlay = ActiveOverlay::None;
                Task::none()
            }
            Message::UsageRangeChanged(range) => {
                self.usage_range = range;
                self.rebuild_report();
                Task::none()
            }
            Message::ToggleAgentFilter(agent) => {
                if self.selected_agent.as_deref() == Some(agent.as_str()) {
                    self.selected_agent = None;
                } else {
                    self.selected_agent = Some(agent);
                }
                self.rebuild_report();
                Task::none()
            }
            Message::RefreshAiData => {
                self.ai_loading = true;
                Task::perform(
                    tokio::task::spawn_blocking(|| {
                        let agents = scan_installed_agents();
                        let events = load_all_usage_events();
                        (agents, events)
                    }),
                    |res| match res {
                        Ok((agents, events)) => Message::AiDataLoaded(agents, events),
                        Err(_) => Message::AiDataLoaded(Vec::new(), Vec::new()),
                    },
                )
            }
            Message::AiDataLoaded(agents, events) => {
                self.detected_agents = agents;
                self.usage_events = events;
                self.ai_loading = false;
                self.rebuild_report();
                Task::none()
            }
            Message::AgyConversationsLoaded(convs, sdk) => {
                self.agy_loading = false;
                self.agy_sdk_status = sdk;
                let selected = self
                    .agy_selected_conv
                    .clone()
                    .filter(|id| convs.iter().any(|c| &c.id == id))
                    .or_else(|| convs.first().map(|c| c.id.clone()));
                self.agy_conversations = convs;
                self.agy_selected_conv = selected.clone();
                if let Some(conv_id) = selected {
                    return Task::perform(
                        tokio::task::spawn_blocking(move || {
                            let msgs = load_conversation_messages(&conv_id);
                            (conv_id, msgs)
                        }),
                        |res| match res {
                            Ok((id, msgs)) => Message::AgyMessagesLoaded(id, msgs),
                            Err(_) => Message::AgyMessagesLoaded(String::new(), Vec::new()),
                        },
                    );
                }
                Task::none()
            }
            Message::AgyMessagesLoaded(conv_id, msgs) => {
                if self.agy_selected_conv.as_deref() == Some(conv_id.as_str()) {
                    self.agy_conv_messages = msgs;
                }
                Task::none()
            }
            Message::AgySdkReply(res) => {
                self.agy_chat_busy = false;
                match res {
                    Ok(msg) => {
                        self.agy_chat_messages.push(msg);
                    }
                    Err(err) => {
                        self.agy_chat_messages.push(ChatMessageItem {
                            role: ChatRole::Assistant,
                            model: Some("Error".to_string()),
                            timestamp: chrono::Local::now().format("%H:%M").to_string(),
                            content: err,
                            thinking: None,
                            tools: Vec::new(),
                        });
                    }
                }
                Task::none()
            }
            Message::Agy(agy_msg) => match agy_msg {
                AgyMessage::SelectTab(tab) => {
                    self.agy_tab = tab;
                    if tab == AgyTab::NativeChat {
                        return text_input::focus(self.agy_chat_input_id.clone());
                    }
                    Task::none()
                }
                AgyMessage::SearchChanged(q) => {
                    self.agy_search = q;
                    Task::none()
                }
                AgyMessage::SelectConversation(conv_id) => {
                    self.agy_selected_conv = Some(conv_id.clone());
                    Task::perform(
                        tokio::task::spawn_blocking(move || {
                            let msgs = load_conversation_messages(&conv_id);
                            (conv_id, msgs)
                        }),
                        |res| match res {
                            Ok((id, msgs)) => Message::AgyMessagesLoaded(id, msgs),
                            Err(_) => Message::AgyMessagesLoaded(String::new(), Vec::new()),
                        },
                    )
                }
                AgyMessage::OpenProjectTerminal(path) => {
                    self.overlay = ActiveOverlay::None;
                    let shell = self.config.default_shell;
                    self.spawn_session_in_dir(shell, path, None);
                    Task::none()
                }
                AgyMessage::Refresh => self.spawn_agy_refresh_task(),
                AgyMessage::ChatInputChanged(v) => {
                    self.agy_chat_input = v;
                    Task::none()
                }
                AgyMessage::SendChat => {
                    let prompt = self.agy_chat_input.trim().to_string();
                    if prompt.is_empty() || self.agy_chat_busy {
                        return Task::none();
                    }
                    let python_exe = match &self.agy_sdk_status {
                        SdkStatus::Ready(exe) => exe.clone(),
                        _ => {
                            self.agy_chat_messages.push(ChatMessageItem {
                                role: ChatRole::Assistant,
                                model: Some("SDK".to_string()),
                                timestamp: chrono::Local::now().format("%H:%M").to_string(),
                                content: "Install `google-antigravity` SDK first using the button above.".to_string(),
                                thinking: None,
                                tools: Vec::new(),
                            });
                            return Task::none();
                        }
                    };
                    self.agy_chat_input.clear();
                    self.agy_chat_messages.push(ChatMessageItem {
                        role: ChatRole::User,
                        model: None,
                        timestamp: chrono::Local::now().format("%H:%M").to_string(),
                        content: prompt.clone(),
                        thinking: None,
                        tools: Vec::new(),
                    });
                    self.agy_chat_busy = true;
                    let cwd = self
                        .active_session_id()
                        .and_then(|id| self.session_by_id(id))
                        .map(|s| s.cwd.clone())
                        .unwrap_or_else(|| self.default_cwd.clone());
                    Task::perform(
                        tokio::task::spawn_blocking(move || {
                            send_sdk_chat_prompt(&python_exe, &cwd, &prompt)
                        }),
                        |res| match res {
                            Ok(reply) => Message::AgySdkReply(reply),
                            Err(_) => {
                                Message::AgySdkReply(Err("SDK worker failed.".to_string()))
                            }
                        },
                    )
                }
                AgyMessage::InstallSdk => {
                    let cmd = match &self.agy_sdk_status {
                        SdkStatus::MissingSdk(py) => {
                            format!("{py} -m pip install google-antigravity\r")
                        }
                        _ => "pip install google-antigravity\r".to_string(),
                    };
                    self.overlay = ActiveOverlay::None;
                    let shell = self.config.default_shell;
                    let cwd = self.default_cwd.clone();
                    self.spawn_session_in_dir(shell, cwd, Some(&cmd));
                    Task::none()
                }
                AgyMessage::ClearChat => {
                    self.agy_chat_messages.clear();
                    Task::none()
                }
                AgyMessage::Close => {
                    self.overlay = ActiveOverlay::None;
                    Task::none()
                }
            },
            Message::WorkDataLoaded(remote, data) => {
                self.detected_remote = remote;
                self.work_data = data;
                self.work_loading = false;
                Task::none()
            }
            Message::Work(work_msg) => match work_msg {
                WorkMessage::SelectTab(tab) => {
                    self.work_tab = tab;
                    Task::none()
                }
                WorkMessage::SelectJiraFilter(f) => {
                    self.jira_filter = f;
                    Task::none()
                }
                WorkMessage::Refresh => self.spawn_work_fetch_task(),
                WorkMessage::StartJiraBranch(key, summary) => {
                    self.overlay = ActiveOverlay::None;
                    let branch = slugify_branch(&key, &summary);
                    if let Some(id) = self.active_session_id() {
                        if let Some(session) = self.session_by_id(id) {
                            session.scroll_to_bottom();
                            session.write_bytes(format!("git checkout -b {branch}\r").as_bytes());
                        }
                    }
                    Task::none()
                }
                WorkMessage::CheckoutPr(branch) => {
                    self.overlay = ActiveOverlay::None;
                    if let Some(id) = self.active_session_id() {
                        if let Some(session) = self.session_by_id(id) {
                            session.scroll_to_bottom();
                            session.write_bytes(
                                format!("git fetch && git checkout {branch}\r").as_bytes(),
                            );
                        }
                    }
                    Task::none()
                }
                WorkMessage::OpenUrl(url) => {
                    open_in_browser(&url);
                    Task::none()
                }
                WorkMessage::SetJiraDomain(v) => {
                    self.config.integrations.jira_domain = v;
                    Task::none()
                }
                WorkMessage::SetJiraEmail(v) => {
                    self.config.integrations.jira_email = v;
                    Task::none()
                }
                WorkMessage::SetJiraToken(v) => {
                    self.config.integrations.jira_api_token = v;
                    Task::none()
                }
                WorkMessage::SetJiraClientId(v) => {
                    self.config.integrations.jira_oauth_client_id = v;
                    Task::none()
                }
                WorkMessage::SetJiraClientSecret(v) => {
                    self.config.integrations.jira_oauth_client_secret = v;
                    Task::none()
                }
                WorkMessage::SetBitbucketWorkspace(v) => {
                    self.config.integrations.bitbucket_workspace = v;
                    Task::none()
                }
                WorkMessage::SetBitbucketUser(v) => {
                    self.config.integrations.bitbucket_username = v;
                    Task::none()
                }
                WorkMessage::SetBitbucketPass(v) => {
                    self.config.integrations.bitbucket_app_password = v;
                    Task::none()
                }
                WorkMessage::SetGithubToken(v) => {
                    self.config.integrations.github_token = v;
                    Task::none()
                }
                WorkMessage::SetGithubUser(v) => {
                    self.config.integrations.github_username = v;
                    Task::none()
                }
                WorkMessage::SetGithubClientId(v) => {
                    self.config.integrations.github_oauth_client_id = v;
                    Task::none()
                }
                WorkMessage::SetGithubClientSecret(v) => {
                    self.config.integrations.github_oauth_client_secret = v;
                    Task::none()
                }
                WorkMessage::ImportGhCli => {
                    self.auth_status = Some("Importing GitHub token via `gh auth token`...".into());
                    Task::perform(
                        tokio::task::spawn_blocking(import_github_cli_token),
                        |res| match res {
                            Ok(Ok(token)) => Message::AuthFlowCompleted(AuthOutcome::GithubToken(
                                token,
                                String::new(),
                            )),
                            Ok(Err(e)) => Message::AuthFlowCompleted(AuthOutcome::Error(e)),
                            Err(_) => Message::AuthFlowCompleted(AuthOutcome::Error(
                                "Task failed".to_string(),
                            )),
                        },
                    )
                }
                WorkMessage::ImportGitGcmGithub => {
                    self.auth_status =
                        Some("Reading GitHub credentials from Git Credential Manager...".into());
                    Task::perform(
                        tokio::task::spawn_blocking(|| import_git_credential_manager("github.com")),
                        |res| match res {
                            Ok(Ok((user, pass))) => {
                                Message::AuthFlowCompleted(AuthOutcome::GithubToken(pass, user))
                            }
                            Ok(Err(e)) => Message::AuthFlowCompleted(AuthOutcome::Error(e)),
                            Err(_) => Message::AuthFlowCompleted(AuthOutcome::Error(
                                "Task failed".to_string(),
                            )),
                        },
                    )
                }
                WorkMessage::ImportGitGcmBitbucket => {
                    self.auth_status =
                        Some("Reading Bitbucket credentials from Git Credential Manager...".into());
                    Task::perform(
                        tokio::task::spawn_blocking(|| {
                            import_git_credential_manager("bitbucket.org")
                        }),
                        |res| match res {
                            Ok(Ok((user, pass))) => {
                                Message::AuthFlowCompleted(AuthOutcome::BitbucketCreds(user, pass))
                            }
                            Ok(Err(e)) => Message::AuthFlowCompleted(AuthOutcome::Error(e)),
                            Err(_) => Message::AuthFlowCompleted(AuthOutcome::Error(
                                "Task failed".to_string(),
                            )),
                        },
                    )
                }
                WorkMessage::StartAtlassianOAuth => {
                    let cid = self.config.integrations.jira_oauth_client_id.clone();
                    let secret = self.config.integrations.jira_oauth_client_secret.clone();
                    self.config.save();
                    self.auth_status = Some(
                        "Opening browser for Atlassian OAuth 2.0... Approve access to finish."
                            .into(),
                    );
                    Task::perform(
                        tokio::task::spawn_blocking(move || run_atlassian_oauth(&cid, &secret)),
                        |res| match res {
                            Ok(Ok((access_token, cloud_id, domain))) => {
                                Message::AuthFlowCompleted(AuthOutcome::AtlassianOAuth {
                                    access_token,
                                    cloud_id,
                                    domain,
                                })
                            }
                            Ok(Err(e)) => Message::AuthFlowCompleted(AuthOutcome::Error(e)),
                            Err(_) => Message::AuthFlowCompleted(AuthOutcome::Error(
                                "OAuth task failed".to_string(),
                            )),
                        },
                    )
                }
                WorkMessage::StartGithubOAuth => {
                    let cid = self.config.integrations.github_oauth_client_id.clone();
                    let secret = self.config.integrations.github_oauth_client_secret.clone();
                    self.config.save();
                    self.auth_status = Some(
                        "Opening browser for GitHub OAuth 2.0... Approve access to finish.".into(),
                    );
                    Task::perform(
                        tokio::task::spawn_blocking(move || run_github_oauth(&cid, &secret)),
                        |res| match res {
                            Ok(Ok(token)) => Message::AuthFlowCompleted(AuthOutcome::GithubToken(
                                token,
                                String::new(),
                            )),
                            Ok(Err(e)) => Message::AuthFlowCompleted(AuthOutcome::Error(e)),
                            Err(_) => Message::AuthFlowCompleted(AuthOutcome::Error(
                                "OAuth task failed".to_string(),
                            )),
                        },
                    )
                }
                WorkMessage::SaveAndSync => {
                    self.config.save();
                    self.auth_status = Some("Saved! Syncing Jira, Bitbucket & GitHub...".into());
                    self.spawn_work_fetch_task()
                }
                WorkMessage::Close => {
                    self.overlay = ActiveOverlay::None;
                    Task::none()
                }
            },
            Message::AuthFlowCompleted(outcome) => match outcome {
                AuthOutcome::GithubToken(token, user) => {
                    self.config.integrations.github_token = token;
                    if !user.is_empty() && self.config.integrations.github_username.is_empty() {
                        self.config.integrations.github_username = user;
                    }
                    self.config.save();
                    self.auth_status = Some("GitHub connected! Syncing PRs...".into());
                    self.spawn_work_fetch_task()
                }
                AuthOutcome::BitbucketCreds(user, pass) => {
                    if !user.is_empty() {
                        self.config.integrations.bitbucket_username = user;
                    }
                    self.config.integrations.bitbucket_app_password = pass;
                    self.config.save();
                    self.auth_status = Some("Bitbucket imported from Git! Syncing PRs...".into());
                    self.spawn_work_fetch_task()
                }
                AuthOutcome::AtlassianOAuth {
                    access_token,
                    cloud_id,
                    domain,
                } => {
                    self.config.integrations.jira_oauth_access_token = access_token;
                    self.config.integrations.jira_cloud_id = cloud_id;
                    if !domain.is_empty() {
                        self.config.integrations.jira_domain = domain;
                    }
                    self.config.save();
                    self.auth_status = Some("Atlassian OAuth 2.0 connected! Syncing Jira...".into());
                    self.spawn_work_fetch_task()
                }
                AuthOutcome::Error(err) => {
                    self.auth_status = Some(err);
                    Task::none()
                }
            },
            Message::Settings(s_msg) => match s_msg {
                SettingsMessage::SelectShell(shell) => {
                    self.config.default_shell = shell;
                    self.config.save();
                    Task::none()
                }
                SettingsMessage::CustomShellChanged(path) => {
                    self.config.custom_shell_path = path;
                    self.config.save();
                    Task::none()
                }
                SettingsMessage::SelectFontSize(size) => {
                    self.config.font_size = size;
                    self.config.save();
                    self.clear_all_caches();
                    Task::none()
                }
                SettingsMessage::SelectTheme(theme) => {
                    self.config.terminal_theme = theme;
                    self.config.save();
                    self.clear_all_caches();
                    Task::none()
                }
                SettingsMessage::OpenWithSelectedShell => {
                    self.overlay = ActiveOverlay::None;
                    let shell = self.config.default_shell;
                    self.spawn_session_with_shell(shell);
                    Task::none()
                }
                SettingsMessage::Close => {
                    self.overlay = ActiveOverlay::None;
                    Task::none()
                }
            },
            Message::FilterChanged(filter) => {
                self.sidebar_filter = filter;
                Task::none()
            }
            Message::SearchChanged(q) => {
                self.search_query = q;
                Task::none()
            }
            Message::PaletteQueryChanged(q) => {
                self.palette_query = q;
                Task::none()
            }
            Message::RunPaletteAction(action) => {
                self.overlay = ActiveOverlay::None;
                match action {
                    PaletteAction::NewShell(shell) => {
                        self.spawn_session_with_shell(shell);
                    }
                    PaletteAction::SplitHorizontal => {
                        self.ensure_split(SplitDirection::Horizontal);
                    }
                    PaletteAction::SplitVertical => {
                        self.ensure_split(SplitDirection::Vertical);
                    }
                    PaletteAction::CloseSplit => {
                        self.close_split();
                    }
                    PaletteAction::ToggleSidebar => {
                        self.sidebar_visible = !self.sidebar_visible;
                        self.clear_all_caches();
                    }
                    PaletteAction::OpenUsageToday => {
                        self.usage_range = UsageRange::Today;
                        self.rebuild_report();
                        self.overlay = ActiveOverlay::Usage;
                    }
                    PaletteAction::OpenUsage30d => {
                        self.usage_range = UsageRange::Days30;
                        self.rebuild_report();
                        self.overlay = ActiveOverlay::Usage;
                    }
                    PaletteAction::OpenAgyHub => {
                        self.overlay = ActiveOverlay::Agy;
                        return self.spawn_agy_refresh_task();
                    }
                    PaletteAction::OpenWorkHub => {
                        self.overlay = ActiveOverlay::Work;
                        return self.spawn_work_fetch_task();
                    }
                    PaletteAction::OpenSettings => {
                        self.overlay = ActiveOverlay::Settings;
                    }
                    PaletteAction::FindInTerminal => {
                        self.term_search_open = true;
                        self.refresh_term_search_matches();
                        self.clear_all_caches();
                        return text_input::focus(self.term_search_input_id.clone());
                    }
                    PaletteAction::RunCommand(cmd) => {
                        if let Some(id) = self.active_session_id() {
                            if let Some(session) = self.session_by_id(id) {
                                session.scroll_to_bottom();
                                session.write_bytes(cmd.as_bytes());
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::Terminal(term_msg) => match term_msg {
                TerminalMessage::ResizePane(idx, cols, rows) => {
                    if let Some(&id) = self.panes.get(idx) {
                        if let Some(session) = self.session_by_id(id) {
                            session.resize(cols, rows);
                            self.clear_pane_cache(idx);
                        }
                    }
                    Task::none()
                }
                TerminalMessage::ScrollPane(idx, delta) => {
                    if let Some(&id) = self.panes.get(idx) {
                        if let Some(session) = self.session_by_id(id) {
                            session.scroll_lines(delta);
                            self.pane_selections.remove(&idx);
                            self.clear_pane_cache(idx);
                        }
                    }
                    Task::none()
                }
                TerminalMessage::SelectionStart(idx, col, row) => {
                    if idx < self.panes.len() {
                        self.focused_pane = idx;
                        self.pane_selections.insert(idx, ((col, row), (col, row)));
                        self.clear_all_caches();
                    }
                    Task::none()
                }
                TerminalMessage::SelectionUpdate(idx, col, row) => {
                    if let Some(sel) = self.pane_selections.get_mut(&idx) {
                        sel.1 = (col, row);
                        self.clear_pane_cache(idx);
                    }
                    Task::none()
                }
                TerminalMessage::SelectionEnd(idx) => {
                    if let Some(&(a, b)) = self.pane_selections.get(&idx) {
                        if a == b {
                            self.pane_selections.remove(&idx);
                            self.clear_pane_cache(idx);
                        }
                    }
                    Task::none()
                }
                TerminalMessage::DoubleClickWord(idx, col, row) => {
                    self.focused_pane = idx.min(self.panes.len().saturating_sub(1));
                    if let Some(&id) = self.panes.get(idx) {
                        if let Some(session) = self.session_by_id(id) {
                            if let Some(bounds) = session.word_bounds_at(col, row) {
                                self.pane_selections.insert(idx, bounds);
                                self.clear_all_caches();
                            }
                        }
                    }
                    Task::none()
                }
                TerminalMessage::RightClickPane(idx) => {
                    if idx < self.panes.len() {
                        self.focused_pane = idx;
                    }
                    if let Some(copy_task) = self.copy_pane_selection(idx) {
                        copy_task
                    } else {
                        clipboard::read().map(Message::PasteClipboard)
                    }
                }
                TerminalMessage::CtrlClickCell(idx, col, row) => {
                    if let Some(&id) = self.panes.get(idx) {
                        if let Some(session) = self.session_by_id(id) {
                            if let Some(url) = session.url_at_cell(col, row) {
                                open_in_browser(&url);
                                self.status_toast = Some(("Opened URL!".to_string(), Instant::now()));
                            }
                        }
                    }
                    Task::none()
                }
            },
            Message::ModifiersChanged(mods) => {
                self.ctrl_held = mods.control();
                Task::none()
            }
            Message::ToggleTermSearch => {
                if self.overlay != ActiveOverlay::None {
                    return Task::none();
                }
                self.term_search_open = !self.term_search_open;
                self.term_search_index = 0;
                self.refresh_term_search_matches();
                self.clear_all_caches();
                if self.term_search_open {
                    text_input::focus(self.term_search_input_id.clone())
                } else {
                    Task::none()
                }
            }
            Message::TermSearchChanged(q) => {
                self.term_search_query = q;
                self.term_search_index = 0;
                self.refresh_term_search_matches();
                self.clear_all_caches();
                Task::none()
            }
            Message::TermSearchNext => {
                self.refresh_term_search_matches();
                let count = self.term_search_matches.len();
                if count > 0 {
                    self.term_search_index = (self.term_search_index + 1) % count;
                    self.clear_pane_cache(self.focused_pane);
                }
                Task::none()
            }
            Message::TermSearchPrev => {
                self.refresh_term_search_matches();
                let count = self.term_search_matches.len();
                if count > 0 {
                    self.term_search_index = if self.term_search_index == 0 {
                        count - 1
                    } else {
                        self.term_search_index - 1
                    };
                    self.clear_pane_cache(self.focused_pane);
                }
                Task::none()
            }
            Message::CloseTermSearch => {
                self.term_search_open = false;
                self.term_search_query.clear();
                self.term_search_index = 0;
                self.term_search_matches.clear();
                self.clear_all_caches();
                Task::none()
            }
            Message::JumpToBottom(idx) => {
                if let Some(&id) = self.panes.get(idx) {
                    if let Some(session) = self.session_by_id(id) {
                        session.scroll_to_bottom();
                        self.clear_pane_cache(idx);
                    }
                }
                Task::none()
            }
            Message::PasteClipboard(maybe_text) => {
                if let Some(clip) = maybe_text {
                    if let Some(id) = self.active_session_id() {
                        if let Some(session) = self.session_by_id(id) {
                            session.scroll_to_bottom();
                            session.write_bytes(clip.as_bytes());
                        }
                    }
                }
                Task::none()
            }
            Message::KeyboardEvent(Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            })) => self.handle_key_press(key, modifiers, text.as_deref()),
            Message::KeyboardEvent(_) => Task::none(),
        }
    }

    fn handle_key_press(
        &mut self,
        key: Key,
        modifiers: Modifiers,
        text: Option<&str>,
    ) -> Task<Message> {
        let ctrl = modifiers.control();
        let shift = modifiers.shift();

        // Global shortcuts
        if ctrl {
            match key.as_ref() {
                Key::Character("k") | Key::Character("K") => {
                    return self.update(Message::TogglePalette);
                }
                Key::Character("t") | Key::Character("T") => {
                    self.overlay = ActiveOverlay::None;
                    let shell = self.config.default_shell;
                    self.spawn_session_with_shell(shell);
                    return Task::none();
                }
                Key::Character("w") | Key::Character("W") => {
                    if let Some(id) = self.active_session_id() {
                        self.archive_session(id);
                    }
                    return Task::none();
                }
                Key::Character("b") | Key::Character("B") => {
                    self.sidebar_visible = !self.sidebar_visible;
                    self.clear_all_caches();
                    return Task::none();
                }
                Key::Character("u") | Key::Character("U") => {
                    self.overlay = if self.overlay == ActiveOverlay::Usage {
                        ActiveOverlay::None
                    } else {
                        ActiveOverlay::Usage
                    };
                    return Task::none();
                }
                Key::Character("j") | Key::Character("J") => {
                    return self.update(Message::ToggleWork);
                }
                Key::Character("f") | Key::Character("F") => {
                    return self.update(Message::ToggleTermSearch);
                }
                Key::Character(",") => {
                    self.overlay = if self.overlay == ActiveOverlay::Settings {
                        ActiveOverlay::None
                    } else {
                        ActiveOverlay::Settings
                    };
                    return Task::none();
                }
                Key::Character("\\") | Key::Character("|") => {
                    self.overlay = ActiveOverlay::None;
                    self.toggle_split();
                    return Task::none();
                }
                Key::Character("d") | Key::Character("D") if shift => {
                    self.overlay = ActiveOverlay::None;
                    self.toggle_split();
                    return Task::none();
                }
                Key::Character("a") | Key::Character("A") if shift => {
                    return self.update(Message::ToggleAgy);
                }
                Key::Character("c") | Key::Character("C") if shift => {
                    if let Some(task) = self.copy_pane_selection(self.focused_pane) {
                        return task;
                    }
                    return Task::none();
                }
                Key::Character("v") | Key::Character("V") if shift => {
                    return clipboard::read().map(Message::PasteClipboard);
                }
                _ => {}
            }
        }

        if self.overlay != ActiveOverlay::None {
            if matches!(key, Key::Named(Named::Escape)) {
                self.overlay = ActiveOverlay::None;
            }
            return Task::none();
        }

        if self.term_search_open && matches!(key, Key::Named(Named::Escape)) {
            return self.update(Message::CloseTermSearch);
        }

        // Clear active text selection on typing
        if self.pane_selections.remove(&self.focused_pane).is_some() {
            self.clear_pane_cache(self.focused_pane);
        }

        let is_enter = matches!(key, Key::Named(Named::Enter));

        // Forward terminal keystrokes to the active PTY session
        let Some(active_id) = self.active_session_id() else {
            return Task::none();
        };
        let Some(session) = self.session_by_id(active_id) else {
            return Task::none();
        };

        if let Some(bytes) = encode_key_to_pty(key, modifiers, text) {
            session.scroll_to_bottom();
            session.write_bytes(&bytes);
            if let Some(cache) = self.canvas_caches.get(self.focused_pane) {
                cache.clear();
            }
        }

        if is_enter {
            if let Some(s) = self.sessions.iter_mut().find(|s| s.id == active_id) {
                s.refresh_context();
            }
        }

        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let tick = iced::time::every(Duration::from_millis(33)).map(|_| Message::Tick);
        let events = event::listen_with(|event, status, _id| match &event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(mods)) => {
                Some(Message::ModifiersChanged(*mods))
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                let is_global = modifiers.control()
                    && matches!(
                        key.as_ref(),
                        Key::Character("k")
                            | Key::Character("K")
                            | Key::Character("t")
                            | Key::Character("T")
                            | Key::Character("w")
                            | Key::Character("W")
                            | Key::Character("b")
                            | Key::Character("B")
                            | Key::Character("u")
                            | Key::Character("U")
                            | Key::Character("j")
                            | Key::Character("J")
                            | Key::Character("f")
                            | Key::Character("F")
                            | Key::Character(",")
                            | Key::Character("\\")
                            | Key::Character("|")
                    )
                    || (modifiers.control()
                        && modifiers.shift()
                        && matches!(
                            key.as_ref(),
                            Key::Character("d")
                                | Key::Character("D")
                                | Key::Character("a")
                                | Key::Character("A")
                                | Key::Character("c")
                                | Key::Character("C")
                                | Key::Character("v")
                                | Key::Character("V")
                        ))
                    || matches!(key, Key::Named(Named::Escape));

                if is_global || status == Status::Ignored {
                    Some(Message::KeyboardEvent(event))
                } else {
                    None
                }
            }
            _ => None,
        });

        Subscription::batch([tick, events])
    }

    pub fn view(&self) -> Element<'_, Message> {
        let active_id = self.active_session_id();
        let top_bar = view_tab_bar(
            &self.sessions,
            active_id,
            self.sidebar_visible,
            self.panes.len() >= 2,
            self.overlay == ActiveOverlay::Usage,
            self.overlay == ActiveOverlay::Settings,
            self.overlay == ActiveOverlay::Palette,
            self.overlay == ActiveOverlay::Work,
            self.overlay == ActiveOverlay::Agy,
            Message::ToggleSidebar,
            Message::SelectSession,
            Message::CloseSession,
            Message::NewSession,
            Message::ToggleSplit,
            Message::ToggleAgy,
            Message::ToggleUsage,
            Message::ToggleWork,
            Message::ToggleSettings,
            Message::TogglePalette,
        );

        let toast_ref = self.status_toast.as_ref().map(|(s, _)| s.as_str());

        let main_area: Element<Message> = match self.overlay {
            ActiveOverlay::Usage => view_usage_dashboard(
                &self.usage_report,
                self.usage_range,
                self.selected_agent.as_deref(),
                self.ai_loading,
                Message::UsageRangeChanged,
                Message::ToggleAgentFilter,
                Message::RefreshAiData,
                Message::CloseOverlay,
            ),
            ActiveOverlay::Settings => view_settings(&self.config).map(Message::Settings),
            ActiveOverlay::Palette => view_palette(
                &self.palette_query,
                self.palette_input_id.clone(),
                Message::PaletteQueryChanged,
                Message::RunPaletteAction,
                Message::CloseOverlay,
            ),
            ActiveOverlay::Work => view_work_hub(
                self.work_tab,
                self.jira_filter,
                &self.work_data,
                &self.config.integrations,
                self.detected_remote.as_ref(),
                self.auth_status.as_deref(),
                self.work_loading,
            )
            .map(Message::Work),
            ActiveOverlay::Agy => view_agy_hub(
                self.agy_tab,
                &self.agy_conversations,
                self.agy_selected_conv.as_deref(),
                &self.agy_conv_messages,
                &self.agy_search,
                &self.agy_sdk_status,
                &self.agy_chat_messages,
                &self.agy_chat_input,
                self.agy_chat_input_id.clone(),
                self.agy_chat_busy,
                self.agy_loading,
            )
            .map(Message::Agy),
            ActiveOverlay::None => {
                let mut pane_elements = Vec::new();
                for (idx, &pane_session_id) in self.panes.iter().enumerate() {
                    if let Some(session) = self.session_by_id(pane_session_id) {
                        let cache = self
                            .canvas_caches
                            .get(idx)
                            .unwrap_or(&self.canvas_caches[0]);
                        let selection = self.pane_selections.get(&idx).copied();
                        let is_focused = idx == self.focused_pane;
                        let matches: &[(usize, usize, usize)] =
                            if self.term_search_open && is_focused {
                                &self.term_search_matches
                            } else {
                                &[]
                            };
                        let active_match = if matches.is_empty() {
                            0
                        } else {
                            self.term_search_index % matches.len()
                        };
                        let search_overlay = if self.term_search_open && is_focused {
                            Some(SearchOverlay {
                                query: &self.term_search_query,
                                input_id: self.term_search_input_id.clone(),
                                current: active_match,
                                total: matches.len(),
                                on_change: Message::TermSearchChanged,
                                on_next: Message::TermSearchNext,
                                on_prev: Message::TermSearchPrev,
                                on_close: Message::CloseTermSearch,
                            })
                        } else {
                            None
                        };
                        let pane_el = view_terminal_pane(
                            session,
                            idx,
                            is_focused,
                            self.ctrl_held,
                            self.config.font_size,
                            self.terminal_font,
                            self.config.terminal_theme,
                            selection,
                            matches,
                            active_match,
                            search_overlay,
                            toast_ref,
                            cache,
                            Message::Terminal,
                            Message::JumpToBottom(idx),
                            Message::TogglePalette,
                        );
                        pane_elements.push(pane_el);
                    }
                }

                if pane_elements.is_empty() {
                    container(text("No active terminal session").color(theme::TEXT_MUTED))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::Alignment::Center)
                        .align_y(iced::Alignment::Center)
                        .into()
                } else {
                    match self.split_direction {
                        SplitDirection::Horizontal => {
                            let mut r = row![].spacing(0).width(Length::Fill).height(Length::Fill);
                            for el in pane_elements {
                                r = r.push(el);
                            }
                            r.into()
                        }
                        SplitDirection::Vertical => {
                            let mut c = column![].spacing(0).width(Length::Fill).height(Length::Fill);
                            for el in pane_elements {
                                c = c.push(el);
                            }
                            c.into()
                        }
                    }
                }
            }
        };

        let content_row = if self.sidebar_visible && self.overlay == ActiveOverlay::None {
            let sidebar = view_sidebar(
                &self.sessions,
                active_id,
                self.sidebar_filter,
                &self.search_query,
                self.search_input_id.clone(),
                &self.detected_agents,
                Message::NewSession,
                Message::SearchChanged,
                Message::FilterChanged,
                Message::SelectSession,
                Message::ToggleUsage,
            );
            row![sidebar, main_area]
                .width(Length::Fill)
                .height(Length::Fill)
        } else {
            row![main_area].width(Length::Fill).height(Length::Fill)
        };

        container(
            column![
                top_bar,
                container(Space::with_height(0)),
                content_row,
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::app_bg)
        .into()
    }
}

fn encode_key_to_pty(key: Key, modifiers: Modifiers, text: Option<&str>) -> Option<Vec<u8>> {
    if modifiers.control() && !modifiers.alt() {
        if let Key::Character(c) = key.as_ref() {
            let ch = c.chars().next()?;
            let lower = ch.to_ascii_lowercase();
            if lower.is_ascii_lowercase() {
                let ctrl_byte = (lower as u8) - b'a' + 1;
                return Some(vec![ctrl_byte]);
            }
        }
    }

    match key.as_ref() {
        Key::Named(Named::Enter) => Some(b"\r".to_vec()),
        Key::Named(Named::Backspace) => Some(vec![0x7f]),
        Key::Named(Named::Tab) => Some(b"\t".to_vec()),
        Key::Named(Named::Escape) => Some(vec![0x1b]),
        Key::Named(Named::Space) => Some(b" ".to_vec()),
        Key::Named(Named::ArrowUp) => Some(b"\x1b[A".to_vec()),
        Key::Named(Named::ArrowDown) => Some(b"\x1b[B".to_vec()),
        Key::Named(Named::ArrowRight) => Some(b"\x1b[C".to_vec()),
        Key::Named(Named::ArrowLeft) => Some(b"\x1b[D".to_vec()),
        Key::Named(Named::Home) => Some(b"\x1b[H".to_vec()),
        Key::Named(Named::End) => Some(b"\x1b[F".to_vec()),
        Key::Named(Named::Delete) => Some(b"\x1b[3~".to_vec()),
        Key::Named(Named::PageUp) => Some(b"\x1b[5~".to_vec()),
        Key::Named(Named::PageDown) => Some(b"\x1b[6~".to_vec()),
        _ => {
            if let Some(s) = text {
                if !s.is_empty() && !s.chars().all(|c| c.is_control()) {
                    return Some(s.as_bytes().to_vec());
                }
            }
            if let Key::Character(c) = key.as_ref() {
                if !modifiers.control() && !modifiers.alt() {
                    return Some(c.as_bytes().to_vec());
                }
            }
            None
        }
    }
}
