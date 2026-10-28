//! Check for a newer release of the editor on GitHub.
//!
//! The repository may be private: a token is then taken from the environment
//! (`FLP_GITHUB_TOKEN`, `GITHUB_TOKEN`, `GH_TOKEN`) or from `gh auth token`.
//! The token is never stored by the editor and is only sent to `api.github.com`.

use std::cmp::Ordering;
use std::process::Command;
use std::time::Duration;

use eframe::egui;
use serde::Deserialize;

/// Version of this build.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Repository checked when `update_repo` is not set in the editor settings.
pub const DEFAULT_REPO: &str = "phazz1980/FLP_next";

/// Newest releases requested from GitHub: enough to tell what changed.
const RELEASES_PER_PAGE: usize = 30;
/// Releases listed in the notes and characters per item.
const MAX_RELEASES: usize = 10;
const MAX_LEN: usize = 110;

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// Newest version.
    pub version: String,
    /// Direct download of the release archive, or the release page when it has none.
    pub url: String,
    /// Notes of every release newer than the running version, newest first.
    pub changes: Vec<Change>,
}

/// Brief notes of one release.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub version: String,
    pub items: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum Check {
    Newer(Release),
    UpToDate,
    /// No published release in the repository yet.
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
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
}

impl ApiRelease {
    /// Where the browser goes to get this release: the archive itself (a `.zip`
    /// when there are several files) or, without files, the release page.
    fn download_url(&self) -> &str {
        self.assets
            .iter()
            .find(|a| a.name.to_lowercase().ends_with(".zip"))
            .or(self.assets.first())
            .map_or(&self.html_url, |a| &a.browser_download_url)
    }
}

/// Asks GitHub for the published releases of `repo` ("owner/name") and compares
/// them with the running version.
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
    let releases: Vec<ApiRelease> = match get(&format!("/releases?per_page={RELEASES_PER_PAGE}")) {
        Ok(r) => r
            .into_json()
            .map_err(|e| format!("ответ GitHub не прочитан: {e}"))?,
        Err(Failure::Status(401)) => {
            return Err("GitHub отклонил токен (401): проверьте FLP_GITHUB_TOKEN".to_owned())
        }
        Err(Failure::Status(403 | 429)) => {
            return Err("GitHub ограничил число запросов, попробуйте позже".to_owned())
        }
        // A repository without releases answers with an empty list, so 404 means
        // the repository itself is missing or hidden.
        Err(Failure::Status(404)) => {
            return Err(format!(
                "репозиторий {repo} не найден или закрытый: задайте токен GitHub \
                 в переменной FLP_GITHUB_TOKEN (право Contents: read) или выполните gh auth login"
            ))
        }
        Err(Failure::Status(code)) => return Err(format!("ошибка GitHub {code}")),
        Err(Failure::Network(e)) => return Err(format!("нет связи с GitHub: {e}")),
    };
    Ok(pick(releases, CURRENT))
}

/// Finds the published releases newer than `current` and collects their notes.
fn pick(releases: Vec<ApiRelease>, current: &str) -> Check {
    let mut published: Vec<(String, ApiRelease)> = releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .map(|r| (r.tag_name.trim_start_matches(['v', 'V']).to_owned(), r))
        .collect();
    if published.is_empty() {
        return Check::NoReleases;
    }
    published.retain(|(version, _)| compare(version, current).is_gt());
    published.sort_by(|a, b| compare(&b.0, &a.0));
    let Some((version, newest)) = published.first() else {
        return Check::UpToDate;
    };
    Check::Newer(Release {
        version: version.clone(),
        url: newest.download_url().to_owned(),
        changes: published
            .iter()
            .take(MAX_RELEASES)
            .map(|(version, r)| Change {
                version: version.clone(),
                items: summarize(r.body.as_deref().unwrap_or_default()),
            })
            .collect(),
    })
}

