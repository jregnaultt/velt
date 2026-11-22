use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitRemote {
    GitHub { owner: String, repo: String },
    Bitbucket { workspace: String, repo: String },
    Other(String),
}

impl GitRemote {
    pub fn label(&self) -> String {
        match self {
            GitRemote::GitHub { owner, repo } => format!("GitHub: {owner}/{repo}"),
            GitRemote::Bitbucket { workspace, repo } => format!("Bitbucket: {workspace}/{repo}"),
            GitRemote::Other(url) => format!("Git: {url}"),
        }
    }
}

pub fn detect_git_remote(cwd: &Path) -> Option<GitRemote> {
    let mut cur = Some(cwd);
    while let Some(dir) = cur {
        let cfg = dir.join(".git").join("config");
        if cfg.is_file() {
            let raw = std::fs::read_to_string(cfg).ok()?;
            return parse_git_config_origin(&raw);
        }
        cur = dir.parent();
    }
    None
}

fn parse_git_config_origin(raw: &str) -> Option<GitRemote> {
    let mut in_origin = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_origin = trimmed == r#"[remote "origin"]"#;
            continue;
        }
        if in_origin {
            if let Some(rest) = trimmed.strip_prefix("url") {
                let rest = rest.trim_start();
                if let Some(url) = rest.strip_prefix('=') {
                    return Some(parse_remote_url(url.trim()));
                }
            }
        }
    }
    None
}

pub fn parse_remote_url(url: &str) -> GitRemote {
    let cleaned = url.trim().trim_end_matches(".git");

    if let Some(rest) = cleaned.strip_prefix("git@github.com:") {
        if let Some((owner, repo)) = split_two(rest) {
            return GitRemote::GitHub { owner, repo };
        }
    }
    if let Some(rest) = cleaned
        .strip_prefix("https://github.com/")
        .or_else(|| cleaned.strip_prefix("http://github.com/"))
    {
        if let Some((owner, repo)) = split_two(rest) {
            return GitRemote::GitHub { owner, repo };
        }
    }

    if let Some(rest) = cleaned.strip_prefix("git@bitbucket.org:") {
        if let Some((workspace, repo)) = split_two(rest) {
            return GitRemote::Bitbucket { workspace, repo };
        }
    }
    if let Some(rest) = cleaned
        .strip_prefix("https://bitbucket.org/")
        .or_else(|| cleaned.strip_prefix("http://bitbucket.org/"))
        .or_else(|| {
            cleaned
                .strip_prefix("https://")
                .and_then(|s| s.split_once("@bitbucket.org/").map(|(_, r)| r))
        })
    {
        if let Some((workspace, repo)) = split_two(rest) {
            return GitRemote::Bitbucket { workspace, repo };
        }
    }

    GitRemote::Other(url.to_string())
}

fn split_two(path: &str) -> Option<(String, String)> {
    let mut parts = path.trim_matches('/').split('/');
    let a = parts.next()?.trim();
    let b = parts.next()?.trim();
    if a.is_empty() || b.is_empty() {
        None
    } else {
        Some((a.to_string(), b.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_https_and_ssh() {
        assert_eq!(
            parse_remote_url("https://github.com/midudev/midterm.git"),
            GitRemote::GitHub {
                owner: "midudev".to_string(),
                repo: "midterm".to_string()
            }
        );
        assert_eq!(
            parse_remote_url("git@github.com:Regnault/TerminalPlus.git"),
            GitRemote::GitHub {
                owner: "Regnault".to_string(),
                repo: "TerminalPlus".to_string()
            }
        );
    }

    #[test]
    fn parses_bitbucket_https_and_ssh() {
        assert_eq!(
            parse_remote_url("https://user@bitbucket.org/myteam/backend-api.git"),
            GitRemote::Bitbucket {
                workspace: "myteam".to_string(),
                repo: "backend-api".to_string()
            }
        );
        assert_eq!(
            parse_remote_url("git@bitbucket.org:myteam/frontend-app.git"),
            GitRemote::Bitbucket {
                workspace: "myteam".to_string(),
                repo: "frontend-app".to_string()
            }
        );
    }
}
