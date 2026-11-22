use chrono::{DateTime, Local, Utc};
use serde_json::{json, Value};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub last_updated: DateTime<Utc>,
    pub last_updated_label: String,
    pub is_live: bool,
    pub turn_count: usize,
    pub last_model: String,
    pub workspace_hint: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct ChatMessageItem {
    pub role: ChatRole,
    pub model: Option<String>,
    pub timestamp: String,
    pub content: String,
    pub thinking: Option<String>,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkStatus {
    Ready(String),
    MissingSdk(String),
    MissingPython,
}

pub fn extract_clean_user_request(raw: &str) -> String {
    if let Some(start) = raw.find("<USER_REQUEST>") {
        let after = &raw[start + "<USER_REQUEST>".len()..];
        let inner = if let Some(end) = after.find("</USER_REQUEST>") {
            &after[..end]
        } else {
            after
        };
        return inner.trim().to_string();
    }
    raw.trim().to_string()
}

fn format_relative(dt: DateTime<Utc>) -> String {
    let secs = (Utc::now() - dt).num_seconds().max(0);
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

fn extract_workspace_hint_from_text(text: &str) -> Option<PathBuf> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Directory:") {
            let p = PathBuf::from(rest.trim());
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    None
}

pub fn list_antigravity_conversations() -> Vec<ConversationSummary> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let brain_dir = home.join(".gemini").join("antigravity").join("brain");
    let Ok(entries) = std::fs::read_dir(brain_dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let conv_dir = entry.path();
        if !conv_dir.is_dir() {
            continue;
        }
        let Some(conv_id) = conv_dir.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let transcript = conv_dir
            .join(".system_generated")
            .join("logs")
            .join("transcript.jsonl");
        if !transcript.is_file() {
            continue;
        }
        let Ok(file) = File::open(&transcript) else {
            continue;
        };

        let mut title = String::new();
        let mut last_updated: Option<DateTime<Utc>> = None;
        let mut turn_count = 0usize;
        let mut last_model = "Gemini".to_string();
        let mut workspace_hint: Option<PathBuf> = None;

        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(ts) = v
                .get("created_at")
                .and_then(|x| x.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
            {
                last_updated = Some(ts);
            }
            let step_type = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
            if step_type == "USER_INPUT" {
                if title.is_empty() {
                    if let Some(raw) = v.get("content").and_then(|x| x.as_str()) {
                        let clean = extract_clean_user_request(raw);
                        let first_line = clean.lines().next().unwrap_or("").trim();
                        if !first_line.is_empty() {
                            title = if first_line.chars().count() > 52 {
                                let t: String = first_line.chars().take(51).collect();
                                format!("{t}...")
                            } else {
                                first_line.to_string()
                            };
                        }
                    }
                }
            } else if step_type == "PLANNER_RESPONSE" {
                turn_count += 1;
                if let Some(m) = v.get("model").and_then(|x| x.as_str()) {
                    if !m.is_empty() {
                        last_model = m.to_string();
                    }
                }
            }
            if workspace_hint.is_none() {
                if let Some(c) = v.get("content").and_then(|x| x.as_str()) {
                    workspace_hint = extract_workspace_hint_from_text(c);
                }
            }
        }

        if let Some(updated) = last_updated {
            if title.is_empty() {
                title = format!("Session {}", &conv_id[..conv_id.len().min(8)]);
            }
            let is_live = (Utc::now() - updated).num_seconds().abs() <= 90;
            out.push(ConversationSummary {
                id: conv_id.to_string(),
                title,
                last_updated: updated,
                last_updated_label: format_relative(updated),
                is_live,
                turn_count,
                last_model,
                workspace_hint,
            });
        }
    }

    out.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
    out
}

pub fn load_conversation_messages(conv_id: &str) -> Vec<ChatMessageItem> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let transcript = home
        .join(".gemini")
        .join("antigravity")
        .join("brain")
        .join(conv_id)
        .join(".system_generated")
        .join("logs")
        .join("transcript.jsonl");
    let Ok(file) = File::open(transcript) else {
        return Vec::new();
    };
    parse_conversation_reader(BufReader::new(file))
}

pub fn parse_conversation_reader<R: BufRead>(reader: R) -> Vec<ChatMessageItem> {
    let mut messages = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        let step_type = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        let ts_label = v
            .get("created_at")
            .and_then(|x| x.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Local).format("%H:%M").to_string())
            .unwrap_or_default();

        if step_type == "USER_INPUT" {
            let raw = v.get("content").and_then(|x| x.as_str()).unwrap_or("");
            let clean = extract_clean_user_request(raw);
            if !clean.is_empty() {
                messages.push(ChatMessageItem {
                    role: ChatRole::User,
                    model: None,
                    timestamp: ts_label,
                    content: clean,
                    thinking: None,
                    tools: Vec::new(),
                });
            }
        } else if step_type == "PLANNER_RESPONSE" {
            let content = v
                .get("content")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let thinking = v
                .get("thinking")
                .and_then(|x| x.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| {
                    if s.chars().count() > 260 {
                        let t: String = s.chars().take(259).collect();
                        format!("{t}...")
                    } else {
                        s.to_string()
                    }
                });
            let model = v
                .get("model")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            let mut tools = Vec::new();
            if let Some(arr) = v.get("tool_calls").and_then(|x| x.as_array()) {
                for tc in arr {
                    if let Some(name) = tc.get("name").and_then(|x| x.as_str()) {
                        let short = name.strip_prefix("default_api:").unwrap_or(name);
                        tools.push(short.to_string());
                    }
                }
            }

            if !content.is_empty() || !tools.is_empty() {
                messages.push(ChatMessageItem {
                    role: ChatRole::Assistant,
                    model,
                    timestamp: ts_label,
                    content,
                    thinking,
                    tools,
                });
            }
        }
    }

    messages
}