/// Short list of what a release changed, taken from its notes: the bullet points
/// (or, without bullets, the plain lines) with the markdown marks removed.
/// A bullet continued on indented lines is joined into one item.
fn summarize(notes: &str) -> Vec<String> {
    let mut bullets: Vec<String> = Vec::new();
    let mut plain: Vec<String> = Vec::new();
    let mut continuing = false;
    for line in notes.lines() {
        let text = line.trim();
        let skipped = text.is_empty()
            || text.starts_with('#')
            || text
                .trim_start_matches('*')
                .to_lowercase()
                .starts_with("full changelog");
        if skipped {
            continuing = false;
        } else if let Some(item) = strip_bullet(text) {
            bullets.push(item.to_owned());
            continuing = true;
        } else if continuing && line.starts_with(char::is_whitespace) {
            if let Some(last) = bullets.last_mut() {
                last.push(' ');
                last.push_str(text);
            }
        } else {
            plain.push(text.to_owned());
        }
    }
    let chosen = if bullets.is_empty() { plain } else { bullets };
    chosen
        .iter()
        .map(|l| clean(l))
        .filter(|l| !l.is_empty())
        .collect()
}

fn strip_bullet(line: &str) -> Option<&str> {
    ["- ", "* ", "+ "]
        .iter()
        .find_map(|mark| line.strip_prefix(mark))
}

