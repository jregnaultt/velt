use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    Antigravity,
    ClaudeCode,
    Codex,
    GeminiCli,
    OpenCode,
    Ollama,
    GrokCli,
}

#[derive(Debug, Clone)]
pub struct DetectedAgent {
    pub name: String,
    pub short_name: &'static str,
    #[allow(dead_code)]
    pub kind: AgentKind,
    pub installed: bool,
    #[allow(dead_code)]
    pub detail: String,
}

fn find_cli(names: &[&str]) -> bool {
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in names {
                if dir.join(name).is_file() {
                    return true;
                }
            }
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let npm_dir = PathBuf::from(appdata).join("npm");
        for name in names {
            if npm_dir.join(name).is_file() {
                return true;
            }
        }
    }
    false
}

pub fn scan_installed_agents() -> Vec<DetectedAgent> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    // Antigravity
    let agy_dir = home.join(".gemini").join("antigravity");
    let agy_brain = agy_dir.join("brain");
    let agy_conv_count = std::fs::read_dir(&agy_brain)
        .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count())
        .unwrap_or(0);
    let agy_installed = agy_dir.exists() || find_cli(&["agy.exe", "agy.cmd", "agy"]);

    // Codex
    let codex_dir = home.join(".codex");
    let codex_cli = find_cli(&["codex.exe", "codex.cmd", "codex"]);
    let codex_installed = codex_dir.exists() || codex_cli;

    // Claude Code
    let claude_dir = home.join(".claude");
    let claude_cli = find_cli(&["claude.exe", "claude.cmd", "claude"]);
    let claude_installed = claude_dir.exists() || claude_cli;

    // Gemini CLI
    let gemini_cli = find_cli(&["gemini.exe", "gemini.cmd", "gemini"]);

    // OpenCode
    let opencode_dir = home.join(".opencode");
    let opencode_cli = find_cli(&["opencode.exe", "opencode.cmd", "opencode"]);
    let opencode_installed = opencode_dir.exists() || opencode_cli;

    // Ollama
    let ollama_dir = home.join(".ollama");
    let ollama_cli = find_cli(&["ollama.exe", "ollama"]);
    let ollama_installed = ollama_dir.exists() || ollama_cli;

    // Grok CLI
    let grok_dir = home.join(".grok");
    let grok_cli = find_cli(&["grok.exe", "grok.cmd", "grok"]);
    let grok_installed = grok_dir.exists() || grok_cli;

    vec![
        DetectedAgent {
            name: "Antigravity".to_string(),
            short_name: "AGY",
            kind: AgentKind::Antigravity,
            installed: agy_installed,
            detail: if agy_conv_count > 0 {
                format!("{agy_conv_count} logs")
            } else if agy_installed {
                "installed".to_string()
            } else {
                "not found".to_string()
            },
        },
        DetectedAgent {
            name: "Codex".to_string(),
            short_name: "Codex",
            kind: AgentKind::Codex,
            installed: codex_installed,
            detail: if codex_cli {
                "CLI + config".to_string()
            } else if codex_dir.exists() {
                "local config".to_string()
            } else {
                "not found".to_string()
            },
        },
        DetectedAgent {
            name: "Claude Code".to_string(),
            short_name: "Claude",
            kind: AgentKind::ClaudeCode,
            installed: claude_installed,
            detail: if claude_installed {
                "installed".to_string()
            } else {
                "not found".to_string()
            },
        },
        DetectedAgent {
            name: "Gemini CLI".to_string(),
            short_name: "Gemini",
            kind: AgentKind::GeminiCli,
            installed: gemini_cli,
            detail: if gemini_cli { "CLI" } else { "not found" }.to_string(),
        },
        DetectedAgent {
            name: "opencode".to_string(),
            short_name: "OpenCode",
            kind: AgentKind::OpenCode,
            installed: opencode_installed,
            detail: if opencode_installed {
                "installed"
            } else {
                "not found"
            }
            .to_string(),
        },
        DetectedAgent {
            name: "Ollama".to_string(),
            short_name: "Ollama",
            kind: AgentKind::Ollama,
            installed: ollama_installed,
            detail: if ollama_installed {
                "local models"
            } else {
                "not found"
            }
            .to_string(),
        },
        DetectedAgent {
            name: "Grok CLI".to_string(),
            short_name: "Grok",
            kind: AgentKind::GrokCli,
            installed: grok_installed,
            detail: if grok_installed {
                "installed"
            } else {
                "not found"
            }
            .to_string(),
        },
    ]
}
