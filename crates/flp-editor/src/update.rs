//! Check for a newer release of the editor on GitHub.
//!
//! The repository may be private: a token is then taken from the environment
//! (`FLP_GITHUB_TOKEN`, `GITHUB_TOKEN`, `GH_TOKEN`) or from `gh auth token`.
//! The token is never stored by the editor and is only sent to `api.github.com`.

use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

/// Version of this build.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Repository checked when `update_repo` is not set in the editor settings.
pub const DEFAULT_REPO: &str = "phazz1980/FLP_next";

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub url: String,
}

#[derive(Debug, PartialEq)]
pub enum Check {
    Newer(Release),
    UpToDate,
    /// No published release is visible (none yet, or the token has no access).
    NoReleases,
}

/// Why a GitHub request failed.
enum Failure {
    Status(u16),
    Network(String),
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    html_url: String,
}

/// Asks GitHub for the latest published release of `repo` ("owner/name").
pub fn check(repo: &str) -> Result<Check, String> {
    if !valid_repo(repo) {
        return Err(format!(
            "некорректный адрес репозитория «{repo}», нужен вид владелец/имя"
        ));
    }
    let token = token();
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .build();
    let get = |path: &str| {
        let mut request = agent
            .get(&format!("https://api.github.com/repos/{repo}{path}"))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .set(
                "User-Agent",
                concat!("FLP_next/", env!("CARGO_PKG_VERSION")),
            );
        if let Some(token) = &token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        request.call().map_err(|e| match e {
            ureq::Error::Status(code, _) => Failure::Status(code),
            e => Failure::Network(e.to_string()),
        })
    };
    let release: ApiRelease = match get("/releases/latest") {
        Ok(r) => r
            .into_json()
            .map_err(|e| format!("ответ GitHub не прочитан: {e}"))?,
        Err(Failure::Status(401)) => {
            return Err("GitHub отклонил токен (401): проверьте FLP_GITHUB_TOKEN".to_owned())
        }
        Err(Failure::Status(403 | 429)) => {
            return Err("GitHub ограничил число запросов, попробуйте позже".to_owned())
        }
        // No release yet, unless the repository itself is not visible.
        Err(Failure::Status(404)) if get("").is_ok() => return Ok(Check::NoReleases),
        Err(Failure::Status(404)) => {
            return Err(format!(
                "репозиторий {repo} не найден или закрытый: задайте токен GitHub \
                 в переменной FLP_GITHUB_TOKEN (право Contents: read) или выполните gh auth login"
            ))
        }
        Err(Failure::Status(code)) => return Err(format!("ошибка GitHub {code}")),
        Err(Failure::Network(e)) => return Err(format!("нет связи с GitHub: {e}")),
    };
    let version = release.tag_name.trim_start_matches(['v', 'V']).to_owned();
    Ok(if is_newer(&version, CURRENT) {
        Check::Newer(Release {
            version,
            url: release.html_url,
        })
    } else {
        Check::UpToDate
    })
}

fn valid_repo(repo: &str) -> bool {
    let part = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    repo.split_once('/')
        .is_some_and(|(owner, name)| part(owner) && part(name))
}

fn token() -> Option<String> {
    for var in ["FLP_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"] {
        if let Some(t) = std::env::var(var).ok().map(|t| t.trim().to_owned()) {
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    let mut cmd = Command::new("gh");
    cmd.args(["auth", "token"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok().filter(|o| o.status.success())?;
    let t = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!t.is_empty()).then_some(t)
}

/// Numeric parts of "1.2.3" (a suffix like "-beta" is ignored).
fn parts(version: &str) -> Vec<u64> {
    version
        .split(['-', '+'])
        .next()
        .unwrap_or_default()
        .split('.')
        .map(|p| p.trim().parse().unwrap_or(0))
        .collect()
}

fn is_newer(candidate: &str, current: &str) -> bool {
    let (mut a, mut b) = (parts(candidate), parts(current));
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a > b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("0.10.0", "0.9.5"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn repo_address_is_checked() {
        assert!(valid_repo("phazz1980/FLP_next"));
        assert!(!valid_repo("phazz1980"));
        assert!(!valid_repo("a/b/c"));
        assert!(!valid_repo("a/b?x=1"));
        assert!(!valid_repo("/b"));
    }
}