/// `[text](address)` becomes `text`.
fn strip_links(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let link = after
            .find("](")
            .filter(|&mid| !after[..mid].contains(['[', ']']));
        if let Some((mid, end)) = link.and_then(|mid| Some((mid, after[mid + 2..].find(')')?))) {
            out.push_str(&rest[..open]);
            out.push_str(&after[..mid]);
            rest = &after[mid + 2 + end + 1..];
        } else {
            out.push_str(&rest[..=open]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Drops markdown emphasis, code marks and link addresses, and shortens a long line.
fn clean(line: &str) -> String {
    let plain = strip_links(line).replace("**", "").replace('`', "");
    let plain = plain.trim();
    if plain.chars().count() > MAX_LEN {
        let cut: String = plain.chars().take(MAX_LEN).collect();
        format!("{}…", cut.trim_end())
    } else {
        plain.to_owned()
    }
}

/// Window with what changed since the running version and the download link.
/// What the window shows next to the install button.
pub enum Install<'a> {
    /// Not started (the button is shown when the editor can update itself in place).
    Idle(Option<&'a str>),
    /// Downloading: bytes received and the archive size (0 when unknown).
    Busy(u64, u64),
    /// Already downloaded: the button only restarts into the new version.
    Ready,
}

/// Returns true when the user pressed "Обновить и перезапустить".
pub fn window(
    ctx: &egui::Context,
    release: &Release,
    open: &mut bool,
    install: Install,
    can_install: bool,
) -> bool {
    let mut start = false;
    egui::Window::new("Доступно обновление")
        .open(open)
        .collapsible(false)
        .resizable(true)
        .default_width(480.0)
        .show(ctx, |ui| {
            ui.label(format!(
                "Новая версия {} (установлена {CURRENT})",
                release.version
            ));
            ui.hyperlink_to(format!("Скачать версию {}", release.version), &release.url);
            match install {
                Install::Busy(done, total) => {
                    let mb = |n: u64| n as f32 / 1_048_576.0;
                    let fraction = if total > 0 { done as f32 / total as f32 } else { 0.0 };
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .text(format!("Скачивание: {:.1} из {:.1} МБ", mb(done), mb(total))),
                    );
                }
                Install::Ready => {
                    ui.label("Обновление скачано.");
                    start = ui
                        .button("Перезапустить и обновить")
                        .on_hover_text("Заменить файлы программы и открыть её заново; несохранённый проект будет сохранён")
                        .clicked();
                }
                Install::Idle(error) => {
                    if can_install {
                        start = ui
                            .button("Обновить и перезапустить")
                            .on_hover_text(
                                "Скачать архив, заменить файлы программы и открыть её заново;                                  несохранённый проект будет сохранён",
                            )
                            .clicked();
                    }
                    if can_install {
                        ui.weak(
                            "Другие открытые окна FLP_next закройте вручную, чтобы они                              обновились: они продолжат работать на старой версии, пока их не                              перезапустят.",
                        );
                    }
                    if let Some(e) = error {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                    }
                }
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for change in &release.changes {
                        ui.strong(&change.version);
                        if change.items.is_empty() {
                            ui.weak("описание не указано");
                        }
                        for item in &change.items {
                            ui.label(format!("• {item}"));
                        }
                        ui.add_space(6.0);
                    }
                });
        });
    start
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

fn compare(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (parts(a), parts(b));
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a.cmp(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, body: &str) -> ApiRelease {
        ApiRelease {
            tag_name: tag.to_owned(),
            html_url: format!("https://example.test/{tag}"),
            body: Some(body.to_owned()),
            draft: false,
            prerelease: false,
            assets: Vec::new(),
        }
    }

    fn asset(name: &str) -> ApiAsset {
        ApiAsset {
            name: name.to_owned(),
            browser_download_url: format!("https://example.test/download/{name}"),
        }
    }

    #[test]
    fn download_goes_straight_to_the_archive() {
        let mut r = release("v1.0.0", "");
        // No files: the release page.
        assert_eq!(r.download_url(), "https://example.test/v1.0.0");
        r.assets = vec![asset("notes.txt"), asset("FLP_next.ZIP")];
        assert_eq!(
            r.download_url(),
            "https://example.test/download/FLP_next.ZIP"
        );
        r.assets = vec![asset("notes.txt")];
        assert_eq!(r.download_url(), "https://example.test/download/notes.txt");
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(compare("0.2.0", "0.1.0").is_gt());
        assert!(compare("0.10.0", "0.9.5").is_gt());
        assert!(compare("1.0", "0.9.9").is_gt());
        assert!(compare("0.1.0", "0.1.0").is_eq());
        assert!(compare("0.1", "0.1.0").is_eq());
        assert!(compare("0.1.0", "0.2.0").is_lt());
    }

    #[test]
    fn repo_address_is_checked() {
        assert!(valid_repo("phazz1980/FLP_next"));
        assert!(!valid_repo("phazz1980"));
        assert!(!valid_repo("a/b/c"));
        assert!(!valid_repo("a/b?x=1"));
        assert!(!valid_repo("/b"));
    }

    #[test]
    fn lists_notes_of_all_newer_releases() {
        let releases = vec![
            release(
                "v0.3.0",
                "## Что нового\n- **Таймеры** получили `reset`\n- Новый блок\n",
            ),
            release("v0.2.0", "Просто текст\n\nFull Changelog: https://x"),
            release("v0.1.0", "- старое"),
        ];
        let Check::Newer(r) = pick(releases, "0.1.0") else {
            panic!("expected a newer release");
        };
        assert_eq!(r.version, "0.3.0");
        assert_eq!(r.url, "https://example.test/v0.3.0");
        assert_eq!(r.changes.len(), 2);
        assert_eq!(r.changes[0].items, ["Таймеры получили reset", "Новый блок"]);
        assert_eq!(r.changes[1].version, "0.2.0");
        assert_eq!(r.changes[1].items, ["Просто текст"]);
    }

    #[test]
    fn skips_drafts_and_prereleases() {
        let mut draft = release("v9.0.0", "- x");
        draft.draft = true;
        let mut pre = release("v8.0.0", "- x");
        pre.prerelease = true;
        assert_eq!(pick(vec![draft, pre], "0.1.0"), Check::NoReleases);
        assert_eq!(pick(vec![release("v0.1.0", "")], "0.1.0"), Check::UpToDate);
    }

    #[test]
    fn notes_keep_every_item_but_shorten_long_ones() {
        let body: String = (1..=10).map(|i| format!("- пункт {i}\n")).collect();
        let items = summarize(&body);
        assert_eq!(items.len(), 10);
        let long = format!("- {}", "я".repeat(300));
        assert!(summarize(&long)[0].chars().count() <= MAX_LEN + 1);
        assert!(summarize("").is_empty());
    }

    #[test]
    fn notes_of_wrapped_bullets_with_links() {
        let body = "* [BUG #1](https://x/1):
  Fixed the **crash** in `rg` [a] here.
* Second [one](u)
";
        assert_eq!(
            summarize(body),
            ["BUG #1: Fixed the crash in rg [a] here.", "Second one"]
        );
    }
}
