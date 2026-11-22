use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Working,
    Waiting,
    Done,
}

#[derive(Debug, Clone, Copy)]
pub struct TermDimensions {
    pub cols: usize,
    pub screen_lines: usize,
}

impl Dimensions for TermDimensions {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

pub struct TerminalSession {
    pub id: SessionId,
    pub title: String,
    pub shell_label: String,
    pub project_name: String,
    pub cwd: PathBuf,
    pub git_branch: Option<String>,
    pub tech_stack: Vec<&'static str>,
    pub archived: bool,
    pub created_at: Instant,
    pub term: Arc<Mutex<Term<VoidListener>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    size: Arc<Mutex<(u16, u16)>>,
    pub dirty_version: Arc<AtomicU64>,
    last_output_ms: Arc<AtomicU64>,
    pub exited: Arc<AtomicBool>,
    has_user_input: Arc<AtomicBool>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl TerminalSession {
    pub fn spawn(
        id: SessionId,
        title: impl Into<String>,
        shell_label: impl Into<String>,
        shell_cmd: &str,
        shell_args: &[String],
        cwd: PathBuf,
        cols: u16,
        rows: u16,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows.max(4),
                cols: cols.max(10),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open ConPTY: {e}"))?;

        let mut cmd = CommandBuilder::new(shell_cmd);
        for arg in shell_args {
            cmd.arg(arg);
        }
        cmd.cwd(&cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn shell '{shell_cmd}': {e}"))?;
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone PTY reader: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to acquire PTY writer: {e}"))?;

        let dims = TermDimensions {
            cols: cols.max(10) as usize,
            screen_lines: rows.max(4) as usize,
        };
        let term = Arc::new(Mutex::new(Term::new(Config::default(), &dims, VoidListener)));

        let dirty_version = Arc::new(AtomicU64::new(1));
        let last_output_ms = Arc::new(AtomicU64::new(now_ms()));
        let exited = Arc::new(AtomicBool::new(false));

        let term_bg = Arc::clone(&term);
        let dirty_bg = Arc::clone(&dirty_version);
        let output_bg = Arc::clone(&last_output_ms);
        let exited_bg = Arc::clone(&exited);

        std::thread::Builder::new()
            .name(format!("pty-reader-{}", id.0))
            .spawn(move || {
                let mut reader = reader;
                let mut processor: Processor = Processor::new();
                let mut buf = [0u8; 8192];

                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => {
                            exited_bg.store(true, Ordering::Release);
                            dirty_bg.fetch_add(1, Ordering::Release);
                            break;
                        }
                        Ok(n) => {
                            if let Ok(mut term_guard) = term_bg.lock() {
                                for &byte in &buf[..n] {
                                    processor.advance(&mut *term_guard, byte);
                                }
                            }
                            output_bg.store(now_ms(), Ordering::Release);
                            dirty_bg.fetch_add(1, Ordering::Release);
                        }
                        Err(_) => {
                            exited_bg.store(true, Ordering::Release);
                            dirty_bg.fetch_add(1, Ordering::Release);
                            break;
                        }
                    }
                }
                let _ = child.wait();
            })
            .map_err(|e| format!("Failed to spawn PTY reader thread: {e}"))?;

        let project_name = cwd
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("workspace")
            .to_string();

        let git_branch = detect_git_branch(&cwd);
        let tech_stack = detect_tech_stack(&cwd);

        Ok(Self {
            id,
            title: title.into(),
            shell_label: shell_label.into(),
            project_name,
            cwd,
            git_branch,
            tech_stack,
            archived: false,
            created_at: Instant::now(),
            term,
            master: Arc::new(Mutex::new(pair.master)),
            writer: Arc::new(Mutex::new(writer)),
            size: Arc::new(Mutex::new((cols.max(10), rows.max(4)))),
            dirty_version,
            last_output_ms,
            exited,
            has_user_input: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn write_bytes(&self, bytes: &[u8]) {
        if bytes.is_empty() || self.exited.load(Ordering::Acquire) {
            return;
        }
        self.has_user_input.store(true, Ordering::Release);
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(bytes);
            let _ = writer.flush();
        }
    }

    pub fn send_exit(&self) {
        if self.exited.load(Ordering::Acquire) {
            return;
        }
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(b"exit\r");
            let _ = writer.flush();
        }
    }

    pub fn has_user_input(&self) -> bool {
        self.has_user_input.load(Ordering::Acquire)
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        let cols = cols.max(10);
        let rows = rows.max(4);
        if let Ok(mut current) = self.size.lock() {
            if *current == (cols, rows) {
                return;
            }
            *current = (cols, rows);
        }

        if let Ok(master) = self.master.lock() {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }

        if let Ok(mut term) = self.term.lock() {
            term.resize(TermDimensions {
                cols: cols as usize,
                screen_lines: rows as usize,
            });
        }
        self.dirty_version.fetch_add(1, Ordering::Release);
    }

    pub fn current_size(&self) -> (u16, u16) {
        self.size.lock().map(|g| *g).unwrap_or((80, 24))
    }

    pub fn scroll_lines(&self, delta: i32) {
        if let Ok(mut term) = self.term.lock() {
            term.scroll_display(Scroll::Delta(delta));
        }
        self.dirty_version.fetch_add(1, Ordering::Release);
    }

    pub fn scroll_to_bottom(&self) {
        if let Ok(mut term) = self.term.lock() {
            term.scroll_display(Scroll::Bottom);
        }
        self.dirty_version.fetch_add(1, Ordering::Release);
    }

    pub fn display_offset(&self) -> usize {
        self.term
            .lock()
            .map(|t| t.grid().display_offset())
            .unwrap_or(0)
    }

    pub fn dirty_version(&self) -> u64 {
        self.dirty_version.load(Ordering::Acquire)
    }

    pub fn status(&self) -> SessionStatus {
        if self.exited.load(Ordering::Acquire) {
            return SessionStatus::Done;
        }
        let last = self.last_output_ms.load(Ordering::Acquire);
        let age = now_ms().saturating_sub(last);
        if age <= 3_000 {
            SessionStatus::Working
        } else {
            SessionStatus::Waiting
        }
    }

    pub fn elapsed_label(&self) -> String {
        let secs = self.created_at.elapsed().as_secs();
        if secs < 60 {
            format!("{secs}s")
        } else if secs < 3600 {
            format!("{}m", secs / 60)
        } else {
            format!("{}h", secs / 3600)
        }
    }

    pub fn refresh_context(&mut self) {
        self.git_branch = detect_git_branch(&self.cwd);
        self.tech_stack = detect_tech_stack(&self.cwd);
    }

    pub fn extract_text_selection(&self, start: (usize, usize), end: (usize, usize)) -> String {
        let ((sc, sr), (ec, er)) = normalize_selection(start, end);
        let Ok(term) = self.term.lock() else {
            return String::new();
        };
        let content = term.renderable_content();
        let display_offset = content.display_offset as i32;
        let screen_lines = term.screen_lines();
        let cols = term.columns();
        if screen_lines == 0 || cols == 0 {
            return String::new();
        }

        let mut grid_chars = vec![vec![' '; cols]; screen_lines];
        for indexed in content.display_iter {
            let r = indexed.point.line.0 + display_offset;
            let c = indexed.point.column.0;
            if r >= 0 && (r as usize) < screen_lines && c < cols {
                let ch = indexed.cell.c;
                grid_chars[r as usize][c] = if ch == '\0' { ' ' } else { ch };
            }
        }

        let mut lines = Vec::new();
        for r in sr..=er.min(screen_lines - 1) {
            let c_start = if r == sr { sc } else { 0 };
            let c_end = if r == er { ec.min(cols - 1) } else { cols - 1 };
            if c_start <= c_end && c_end < cols {
                let line: String = grid_chars[r][c_start..=c_end].iter().collect();
                lines.push(line.trim_end().to_string());
            }
        }
        lines.join("\n")
    }

    pub fn word_bounds_at(&self, col: usize, row: usize) -> Option<((usize, usize), (usize, usize))> {
        let Ok(term) = self.term.lock() else {
            return None;
        };
        let content = term.renderable_content();
        let display_offset = content.display_offset as i32;
        let screen_lines = term.screen_lines();
        let cols = term.columns();
        if row >= screen_lines || col >= cols {
            return None;
        }

        let mut line_chars = vec![' '; cols];
        for indexed in content.display_iter {
            let r = indexed.point.line.0 + display_offset;
            let c = indexed.point.column.0;
            if r == row as i32 && c < cols {
                line_chars[c] = indexed.cell.c;
            }
        }

        let is_word = |ch: char| !ch.is_whitespace() && ch != '\0';
        if !is_word(line_chars[col]) {
            return None;
        }
        let mut left = col;
        while left > 0 && is_word(line_chars[left - 1]) {
            left -= 1;
        }
        let mut right = col;
        while right + 1 < cols && is_word(line_chars[right + 1]) {
            right += 1;
        }
        Some(((left, row), (right, row)))
    }

    pub fn visible_rows_text(&self) -> Vec<String> {
        let Ok(term) = self.term.lock() else {
            return Vec::new();
        };
        let content = term.renderable_content();
        let display_offset = content.display_offset as i32;
        let screen_lines = term.screen_lines();
        let cols = term.columns();
        if screen_lines == 0 || cols == 0 {
            return Vec::new();
        }
        let mut grid_chars = vec![vec![' '; cols]; screen_lines];
        for indexed in content.display_iter {
            let r = indexed.point.line.0 + display_offset;
            let c = indexed.point.column.0;
            if r >= 0 && (r as usize) < screen_lines && c < cols {
                let ch = indexed.cell.c;
                grid_chars[r as usize][c] = if ch == '\0' { ' ' } else { ch };
            }
        }
        grid_chars
            .into_iter()
            .map(|row_chars| row_chars.into_iter().collect())
            .collect()
    }

    pub fn find_visible_matches(&self, query: &str) -> Vec<(usize, usize, usize)> {
        let rows = self.visible_rows_text();
        find_matches_in_rows(&rows, query)
    }

    pub fn url_at_cell(&self, col: usize, row: usize) -> Option<String> {
        let rows = self.visible_rows_text();
        let line = rows.get(row)?;
        extract_url_at_col(line, col)
    }
}

pub fn find_matches_in_rows(rows: &[String], query: &str) -> Vec<(usize, usize, usize)> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let needle_chars: Vec<char> = needle.chars().collect();
    let nlen = needle_chars.len();
    let mut matches = Vec::new();
    for (r_idx, row) in rows.iter().enumerate() {
        let row_chars: Vec<char> = row.to_lowercase().chars().collect();
        if row_chars.len() < nlen {
            continue;
        }
        let mut c = 0usize;
        while c + nlen <= row_chars.len() {
            if row_chars[c..c + nlen] == needle_chars[..] {
                matches.push((r_idx, c, c + nlen - 1));
                c += nlen;
            } else {
                c += 1;
            }
        }
    }
    matches
}

pub fn extract_url_at_col(line: &str, col: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    if col >= chars.len() {
        return None;
    }

    let mut i = 0usize;
    while i < chars.len() {
        let remaining: String = chars[i..].iter().take(8).collect();
        if remaining.starts_with("https://") || remaining.starts_with("http://") {
            let start = i;
            let mut end = i;
            while end < chars.len() && !chars[end].is_whitespace() {
                end += 1;
            }
            while end > start
                && matches!(
                    chars[end - 1],
                    ')' | ']' | '>' | ',' | '.' | ';' | '\'' | '"'
                )
            {
                end -= 1;
            }
            if end > start && col >= start && col < end {
                let url: String = chars[start..end].iter().collect();
                return Some(url);
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

pub fn normalize_selection(a: (usize, usize), b: (usize, usize)) -> ((usize, usize), (usize, usize)) {
    if a.1 < b.1 || (a.1 == b.1 && a.0 <= b.0) {
        (a, b)
    } else {
        (b, a)
    }
}

pub fn detect_git_branch(cwd: &Path) -> Option<String> {
    let mut cur = Some(cwd);
    while let Some(dir) = cur {
        let head = dir.join(".git").join("HEAD");
        if head.is_file() {
            let raw = std::fs::read_to_string(head).ok()?;
            let trimmed = raw.trim();
            if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
                return Some(branch.to_string());
            }
            if trimmed.len() >= 7 {
                return Some(trimmed[..7].to_string());
            }
            return None;
        }
        cur = dir.parent();
    }
    None
}

pub fn detect_tech_stack(cwd: &Path) -> Vec<&'static str> {
    let checks: [(&str, &str); 7] = [
        ("Cargo.toml", "Rust"),
        ("artisan", "Laravel"),
        ("composer.json", "PHP"),
        ("tsconfig.json", "TS"),
        ("package.json", "Node"),
        ("pyproject.toml", "Python"),
        ("go.mod", "Go"),
    ];
    let mut tags = Vec::new();
    for (file, label) in checks {
        if cwd.join(file).exists() {
            tags.push(label);
            if tags.len() >= 3 {
                break;
            }
        }
    }
    if tags.is_empty() && cwd.join("requirements.txt").exists() {
        tags.push("Python");
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::grid::Indexed;
    use std::time::Duration;

    #[test]
    fn conpty_powershell_outputs_marker() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let session = TerminalSession::spawn(
            SessionId(1),
            "Test Shell",
            "powershell",
            "powershell.exe",
            &["-NoLogo".to_string()],
            cwd,
            100,
            30,
        )
        .expect("Failed to spawn ConPTY PowerShell session");

        std::thread::sleep(Duration::from_millis(700));
        session.write_bytes(b"echo TERMINALPLUS_OK\r");

        let mut found = false;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(150));
            if let Ok(term) = session.term.lock() {
                let mut text = String::new();
                for Indexed { cell, .. } in term.grid().display_iter() {
                    text.push(cell.c);
                }
                if text.contains("TERMINALPLUS_OK") {
                    found = true;
                    break;
                }
            }
        }

        session.write_bytes(b"exit\r");
        assert!(found, "Expected TERMINALPLUS_OK to appear in ConPTY grid");
    }

    #[test]
    fn detects_rust_stack_in_workspace() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let stack = detect_tech_stack(&cwd);
        assert!(stack.contains(&"Rust"));
    }

    #[test]
    fn finds_case_insensitive_matches_in_rows() {
        let rows = vec![
            "Running Cargo test on Velt".to_string(),
            "VELT v0.1.0 ready".to_string(),
        ];
        let matches = find_matches_in_rows(&rows, "velt");
        assert_eq!(matches, vec![(0, 22, 25), (1, 0, 3)]);
    }

    #[test]
    fn extracts_url_at_column_and_trims_punctuation() {
        let line = "See docs at (https://github.com/jregnaultt/velt). Thanks!";
        assert_eq!(
            extract_url_at_col(line, 15).as_deref(),
            Some("https://github.com/jregnaultt/velt")
        );
        assert_eq!(extract_url_at_col(line, 4), None);
    }
}
