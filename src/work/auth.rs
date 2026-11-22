use crate::work::client::{curl_get_json, curl_post_json, open_in_browser};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn import_github_cli_token() -> Result<String, String> {
    let mut cmd = Command::new("gh");
    cmd.args(["auth", "token"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|_| "GitHub CLI (gh) is not installed or not in PATH.".to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh auth failed: {}", err.trim()));
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        Err("gh returned an empty token. Run `gh auth login` first.".to_string())
    } else {
        Ok(token)
    }
}

pub fn import_git_credential_manager(host: &str) -> Result<(String, String), String> {
    let mut cmd = Command::new("git");
    cmd.args(["credential", "fill"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to run git credential fill: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = format!("protocol=https\nhost={host}\n\n");
        let _ = stdin.write_all(payload.as_bytes());
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("git credential failed: {e}"))?;
    if !output.status.success() {
        return Err(format!("No stored Git credential found for {host}."));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_git_credential_output(&stdout)
        .ok_or_else(|| format!("Git Credential Manager returned no password for {host}."))
}

pub fn parse_git_credential_output(raw: &str) -> Option<(String, String)> {
    let mut user = String::new();
    let mut pass = String::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(v) = trimmed.strip_prefix("username=") {
            user = v.to_string();
        } else if let Some(v) = trimmed.strip_prefix("password=") {
            pass = v.to_string();
        }
    }
    if pass.is_empty() {
        None
    } else {
        Some((user, pass))
    }
}

pub fn extract_query_param(request_line: &str, key: &str) -> Option<String> {
    let path = request_line.split_whitespace().nth(1)?;
    let (_, query) = path.split_once('?')?;
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key && !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn wait_for_oauth_callback(port: u16, timeout_secs: u64) -> Result<String, String> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("Could not bind OAuth callback port {port}: {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to configure OAuth socket: {e}"))?;

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let first_line = req.lines().next().unwrap_or("");

                if let Some(err) = extract_query_param(first_line, "error") {
                    let html = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='background:#0a0a0b;color:#e4e4e7;font-family:sans-serif;padding:40px'><h2>Authorization cancelled</h2><p>You can close this tab and return to Velt.</p></body></html>";
                    let _ = stream.write_all(html.as_bytes());
                    return Err(format!("OAuth error: {err}"));
                }

                if let Some(code) = extract_query_param(first_line, "code") {
                    let html = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='background:#0a0a0b;color:#e4e4e7;font-family:sans-serif;padding:40px'><h2>Connected to Velt!</h2><p>Authorization succeeded. You can close this browser tab.</p></body></html>";
                    let _ = stream.write_all(html.as_bytes());
                    return Ok(code);
                }

                let not_found = "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(not_found.as_bytes());
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(e) => return Err(format!("OAuth listener error: {e}")),
        }
    }

    Err("OAuth timed out waiting for browser callback (90s).".to_string())
}

pub fn run_atlassian_oauth(
    client_id: &str,
    client_secret: &str,
) -> Result<(String, String, String), String> {
    let cid = client_id.trim();
    let secret = client_secret.trim();
    if cid.is_empty() || secret.is_empty() {
        return Err("Enter Atlassian OAuth Client ID and Client Secret first.".to_string());
    }

    let redirect_uri = "http://127.0.0.1:48271/callback";
    let auth_url = format!(
        "https://auth.atlassian.com/authorize?audience=api.atlassian.com&client_id={cid}&scope=read%3Ajira-work%20read%3Ajira-user%20offline_access&redirect_uri=http%3A%2F%2F127.0.0.1%3A48271%2Fcallback&state=velt&response_type=code&prompt=consent"
    );
    open_in_browser(&auth_url);

    let code = wait_for_oauth_callback(48271, 90)?;

    let body = json!({
        "grant_type": "authorization_code",
        "client_id": cid,
        "client_secret": secret,
        "code": code,
        "redirect_uri": redirect_uri,
    })
    .to_string();

    let token_resp = curl_post_json("https://auth.atlassian.com/oauth/token", &body)?;
    let access_token = token_resp
        .get("access_token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "Atlassian OAuth response missing access_token.".to_string())?
        .to_string();

    let headers = vec![format!("Authorization: Bearer {access_token}")];
    let resources = curl_get_json(
        "https://api.atlassian.com/oauth/token/accessible-resources",
        &headers,
        None,
    )?;

    let first = resources
        .as_array()
        .and_then(|arr| arr.first())
        .ok_or_else(|| "No accessible Jira Cloud sites found for this Atlassian account.".to_string())?;

    let cloud_id = first
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "Missing cloudId in Atlassian accessible-resources.".to_string())?
        .to_string();
    let site_url = first
        .get("url")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .to_string();

    Ok((access_token, cloud_id, site_url))
}

pub fn run_github_oauth(client_id: &str, client_secret: &str) -> Result<String, String> {
    let cid = client_id.trim();
    let secret = client_secret.trim();
    if cid.is_empty() || secret.is_empty() {
        return Err("Enter GitHub OAuth Client ID and Client Secret first.".to_string());
    }

    let redirect_uri = "http://127.0.0.1:48272/callback";
    let auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={cid}&scope=repo%20read%3Auser&redirect_uri=http%3A%2F%2F127.0.0.1%3A48272%2Fcallback&state=velt"
    );
    open_in_browser(&auth_url);

    let code = wait_for_oauth_callback(48272, 90)?;

    let body = json!({
        "client_id": cid,
        "client_secret": secret,
        "code": code,
        "redirect_uri": redirect_uri,
    })
    .to_string();

    let token_resp = curl_post_json("https://github.com/login/oauth/access_token", &body)?;
    token_resp
        .get("access_token")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "GitHub OAuth response missing access_token.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_credential_output_lines() {
        let sample = "protocol=https\nhost=github.com\nusername=Regnault\npassword=gho_testtoken123\n";
        assert_eq!(
            parse_git_credential_output(sample),
            Some(("Regnault".to_string(), "gho_testtoken123".to_string()))
        );
        assert_eq!(parse_git_credential_output("protocol=https\n"), None);
    }

    #[test]
    fn extracts_oauth_code_from_http_request_line() {
        let req = "GET /callback?code=abc123XYZ&state=terminalplus HTTP/1.1";
        assert_eq!(
            extract_query_param(req, "code").as_deref(),
            Some("abc123XYZ")
        );
        assert_eq!(extract_query_param(req, "error"), None);
    }
}
