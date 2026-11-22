use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShellKind {
    GitBash,
    Wsl,
    Cmd,
    PowerShell,
    Pwsh,
    Custom,
}

impl ShellKind {
    pub const ALL: [ShellKind; 6] = [
        ShellKind::GitBash,
        ShellKind::Wsl,
        ShellKind::Cmd,
        ShellKind::PowerShell,
        ShellKind::Pwsh,
        ShellKind::Custom,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ShellKind::GitBash => "Git Bash",
            ShellKind::Wsl => "WSL (Ubuntu / Linux)",
            ShellKind::Cmd => "Command Prompt (CMD)",
            ShellKind::PowerShell => "Windows PowerShell",
            ShellKind::Pwsh => "PowerShell 7 (pwsh)",
            ShellKind::Custom => "Custom executable",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            ShellKind::GitBash => "bash",
            ShellKind::Wsl => "wsl",
            ShellKind::Cmd => "cmd",
            ShellKind::PowerShell => "powershell",
            ShellKind::Pwsh => "pwsh",
            ShellKind::Custom => "shell",
        }
    }

    pub fn resolve_command(&self, custom_path: &str) -> (String, Vec<String>) {
        match self {
            ShellKind::GitBash => {
                let candidates = [
                    r"C:\Program Files\Git\bin\bash.exe",
                    r"C:\Program Files\Git\usr\bin\bash.exe",
                    r"C:\Program Files (x86)\Git\bin\bash.exe",
                ];
                let cmd = candidates
                    .iter()
                    .find(|p| std::path::Path::new(p).exists())
                    .map(|s| (*s).to_string())
                    .unwrap_or_else(|| "bash.exe".to_string());
                (cmd, vec!["--login".to_string(), "-i".to_string()])
            }
            ShellKind::Wsl => ("wsl.exe".to_string(), Vec::new()),
            ShellKind::Cmd => ("cmd.exe".to_string(), Vec::new()),
            ShellKind::PowerShell => ("powershell.exe".to_string(), vec!["-NoLogo".to_string()]),
            ShellKind::Pwsh => ("pwsh.exe".to_string(), vec!["-NoLogo".to_string()]),
            ShellKind::Custom => {
                let trimmed = custom_path.trim();
                if trimmed.is_empty() {
                    ("powershell.exe".to_string(), vec!["-NoLogo".to_string()])
                } else {
                    (trimmed.to_string(), Vec::new())
                }
            }
        }
    }

    pub fn is_available(&self, custom_path: &str) -> bool {
        let (cmd, _) = self.resolve_command(custom_path);
        let p = std::path::Path::new(&cmd);
        if p.is_absolute() {
            return p.exists();
        }
        find_in_path(&cmd)
    }
}

fn find_in_path(exe_name: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(exe_name);
        if candidate.is_file() {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TerminalTheme {
    MidtermDark,
    TokyoNight,
    CatppuccinMocha,
}

impl TerminalTheme {
    pub const ALL: [TerminalTheme; 3] = [
        TerminalTheme::MidtermDark,
        TerminalTheme::TokyoNight,
        TerminalTheme::CatppuccinMocha,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            TerminalTheme::MidtermDark => "Midterm Dark",
            TerminalTheme::TokyoNight => "Tokyo Night",
            TerminalTheme::CatppuccinMocha => "Catppuccin Mocha",
        }
    }
}

impl Default for TerminalTheme {
    fn default() -> Self {
        TerminalTheme::MidtermDark
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationsConfig {
    pub jira_domain: String,
    pub jira_email: String,
    pub jira_api_token: String,
    pub jira_oauth_client_id: String,
    pub jira_oauth_client_secret: String,
    pub jira_oauth_access_token: String,
    pub jira_cloud_id: String,
    pub bitbucket_workspace: String,
    pub bitbucket_username: String,
    pub bitbucket_app_password: String,
    pub github_token: String,
    pub github_username: String,
    pub github_oauth_client_id: String,
    pub github_oauth_client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
#[allow(dead_code)]
pub struct RemoteProfile {
    pub name: String,
    pub user: String,
    pub host: String,
    pub port: u16,
    pub extra_args: String,
}

impl Default for RemoteProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            user: String::new(),
            host: String::new(),
            port: 22,
            extra_args: String::new(),
        }
    }
}

impl RemoteProfile {
    #[allow(dead_code)]
    pub fn ssh_command(&self) -> (String, Vec<String>) {
        let mut args = vec!["-t".to_string()];
        if self.port != 0 && self.port != 22 {
            args.push("-p".to_string());
            args.push(self.port.to_string());
        }
        for part in self.extra_args.split_whitespace() {
            args.push(part.to_string());
        }
        let target = if self.user.trim().is_empty() {
            self.host.trim().to_string()
        } else {
            format!("{}@{}", self.user.trim(), self.host.trim())
        };
        args.push(target);
        ("ssh.exe".to_string(), args)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub default_shell: ShellKind,
    pub custom_shell_path: String,
    pub font_size: f32,
    pub terminal_theme: TerminalTheme,
    pub integrations: IntegrationsConfig,
    #[allow(dead_code)]
    pub remote_profiles: Vec<RemoteProfile>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_shell = if ShellKind::GitBash.is_available("") {
            ShellKind::GitBash
        } else {
            ShellKind::PowerShell
        };
        Self {
            default_shell,
            custom_shell_path: String::new(),
            font_size: 13.5,
            terminal_theme: TerminalTheme::MidtermDark,
            integrations: IntegrationsConfig::default(),
            remote_profiles: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".velt")
            .join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<AppConfig>(&raw) {
                return cfg;
            }
        }
        let legacy_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".terminalplus")
            .join("config.json");
        if let Ok(raw) = std::fs::read_to_string(&legacy_path) {
            if let Ok(cfg) = serde_json::from_str::<AppConfig>(&raw) {
                cfg.save();
                return cfg;
            }
        }
        let cfg = Self::default();
        cfg.save();
        cfg
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_profile_builds_ssh_args() {
        let p = RemoteProfile {
            name: "Work PC".to_string(),
            user: "regnault".to_string(),
            host: "100.64.0.12".to_string(),
            port: 2222,
            extra_args: "-o ServerAliveInterval=30".to_string(),
        };
        let (cmd, args) = p.ssh_command();
        assert_eq!(cmd, "ssh.exe");
        assert_eq!(
            args,
            vec![
                "-t",
                "-p",
                "2222",
                "-o",
                "ServerAliveInterval=30",
                "regnault@100.64.0.12"
            ]
        );
    }
}
