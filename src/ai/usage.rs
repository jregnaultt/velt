use crate::ai::scanner::DetectedAgent;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Timelike, Utc};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageRange {
    Today,
    Days7,
    Days30,
    Days90,
    All,
}

impl UsageRange {
    pub fn days(&self) -> Option<i64> {
        match self {
            UsageRange::Today => Some(1),
            UsageRange::Days7 => Some(7),
            UsageRange::Days30 => Some(30),
            UsageRange::Days90 => Some(90),
            UsageRange::All => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UsageEvent {
    pub timestamp: DateTime<Utc>,
    pub agent: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub output_tokens: u64,
    pub duration_secs: u64,
    pub conversation_id: String,
}

#[derive(Debug, Clone)]
pub struct DailyBucket {
    #[allow(dead_code)]
    pub date: NaiveDate,
    pub label: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tasks: usize,
    pub duration_secs: u64,
}

#[derive(Debug, Clone)]
pub struct ModelSummary {
    pub model: String,
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tasks: usize,
}

#[derive(Debug, Clone)]
pub struct AgentSummary {
    pub agent: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub duration_secs: u64,
    #[allow(dead_code)]
    pub installed: bool,
}

#[derive(Debug, Clone)]
pub struct UsageReport {
    pub range_label: String,
    pub total_input_tokens: u64,
    pub total_cached_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tasks: usize,
    pub total_turns: usize,
    pub total_agent_secs: u64,
    pub active_days: usize,
    pub total_window_days: usize,
    pub avg_tokens_per_day: u64,
    pub daily_series: Vec<DailyBucket>,
    pub heatmap: [[u64; 24]; 7],
    pub most_active_label: String,
    pub models: Vec<ModelSummary>,
    pub agents: Vec<AgentSummary>,
    pub providers: Vec<(String, f32)>,
}

impl UsageReport {
    pub fn build(
        events: &[UsageEvent],
        detected: &[DetectedAgent],
        range: UsageRange,
        selected_agent: Option<&str>,
    ) -> Self {
        let now = Utc::now();
        let today = Local::now().date_naive();
        let cutoff = match range {
            UsageRange::Today => {
                use chrono::TimeZone;
                today
                    .and_hms_opt(0, 0, 0)
                    .and_then(|naive| Local.from_local_datetime(&naive).earliest())
                    .map(|dt| dt.with_timezone(&Utc))
            }
            _ => range.days().map(|d| now - Duration::days(d)),
        };

        let filtered_all_agents: Vec<&UsageEvent> = events
            .iter()
            .filter(|e| cutoff.map_or(true, |c| e.timestamp >= c))
            .collect();

        let filtered: Vec<&UsageEvent> = filtered_all_agents
            .iter()
            .copied()
            .filter(|e| selected_agent.map_or(true, |a| e.agent == a))
            .collect();

        let mut total_input_tokens = 0u64;
        let mut total_cached_tokens = 0u64;
        let mut total_output_tokens = 0u64;
        let mut total_agent_secs = 0u64;
        let mut conv_ids: HashSet<(&str, &str)> = HashSet::new();

        let mut by_day: BTreeMap<NaiveDate, (u64, u64, HashSet<(&str, &str)>, u64)> =
            BTreeMap::new();
        let mut heatmap = [[0u64; 24]; 7];

        let mut model_map: HashMap<String, (String, u64, u64, HashSet<(&str, &str)>)> =
            HashMap::new();
        let mut provider_totals: HashMap<String, u64> = HashMap::new();

        for ev in &filtered {
            total_input_tokens = total_input_tokens.saturating_add(ev.input_tokens);
            total_cached_tokens = total_cached_tokens.saturating_add(ev.cached_tokens);
            total_output_tokens = total_output_tokens.saturating_add(ev.output_tokens);
            total_agent_secs = total_agent_secs.saturating_add(ev.duration_secs);
            let conv_key = (ev.agent.as_str(), ev.conversation_id.as_str());
            conv_ids.insert(conv_key);

            let local_dt = ev.timestamp.with_timezone(&Local);
            let date = local_dt.date_naive();
            let day_entry = by_day
                .entry(date)
                .or_insert_with(|| (0, 0, HashSet::new(), 0));
            day_entry.0 = day_entry.0.saturating_add(ev.input_tokens);
            day_entry.1 = day_entry.1.saturating_add(ev.output_tokens);
            day_entry.2.insert(conv_key);
            day_entry.3 = day_entry.3.saturating_add(ev.duration_secs);

            let weekday = local_dt.weekday().num_days_from_monday() as usize;
            let hour = local_dt.hour() as usize;
            if weekday < 7 && hour < 24 {
                heatmap[weekday][hour] = heatmap[weekday][hour]
                    .saturating_add(ev.input_tokens.saturating_add(ev.output_tokens).max(1));
            }

            let m_entry = model_map.entry(ev.model.clone()).or_insert_with(|| {
                (ev.provider.clone(), 0, 0, HashSet::new())
            });
            m_entry.1 = m_entry.1.saturating_add(ev.input_tokens);
            m_entry.2 = m_entry.2.saturating_add(ev.output_tokens);
            m_entry.3.insert(conv_key);

            let p_total = provider_totals.entry(ev.provider.clone()).or_insert(0);
            *p_total = p_total.saturating_add(ev.input_tokens.saturating_add(ev.output_tokens));
        }

        // Fill hourly buckets for Today, or contiguous daily buckets for multi-day ranges
        let (daily_series, start_date, total_window_days, range_label) =
            if range == UsageRange::Today {
                let mut by_hour: [(u64, u64, HashSet<(&str, &str)>, u64); 24] =
                    std::array::from_fn(|_| (0, 0, HashSet::new(), 0));
                for ev in &filtered {
                    let local_dt = ev.timestamp.with_timezone(&Local);
                    let h = (local_dt.hour() as usize).min(23);
                    by_hour[h].0 = by_hour[h].0.saturating_add(ev.input_tokens);
                    by_hour[h].1 = by_hour[h].1.saturating_add(ev.output_tokens);
                    by_hour[h]
                        .2
                        .insert((ev.agent.as_str(), ev.conversation_id.as_str()));
                    by_hour[h].3 = by_hour[h].3.saturating_add(ev.duration_secs);
                }
                let series: Vec<DailyBucket> = by_hour
                    .into_iter()
                    .enumerate()
                    .map(|(h, (inp, out, tasks, secs))| DailyBucket {
                        date: today,
                        label: format!("{h:02}h"),
                        input_tokens: inp,
                        output_tokens: out,
                        tasks: tasks.len(),
                        duration_secs: secs,
                    })
                    .collect();
                (series, today, 1usize, "Today (hourly)".to_string())
            } else {
                let start_date = match range.days() {
                    Some(d) => today - Duration::days(d - 1),
                    None => by_day.keys().next().copied().unwrap_or(today),
                };
                let total_window_days = ((today - start_date).num_days().max(0) + 1) as usize;
                let mut series = Vec::with_capacity(total_window_days.min(365));
                let step_start = if total_window_days > 120 {
                    today - Duration::days(119)
                } else {
                    start_date
                };
                let mut cursor = step_start;
                while cursor <= today {
                    let (inp, out, tasks, secs) = by_day
                        .get(&cursor)
                        .map(|v| (v.0, v.1, v.2.len(), v.3))
                        .unwrap_or((0, 0, 0, 0));
                    series.push(DailyBucket {
                        date: cursor,
                        label: cursor.format("%b %d").to_string(),
                        input_tokens: inp,
                        output_tokens: out,
                        tasks,
                        duration_secs: secs,
                    });
                    cursor += Duration::days(1);
                }
                let label = format!("{} - today", start_date.format("%b %d"));
                (series, start_date, total_window_days, label)
            };
        let _ = start_date;

        let active_days = by_day.len();
        let avg_tokens_per_day = if active_days > 0 {
            total_input_tokens.saturating_add(total_output_tokens) / active_days as u64
        } else {
            0
        };

        let mut best_day = 0usize;
        let mut best_hour = 0usize;
        let mut best_val = 0u64;
        for (d, row) in heatmap.iter().enumerate() {
            for (h, &v) in row.iter().enumerate() {
                if v > best_val {
                    best_val = v;
                    best_day = d;
                    best_hour = h;
                }
            }
        }
        let day_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        let most_active_label = if best_val > 0 {
            format!("most active {} {:02} h", day_names[best_day], best_hour)
        } else {
            "no activity yet".to_string()
        };

        let mut models: Vec<ModelSummary> = model_map
            .into_iter()
            .map(|(model, (provider, input_tokens, output_tokens, tasks))| ModelSummary {
                model,
                provider,
                input_tokens,
                output_tokens,
                tasks: tasks.len(),
            })
            .collect();
        models.sort_by(|a, b| {
            (b.input_tokens + b.output_tokens).cmp(&(a.input_tokens + a.output_tokens))
        });

        let total_provider_tokens: u64 = provider_totals.values().sum();
        let mut providers: Vec<(String, f32)> = if total_provider_tokens > 0 {
            provider_totals
                .into_iter()
                .map(|(p, v)| (p, (v as f32 / total_provider_tokens as f32) * 100.0))
                .collect()
        } else {
            Vec::new()
        };
        providers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Agent list always reflects all agents in the selected date window so user can click/toggle
        let mut agent_map: HashMap<String, (u64, u64, u64, bool)> = HashMap::new();
        for d in detected {
            if d.installed {
                agent_map.insert(d.name.clone(), (0, 0, 0, true));
            }
        }
        for ev in &filtered_all_agents {
            let entry = agent_map
                .entry(ev.agent.clone())
                .or_insert((0, 0, 0, true));
            entry.0 = entry.0.saturating_add(ev.input_tokens);
            entry.1 = entry.1.saturating_add(ev.output_tokens);
            entry.2 = entry.2.saturating_add(ev.duration_secs);
            entry.3 = true;
        }
        let mut agents: Vec<AgentSummary> = agent_map
            .into_iter()
            .map(|(agent, (input_tokens, output_tokens, duration_secs, installed))| AgentSummary {
                agent,
                input_tokens,
                output_tokens,
                duration_secs,
                installed,
            })
            .collect();
        agents.sort_by(|a, b| {
            (b.input_tokens + b.output_tokens)
                .cmp(&(a.input_tokens + a.output_tokens))
                .then_with(|| a.agent.cmp(&b.agent))
        });

        Self {
            range_label,
            total_input_tokens,
            total_cached_tokens,
            total_output_tokens,
            total_tasks: conv_ids.len(),
            total_turns: filtered.len(),
            total_agent_secs,
            active_days,
            total_window_days,
            avg_tokens_per_day,
            daily_series,
            heatmap,
            most_active_label,
            models,
            agents,
            providers,
        }
    }
}

pub fn format_compact(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn format_duration(secs: u64) -> String {
    if secs == 0 {
        return "0m".to_string();
    }
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    if hours > 0 {
        format!("{hours} h")
    } else if mins > 0 {
        format!("{mins} min")
    } else {
        format!("{secs} s")
    }
}

pub fn load_all_usage_events() -> Vec<UsageEvent> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut events = Vec::new();
    load_antigravity_events(&home, &mut events);
    load_claude_events(&home, &mut events);
    load_codex_events(&home, &mut events);
    events
}

fn infer_provider_from_model(model: &str) -> String {
    let lower = model.to_lowercase();
    if lower.contains("claude") || lower.contains("opus") || lower.contains("sonnet") || lower.contains("haiku") || lower.contains("fable") {
        "Anthropic".to_string()
    } else if lower.contains("gemini") || lower.contains("antigravity") {
        "Google".to_string()
    } else if lower.contains("gpt") || lower.contains("codex") || lower.contains("o1") || lower.contains("o3") || lower.contains("o4") {
        "OpenAI".to_string()
    } else if lower.contains("grok") {
        "xAI".to_string()
    } else {
        "Other".to_string()
    }
}

fn extract_model_selection(content: &str) -> Option<String> {
    let marker = "Model Selection` from ";
    let idx = content.find(marker)?;
    let rest = &content[idx + marker.len()..];
    let to_idx = rest.find(" to ")?;
    let after_to = &rest[to_idx + 4..];
    let end = after_to
        .find(". ")
        .or_else(|| after_to.find('\n'))
        .or_else(|| after_to.find('<'))
        .unwrap_or(after_to.len());
    let name = after_to[..end].trim().trim_end_matches('.');
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn load_antigravity_events(home: &Path, out: &mut Vec<UsageEvent>) {
    let brain_dir = home.join(".gemini").join("antigravity").join("brain");
    let Ok(entries) = std::fs::read_dir(brain_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let conv_dir = entry.path();
        if !conv_dir.is_dir() {
            continue;
        }
        let conv_id = conv_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

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
        let reader = BufReader::new(file);
        parse_antigravity_reader(&conv_id, reader, out);
    }
}

fn parse_antigravity_reader<R: BufRead>(conv_id: &str, reader: R, out: &mut Vec<UsageEvent>) {
    let mut current_model = "Gemini (Antigravity)".to_string();
    let mut prev_ts: Option<DateTime<Utc>> = None;

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        let step_type = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        let ts = v
            .get("created_at")
            .and_then(|x| x.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        if step_type == "USER_INPUT" {
            if let Some(content) = v.get("content").and_then(|x| x.as_str()) {
                if let Some(sel) = extract_model_selection(content) {
                    current_model = sel;
                }
            }
        }

        if step_type == "PLANNER_RESPONSE" {
            if let Some(timestamp) = ts {
                let duration_secs = prev_ts
                    .map(|p| (timestamp - p).num_seconds().clamp(1, 300) as u64)
                    .unwrap_or(5);

                let step_model = v
                    .get("model")
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| current_model.clone());

                let cached_tokens = v
                    .get("cache_read_tokens")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let uncached_input = v.get("input_tokens").and_then(|x| x.as_u64());
                let input_tokens = uncached_input
                    .map(|u| u.saturating_add(cached_tokens))
                    .unwrap_or_else(|| {
                        let c_len = v
                            .get("content")
                            .and_then(|x| x.as_str())
                            .map(|s| s.len() as u64)
                            .unwrap_or(0);
                        (c_len / 4).max(100)
                    });
                let output_tokens = v
                    .get("output_tokens")
                    .and_then(|x| x.as_u64())
                    .unwrap_or_else(|| {
                        let c_len = v
                            .get("content")
                            .and_then(|x| x.as_str())
                            .map(|s| s.len() as u64)
                            .unwrap_or(0);
                        let t_len = v
                            .get("thinking")
                            .and_then(|x| x.as_str())
                            .map(|s| s.len() as u64)
                            .unwrap_or(0);
                        ((c_len + t_len) / 4).max(1)
                    });

                let provider = infer_provider_from_model(&step_model);
                out.push(UsageEvent {
                    timestamp,
                    agent: "Antigravity".to_string(),
                    provider,
                    model: step_model,
                    input_tokens,
                    cached_tokens,
                    output_tokens,
                    duration_secs,
                    conversation_id: conv_id.to_string(),
                });
            }
        }

        if let Some(t) = ts {
            prev_ts = Some(t);
        }
    }
}

fn collect_jsonl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_jsonl_files(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(p);
        }
    }
}

fn load_claude_events(home: &Path, out: &mut Vec<UsageEvent>) {
    let projects_dir = home.join(".claude").join("projects");
    if !projects_dir.is_dir() {
        return;
    }
    let mut files = Vec::new();
    collect_jsonl_files(&projects_dir, &mut files);

    for file_path in files {
        let Ok(file) = File::open(&file_path) else {
            continue;
        };
        let conv_fallback = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("claude-session")
            .to_string();
        let mut prev_ts: Option<DateTime<Utc>> = None;

        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let ts = v
                .get("timestamp")
                .and_then(|x| x.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            if let Some(msg) = v.get("message") {
                if let Some(usage) = msg.get("usage") {
                    if let Some(timestamp) = ts {
                        let input = usage
                            .get("input_tokens")
                            .and_then(|x| x.as_u64())
                            .unwrap_or(0);
                        let cache_read = usage
                            .get("cache_read_input_tokens")
                            .and_then(|x| x.as_u64())
                            .unwrap_or(0);
                        let cache_create = usage
                            .get("cache_creation_input_tokens")
                            .and_then(|x| x.as_u64())
                            .unwrap_or(0);
                        let output = usage
                            .get("output_tokens")
                            .and_then(|x| x.as_u64())
                            .unwrap_or(0);
                        let total_in = input.saturating_add(cache_read).saturating_add(cache_create);
                        if total_in > 0 || output > 0 {
                            let model = msg
                                .get("model")
                                .and_then(|x| x.as_str())
                                .unwrap_or("claude")
                                .to_string();
                            let session_id = v
                                .get("sessionId")
                                .and_then(|x| x.as_str())
                                .unwrap_or(&conv_fallback)
                                .to_string();
                            let duration_secs = prev_ts
                                .map(|p| (timestamp - p).num_seconds().clamp(1, 300) as u64)
                                .unwrap_or(5);
                            out.push(UsageEvent {
                                timestamp,
                                agent: "Claude Code".to_string(),
                                provider: "Anthropic".to_string(),
                                model,
                                input_tokens: total_in,
                                cached_tokens: cache_read,
                                output_tokens: output,
                                duration_secs,
                                conversation_id: session_id,
                            });
                        }
                    }
                }
            }
            if let Some(t) = ts {
                prev_ts = Some(t);
            }
        }
    }
}

fn load_codex_events(home: &Path, out: &mut Vec<UsageEvent>) {
    let sessions_dir = home.join(".codex").join("sessions");
    if !sessions_dir.is_dir() {
        return;
    }
    let mut files = Vec::new();
    collect_jsonl_files(&sessions_dir, &mut files);

    for file_path in files {
        let Ok(file) = File::open(&file_path) else {
            continue;
        };
        let conv_id = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("codex-session")
            .to_string();
        let mut current_model = "codex".to_string();
        let mut prev_ts: Option<DateTime<Utc>> = None;

        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(m) = v
                .pointer("/payload/model")
                .or_else(|| v.get("model"))
                .and_then(|x| x.as_str())
            {
                current_model = m.to_string();
            }
            let ts = v
                .get("timestamp")
                .and_then(|x| x.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            let usage_val = v
                .pointer("/payload/info/last_token_usage")
                .or_else(|| v.pointer("/payload/usage"))
                .or_else(|| v.get("usage"));

            if let (Some(usage), Some(timestamp)) = (usage_val, ts) {
                let input = usage
                    .get("input_tokens")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let cached = usage
                    .get("cached_input_tokens")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let output = usage
                    .get("output_tokens")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                if input > 0 || output > 0 {
                    let duration_secs = prev_ts
                        .map(|p| (timestamp - p).num_seconds().clamp(1, 300) as u64)
                        .unwrap_or(5);
                    out.push(UsageEvent {
                        timestamp,
                        agent: "Codex".to_string(),
                        provider: "OpenAI".to_string(),
                        model: current_model.clone(),
                        input_tokens: input,
                        cached_tokens: cached,
                        output_tokens: output,
                        duration_secs,
                        conversation_id: conv_id.clone(),
                    });
                }
            }
            if let Some(t) = ts {
                prev_ts = Some(t);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::scanner::AgentKind;

    #[test]
    fn formats_compact_numbers_and_durations() {
        assert_eq!(format_compact(420), "420");
        assert_eq!(format_compact(22_900), "22.9K");
        assert_eq!(format_compact(61_300_000), "61.3M");
        assert_eq!(format_compact(11_600_000_000), "11.6B");
        assert_eq!(format_duration(0), "0m");
        assert_eq!(format_duration(45), "45 s");
        assert_eq!(format_duration(1320), "22 min");
        assert_eq!(format_duration(7200), "2 h");
    }

    #[test]
    fn builds_usage_report_from_events() {
        let now = Utc::now();
        let events = vec![
            UsageEvent {
                timestamp: now,
                agent: "Antigravity".to_string(),
                provider: "Google".to_string(),
                model: "Gemini 4 Neon (Medium)".to_string(),
                input_tokens: 15_000,
                cached_tokens: 10_000,
                output_tokens: 1_200,
                duration_secs: 60,
                conversation_id: "c1".to_string(),
            },
            UsageEvent {
                timestamp: now,
                agent: "Claude Code".to_string(),
                provider: "Anthropic".to_string(),
                model: "claude-opus".to_string(),
                input_tokens: 5_000,
                cached_tokens: 0,
                output_tokens: 800,
                duration_secs: 30,
                conversation_id: "c2".to_string(),
            },
        ];
        let detected = vec![DetectedAgent {
            name: "Codex".to_string(),
            short_name: "Codex",
            kind: AgentKind::Codex,
            installed: true,
            detail: "local config".to_string(),
        }];

        let report = UsageReport::build(&events, &detected, UsageRange::Days30, None);
        assert_eq!(report.total_input_tokens, 20_000);
        assert_eq!(report.total_output_tokens, 2_000);
        assert_eq!(report.total_tasks, 2);
        assert_eq!(report.total_turns, 2);
        assert_eq!(report.total_agent_secs, 90);
        assert_eq!(report.models.len(), 2);
        assert_eq!(report.agents.len(), 3);

        let filtered = UsageReport::build(&events, &detected, UsageRange::Days30, Some("Antigravity"));
        assert_eq!(filtered.total_input_tokens, 15_000);
        assert_eq!(filtered.total_tasks, 1);
    }

    #[test]
    fn extracts_model_selection_from_user_input() {
        let sample = "<USER_SETTINGS_CHANGE>\nThe user changed setting `Model Selection` from None to Gemini 3.8 Flash (Medium). No need to comment";
        assert_eq!(
            extract_model_selection(sample).as_deref(),
            Some("Gemini 3.8 Flash (Medium)")
        );
    }

    #[test]
    fn today_range_builds_24_hourly_buckets() {
        let now = Utc::now();
        let events = vec![UsageEvent {
            timestamp: now,
            agent: "Antigravity".to_string(),
            provider: "Google".to_string(),
            model: "Gemini 4".to_string(),
            input_tokens: 1_200,
            cached_tokens: 0,
            output_tokens: 300,
            duration_secs: 20,
            conversation_id: "today-1".to_string(),
        }];
        let report = UsageReport::build(&events, &[], UsageRange::Today, None);
        assert_eq!(report.range_label, "Today (hourly)");
        assert_eq!(report.daily_series.len(), 24);
        assert_eq!(report.daily_series[0].label, "00h");
        assert_eq!(report.daily_series[23].label, "23h");
        assert_eq!(report.total_input_tokens, 1_200);
        assert_eq!(report.total_output_tokens, 300);
    }

    #[test]
    fn antigravity_parser_prefers_step_model_and_sums_cache_tokens() {
        let jsonl = concat!(
            r#"{"step_index":0,"type":"USER_INPUT","created_at":"2026-11-22T18:00:00Z","content":"Model Selection` from None to Gemini 4 Neon (Medium)."}"#,
            "\n",
            r#"{"step_index":1,"type":"PLANNER_RESPONSE","created_at":"2026-11-22T18:00:02Z","model":"gemini-4-neon-medium","input_tokens":2000,"cache_read_tokens":10000,"output_tokens":300}"#,
            "\n",
            r#"{"step_index":2,"type":"PLANNER_RESPONSE","created_at":"2026-11-22T18:00:10Z","model":"gemini-4-hydrogen-high","input_tokens":1500,"cache_read_tokens":12000,"output_tokens":500}"#,
            "\n"
        );
        let mut events = Vec::new();
        parse_antigravity_reader("conv-test", std::io::Cursor::new(jsonl), &mut events);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].model, "gemini-4-neon-medium");
        assert_eq!(events[0].input_tokens, 12_000);
        assert_eq!(events[0].cached_tokens, 10_000);
        assert!(events[0].cached_tokens <= events[0].input_tokens);

        assert_eq!(events[1].model, "gemini-4-hydrogen-high");
        assert_eq!(events[1].input_tokens, 13_500);
        assert_eq!(events[1].cached_tokens, 12_000);
        assert!(events[1].cached_tokens <= events[1].input_tokens);
    }
}
