use crate::config::IntegrationsConfig;
use crate::work::git_remote::GitRemote;
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
pub struct JiraIssue {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub status_category: String,
    pub priority: String,
    pub issue_type: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct PullRequestItem {
    #[allow(dead_code)]
    pub provider: &'static str,
    pub id: u64,
    pub title: String,
    pub repo: String,
    pub source_branch: String,
    pub target_branch: String,
    pub author: String,
    pub state: String,
    pub url: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkData {
    pub jira_issues: Vec<JiraIssue>,
    pub jira_status: String,
    pub bitbucket_prs: Vec<PullRequestItem>,
    pub bitbucket_status: String,
    pub github_prs: Vec<PullRequestItem>,
    pub github_status: String,
}

pub fn slugify_branch(key: &str, summary: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in summary.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
        if slug.len() >= 36 {
            break;
        }
    }
    let trimmed = slug.trim_end_matches('-');
    if trimmed.is_empty() {
        format!("feat/{key}")
    } else {
        format!("feat/{key}-{trimmed}")
    }
}

pub fn open_in_browser(url: &str) {
    let trimmed = url.trim();
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("rundll32.exe");
        cmd.args(["url.dll,FileProtocolHandler", trimmed]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("xdg-open").arg(trimmed).spawn();
    }
}

fn escape_curl_val(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], "")
}

pub fn curl_get_json(
    url: &str,
    headers: &[String],
    basic_auth: Option<(&str, &str)>,
) -> Result<Value, String> {
    curl_request_json(url, "GET", None, headers, basic_auth)
}

pub fn curl_post_json(url: &str, body_json: &str) -> Result<Value, String> {
    let headers = vec!["Content-Type: application/json".to_string()];
    curl_request_json(url, "POST", Some(body_json), &headers, None)
}