pub fn check_sdk_status() -> SdkStatus {
    for exe in ["python", "python3", "py"] {
        let mut ver_cmd = Command::new(exe);
        ver_cmd
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(target_os = "windows")]
        ver_cmd.creation_flags(CREATE_NO_WINDOW);

        let Ok(ver_out) = ver_cmd.output() else {
            continue;
        };
        if !ver_out.status.success() {
            continue;
        }

        let mut sdk_cmd = Command::new(exe);
        sdk_cmd
            .args(["-c", "import google.antigravity; print('OK')"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(target_os = "windows")]
        sdk_cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(sdk_out) = sdk_cmd.output() {
            let stdout = String::from_utf8_lossy(&sdk_out.stdout);
            if sdk_out.status.success() && stdout.contains("OK") {
                return SdkStatus::Ready(exe.to_string());
            }
        }
        return SdkStatus::MissingSdk(exe.to_string());
    }
    SdkStatus::MissingPython
}

const BRIDGE_SCRIPT: &str = r#"import asyncio
import json
import sys
from google.antigravity import Agent, LocalAgentConfig, CapabilitiesConfig

async def main():
    payload = json.loads(sys.stdin.read() or "{}")
    prompt = payload.get("prompt", "")
    config = LocalAgentConfig(
        system_instructions="You are Antigravity running inside Velt.",
        capabilities=CapabilitiesConfig(),
    )
    chunks = []
    thoughts = []
    tools = []
    async with Agent(config) as agent:
        response = await agent.chat(prompt)
        async for token in response:
            chunks.append(token)
        if hasattr(response, "thoughts"):
            try:
                async for th in response.thoughts:
                    thoughts.append(str(th))
            except Exception:
                pass
        if hasattr(response, "tool_calls"):
            try:
                async for tc in response.tool_calls:
                    tools.append(getattr(tc, "name", str(tc)))
            except Exception:
                pass
    print(json.dumps({
        "content": "".join(chunks),
        "thinking": "".join(thoughts) if thoughts else None,
        "tools": tools,
    }))

if __name__ == "__main__":
    asyncio.run(main())
"#;

pub fn send_sdk_chat_prompt(
    python_exe: &str,
    cwd: &Path,
    prompt: &str,
) -> Result<ChatMessageItem, String> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let tp_dir = home.join(".velt");
    let _ = std::fs::create_dir_all(&tp_dir);
    let script_path = tp_dir.join("agy_bridge.py");
    std::fs::write(&script_path, BRIDGE_SCRIPT)
        .map_err(|e| format!("Failed to write SDK bridge script: {e}"))?;

    let mut cmd = Command::new(python_exe);
    cmd.arg(&script_path)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to run {python_exe}: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = json!({ "prompt": prompt }).to_string();
        let _ = stdin.write_all(payload.as_bytes());
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("SDK bridge wait failed: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Antigravity SDK error: {}", err.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("Invalid SDK bridge output ({e}): {}", stdout.trim()))?;

    let content = parsed
        .get("content")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let thinking = parsed
        .get("thinking")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let tools = parsed
        .get("tools")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(ChatMessageItem {
        role: ChatRole::Assistant,
        model: Some("Antigravity SDK".to_string()),
        timestamp: Local::now().format("%H:%M").to_string(),
        content,
        thinking,
        tools,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_user_request_tags() {
        let raw = "<USER_REQUEST>\nhola antigravity\n</USER_REQUEST>\n<ADDITIONAL_METADATA>...</ADDITIONAL_METADATA>";
        assert_eq!(extract_clean_user_request(raw), "hola antigravity");
        assert_eq!(extract_clean_user_request("plain message"), "plain message");
    }

    #[test]
    fn parses_antigravity_conversation_timeline() {
        let jsonl = concat!(
            r#"{"step_index":0,"type":"USER_INPUT","created_at":"2026-11-22T18:00:00Z","content":"<USER_REQUEST>\nbuild terminal\n</USER_REQUEST>"}"#,
            "\n",
            r#"{"step_index":1,"type":"PLANNER_RESPONSE","created_at":"2026-11-22T18:00:02Z","model":"gemini-4-neon-medium","thinking":"Checking workspace...","tool_calls":[{"name":"default_api:run_command"}],"content":"Done!"}"#,
            "\n"
        );
        let msgs = parse_conversation_reader(std::io::Cursor::new(jsonl));
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, ChatRole::User);
        assert_eq!(msgs[0].content, "build terminal");
        assert_eq!(msgs[1].role, ChatRole::Assistant);
        assert_eq!(msgs[1].model.as_deref(), Some("gemini-4-neon-medium"));
        assert_eq!(msgs[1].tools, vec!["run_command".to_string()]);
        assert_eq!(msgs[1].content, "Done!");
    }
}