fn curl_request_json(
    url: &str,
    method: &str,
    body_data: Option<&str>,
    headers: &[String],
    basic_auth: Option<(&str, &str)>,
) -> Result<Value, String> {
    let curl_bin = if std::path::Path::new(r"C:\Windows\System32\curl.exe").exists() {
        r"C:\Windows\System32\curl.exe"
    } else {
        "curl"
    };

    let mut cfg = String::new();
    cfg.push_str(&format!("url = \"{}\"\n", escape_curl_val(url)));
    if method != "GET" {
        cfg.push_str(&format!("request = \"{}\"\n", escape_curl_val(method)));
    }
    cfg.push_str("header = \"Accept: application/json\"\n");
    for h in headers {
        cfg.push_str(&format!("header = \"{}\"\n", escape_curl_val(h)));
    }
    if let Some((user, pass)) = basic_auth {
        let cred = format!("{user}:{pass}");
        cfg.push_str(&format!("user = \"{}\"\n", escape_curl_val(&cred)));
    }
    if let Some(body) = body_data {
        cfg.push_str(&format!("data = \"{}\"\n", escape_curl_val(body)));
    }

    let mut cmd = Command::new(curl_bin);
    cmd.args(["-sS", "--max-time", "15", "-w", "\n%{http_code}", "-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to launch curl: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(cfg.as_bytes());
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("curl wait failed: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Network error: {}", err.trim()));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let (body, code_str) = match raw.rsplit_once('\n') {
        Some((b, c)) => (b, c.trim()),
        None => (raw.as_ref(), "0"),
    };
    let status_code: u16 = code_str.parse().unwrap_or(0);
    if !(200..300).contains(&status_code) {
        let snippet: String = body.chars().take(160).collect();
        return Err(format!("HTTP {status_code}: {snippet}"));
    }

    serde_json::from_str::<Value>(body).map_err(|e| format!("Invalid JSON response: {e}"))
}

pub fn fetch_all_work_data(
    cfg: &IntegrationsConfig,
    detected_remote: Option<&GitRemote>,
) -> WorkData {
    let mut data = WorkData::default();

    // 1. Jira Cloud (supports OAuth 2.0 Bearer OR Basic API Token)
    let domain = cfg
        .jira_domain
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let oauth_token = cfg.jira_oauth_access_token.trim();
    let cloud_id = cfg.jira_cloud_id.trim();

    let jira_resp = if !oauth_token.is_empty() && !cloud_id.is_empty() {
        let url = format!(
            "https://api.atlassian.com/ex/jira/{cloud_id}/rest/api/3/search/jql?jql=assignee%3DcurrentUser()%20AND%20resolution%3DUnresolved%20ORDER%20BY%20updated%20DESC&maxResults=30&fields=summary,status,priority,issuetype"
        );
        let headers = vec![format!("Authorization: Bearer {oauth_token}")];
        Some(curl_get_json(&url, &headers, None))
    } else if !domain.is_empty()
        && !cfg.jira_email.trim().is_empty()
        && !cfg.jira_api_token.trim().is_empty()
    {
        let url = format!(
            "https://{domain}/rest/api/3/search/jql?jql=assignee%3DcurrentUser()%20AND%20resolution%3DUnresolved%20ORDER%20BY%20updated%20DESC&maxResults=30&fields=summary,status,priority,issuetype"
        );
        Some(curl_get_json(
            &url,
            &[],
            Some((cfg.jira_email.trim(), cfg.jira_api_token.trim())),
        ))
    } else {
        None
    };

    match jira_resp {
        None => {
            data.jira_status =
                "Connect Jira via OAuth 2.0 or API Token in Connections.".to_string();
        }
        Some(Ok(v)) => {
            if let Some(arr) = v.get("issues").and_then(|x| x.as_array()) {
                for issue in arr {
                    let key = issue
                        .get("key")
                        .and_then(|x| x.as_str())
                        .unwrap_or("ISSUE")
                        .to_string();
                    let fields = issue.get("fields").unwrap_or(&Value::Null);
                    let summary = fields
                        .get("summary")
                        .and_then(|x| x.as_str())
                        .unwrap_or("(no summary)")
                        .to_string();
                    let status = fields
                        .pointer("/status/name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("Open")
                        .to_string();
                    let status_category = fields
                        .pointer("/status/statusCategory/key")
                        .and_then(|x| x.as_str())
                        .unwrap_or("new")
                        .to_string();
                    let priority = fields
                        .pointer("/priority/name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("Medium")
                        .to_string();
                    let issue_type = fields
                        .pointer("/issuetype/name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("Task")
                        .to_string();
                    let browse_base = if domain.is_empty() {
                        "id.atlassian.com"
                    } else {
                        domain
                    };
                    let url = format!("https://{browse_base}/browse/{key}");
                    data.jira_issues.push(JiraIssue {
                        key,
                        summary,
                        status,
                        status_category,
                        priority,
                        issue_type,
                        url,
                    });
                }
                data.jira_status = format!("{} unresolved issues", data.jira_issues.len());
            } else {
                data.jira_status = "Connected (0 issues returned)".to_string();
            }
        }
        Some(Err(e)) => {
            data.jira_status = format!("Jira error: {e}");
        }
    }

    // 2. Bitbucket Cloud
    let bb_user = cfg.bitbucket_username.trim();
    let bb_pass = cfg.bitbucket_app_password.trim();
    if bb_user.is_empty() || bb_pass.is_empty() {
        data.bitbucket_status =
            "Configure Bitbucket username & app password in Connections.".to_string();
    } else {
        let bb_url = match detected_remote {
            Some(GitRemote::Bitbucket { workspace, repo }) => format!(
                "https://api.bitbucket.org/2.0/repositories/{workspace}/{repo}/pullrequests?state=OPEN&pagelen=25"
            ),
            _ => {
                let target = if cfg.bitbucket_workspace.trim().is_empty() {
                    bb_user
                } else {
                    cfg.bitbucket_workspace.trim()
                };
                format!("https://api.bitbucket.org/2.0/pullrequests/{target}?state=OPEN&pagelen=25")
            }
        };

        match curl_get_json(&bb_url, &[], Some((bb_user, bb_pass))) {
            Ok(v) => {
                if let Some(arr) = v.get("values").and_then(|x| x.as_array()) {
                    for pr in arr {
                        let id = pr.get("id").and_then(|x| x.as_u64()).unwrap_or(0);
                        let title = pr
                            .get("title")
                            .and_then(|x| x.as_str())
                            .unwrap_or("(untitled PR)")
                            .to_string();
                        let repo = pr
                            .pointer("/destination/repository/full_name")
                            .and_then(|x| x.as_str())
                            .unwrap_or("repo")
                            .to_string();
                        let source_branch = pr
                            .pointer("/source/branch/name")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let target_branch = pr
                            .pointer("/destination/branch/name")
                            .and_then(|x| x.as_str())
                            .unwrap_or("main")
                            .to_string();
                        let author = pr
                            .pointer("/author/display_name")
                            .and_then(|x| x.as_str())
                            .unwrap_or("user")
                            .to_string();
                        let state = pr
                            .get("state")
                            .and_then(|x| x.as_str())
                            .unwrap_or("OPEN")
                            .to_string();
                        let url = pr
                            .pointer("/links/html/href")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        data.bitbucket_prs.push(PullRequestItem {
                            provider: "Bitbucket",
                            id,
                            title,
                            repo,
                            source_branch,
                            target_branch,
                            author,
                            state,
                            url,
                        });
                    }
                    data.bitbucket_status = format!("{} open PRs", data.bitbucket_prs.len());
                } else {
                    data.bitbucket_status = "Connected (0 open PRs)".to_string();
                }
            }
            Err(e) => {
                data.bitbucket_status = format!("Bitbucket error: {e}");
            }
        }
    }

    // 3. GitHub
    let gh_token = cfg.github_token.trim();
    let gh_headers: Vec<String> = if gh_token.is_empty() {
        vec!["User-Agent: Velt".to_string()]
    } else {
        vec![
            "User-Agent: Velt".to_string(),
            format!("Authorization: Bearer {gh_token}"),
        ]
    };

    if let Some(GitRemote::GitHub { owner, repo }) = detected_remote {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/pulls?state=open&per_page=25");
        match curl_get_json(&url, &gh_headers, None) {
            Ok(v) => {
                if let Some(arr) = v.as_array() {
                    for pr in arr {
                        let id = pr.get("number").and_then(|x| x.as_u64()).unwrap_or(0);
                        let title = pr
                            .get("title")
                            .and_then(|x| x.as_str())
                            .unwrap_or("(untitled PR)")
                            .to_string();
                        let source_branch = pr
                            .pointer("/head/ref")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let target_branch = pr
                            .pointer("/base/ref")
                            .and_then(|x| x.as_str())
                            .unwrap_or("main")
                            .to_string();
                        let author = pr
                            .pointer("/user/login")
                            .and_then(|x| x.as_str())
                            .unwrap_or("user")
                            .to_string();
                        let state = pr
                            .get("state")
                            .and_then(|x| x.as_str())
                            .unwrap_or("open")
                            .to_string();
                        let url = pr
                            .get("html_url")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        data.github_prs.push(PullRequestItem {
                            provider: "GitHub",
                            id,
                            title,
                            repo: format!("{owner}/{repo}"),
                            source_branch,
                            target_branch,
                            author,
                            state,
                            url,
                        });
                    }
                    data.github_status =
                        format!("{owner}/{repo}: {} open PRs", data.github_prs.len());
                }
            }
            Err(e) => {
                data.github_status = format!("GitHub error: {e}");
            }
        }
    } else if !gh_token.is_empty() {
        let url = "https://api.github.com/search/issues?q=is:pr+is:open+involves:@me&per_page=25";
        match curl_get_json(url, &gh_headers, None) {
            Ok(v) => {
                if let Some(arr) = v.get("items").and_then(|x| x.as_array()) {
                    for item in arr {
                        let id = item.get("number").and_then(|x| x.as_u64()).unwrap_or(0);
                        let title = item
                            .get("title")
                            .and_then(|x| x.as_str())
                            .unwrap_or("(untitled PR)")
                            .to_string();
                        let repo_url = item
                            .get("repository_url")
                            .and_then(|x| x.as_str())
                            .unwrap_or("");
                        let repo = repo_url
                            .strip_prefix("https://api.github.com/repos/")
                            .unwrap_or(repo_url)
                            .to_string();
                        let author = item
                            .pointer("/user/login")
                            .and_then(|x| x.as_str())
                            .unwrap_or("user")
                            .to_string();
                        let state = item
                            .get("state")
                            .and_then(|x| x.as_str())
                            .unwrap_or("open")
                            .to_string();
                        let url = item
                            .get("html_url")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        data.github_prs.push(PullRequestItem {
                            provider: "GitHub",
                            id,
                            title,
                            repo,
                            source_branch: String::new(),
                            target_branch: String::new(),
                            author,
                            state,
                            url,
                        });
                    }
                    data.github_status = format!("{} open PRs involving you", data.github_prs.len());
                }
            }
            Err(e) => {
                data.github_status = format!("GitHub error: {e}");
            }
        }
    } else {
        data.github_status =
            "Open a GitHub repo or add a Personal Access Token in Connections.".to_string();
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_jira_branch_names() {
        assert_eq!(
            slugify_branch("KAN-42", "Fix login token refresh bug!"),
            "feat/KAN-42-fix-login-token-refresh-bug"
        );
        assert_eq!(slugify_branch("DEV-7", "   "), "feat/DEV-7");
    }
}
