//! Claude profilleri: aynı Claude Code'un ayrı `CLAUDE_CONFIG_DIR` klasörleri (ör. `alias claude-work=
//! 'CLAUDE_CONFIG_DIR="$HOME/.claude-work" claude'`). Her profil ayrı hesaptır: oturumları, transcript'leri,
//! ayarları, limitleri kendi klasöründe.
//!
//! Ayarlarda seçilen profil engine'in okuduğu klasördür (`paths::set_active`); sohbetler, maliyet, limitler ve
//! harcama defteri yalnızca o profile göredir. Seçim `<app_data>/profile.json`'da.
//!
//! Algılama yalnızca okur: varsayılan klasör + kabuk dosyalarındaki `CLAUDE_CONFIG_DIR=` takma adları + ev
//! klasöründeki `.claude*` klasörleri (içinde `projects/` ya da `settings.json` olan). Kabuk dosyasına yazılmaz.

use crate::engine::paths;
use regex::Regex;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const FILE: &str = "profile.json";
/// Okunan kabuk dosyaları (ev klasörüne göre)
const RC_FILES: &[&str] = &[
    ".zshrc",
    ".zprofile",
    ".zshenv",
    ".bashrc",
    ".bash_profile",
    ".profile",
];

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Uygulama dosyalarındaki sonek (`claude-limits-<id>.json`); `~/.claude` için boş
    pub id: String,
    pub name: String,
    pub dir: String,
    /// Bu profili açan komutlar (`claude`, `claude-work`)
    pub commands: Vec<String>,
    pub active: bool,
}

/// Uygulama dosya adı soneki: `~/.claude` → "" (eski dosya adları aynen kalsın), `~/.claude-work` → "claude-work".
pub fn id_for(dir: &Path, home: &Path) -> String {
    if dir == home.join(".claude") {
        return String::new();
    }
    let base = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let clean: String = base
        .trim_start_matches('.')
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if clean.is_empty() {
        "profile".into()
    } else {
        clean
    }
}

/// `claude-limits.json` + "claude-work" → `claude-limits-claude-work.json`
pub fn file_name(base: &str, id: &str) -> String {
    if id.is_empty() {
        return base.to_string();
    }
    match base.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{id}.{ext}"),
        None => format!("{base}-{id}"),
    }
}

/// Şu an seçili profilin soneki
pub fn active_id() -> String {
    match (paths::claude_dir(), std::env::home_dir()) {
        (Some(dir), Some(home)) => id_for(&dir, &home),
        _ => String::new(),
    }
}

fn expand(raw: &str, home: &Path) -> PathBuf {
    let h = home.to_string_lossy();
    let s = raw.replace("${HOME}", &h).replace("$HOME", &h);
    match s.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None if s == "~" => home.to_path_buf(),
        None => PathBuf::from(s),
    }
}

/// Kabuk metninden `alias ad='… CLAUDE_CONFIG_DIR=<yol> … claude …'` eşleşmeleri → (ad, klasör).
pub fn aliases(text: &str, home: &Path) -> Vec<(String, PathBuf)> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^\s*alias\s+([A-Za-z0-9_.-]+)=(.*)$"#).expect("alias regex")
    });
    static DIR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"CLAUDE_CONFIG_DIR=\\?["']?([^"'\s\\]+)"#).expect("dir regex")
    });
    let mut out = Vec::new();
    for c in RE.captures_iter(text) {
        let body = &c[2];
        if !body.contains("claude") {
            continue;
        }
        if let Some(d) = DIR.captures(body) {
            out.push((c[1].to_string(), expand(&d[1], home)));
        }
    }
    out
}

fn same(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

/// Profil listesi. `default` = profil seçilmediğinde okunan klasör; `rc` = kabuk dosyalarının metni.
pub fn detect(home: &Path, default: &Path, rc: &[String], active: &Path) -> Vec<Profile> {
    let mut list: Vec<(PathBuf, Vec<String>)> =
        vec![(default.to_path_buf(), vec!["claude".into()])];
    let mut add = |dir: PathBuf, cmd: Option<String>| {
        if !dir.is_dir() {
            return;
        }
        if let Some(entry) = list.iter_mut().find(|(d, _)| same(d, &dir)) {
            if let Some(c) = cmd
                && !entry.1.contains(&c)
            {
                entry.1.push(c);
            }
        } else {
            list.push((dir, cmd.into_iter().collect()));
        }
    };
    for text in rc {
        for (name, dir) in aliases(text, home) {
            add(dir, Some(name));
        }
    }
    if let Ok(rd) = std::fs::read_dir(home) {
        let mut dirs: Vec<PathBuf> = rd
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".claude"))
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir() && (p.join("projects").is_dir() || p.join("settings.json").is_file())
            })
            .collect();
        dirs.sort();
        for d in dirs {
            add(d, None);
        }
    }
    list.into_iter()
        .map(|(dir, commands)| {
            let id = id_for(&dir, home);
            // Ad: `claude-<x>` komutundan x; yoksa klasör adından; varsayılan klasör "default"
            let from_cmd = commands
                .iter()
                .find_map(|c| {
                    c.strip_prefix("claude-")
                        .or_else(|| c.strip_prefix("claude_"))
                })
                .map(str::to_string);
            let name = from_cmd.unwrap_or_else(|| {
                if same(&dir, default) {
                    "default".into()
                } else {
                    id.trim_start_matches("claude-").to_string()
                }
            });
            Profile {
                id,
                name,
                dir: dir.to_string_lossy().into_owned(),
                commands,
                active: same(&dir, active),
            }
        })
        .collect()
}

fn rc_texts(home: &Path) -> Vec<String> {
    RC_FILES
        .iter()
        .filter_map(|f| std::fs::read_to_string(home.join(f)).ok())
        .collect()
}

/// Gerçek profiller (kullanıcının ev klasörü).
pub fn list() -> Vec<Profile> {
    let (Some(home), Some(default), Some(active)) = (
        std::env::home_dir(),
        paths::default_claude_dir(),
        paths::claude_dir(),
    ) else {
        return vec![];
    };
    detect(&home, &default, &rc_texts(&home), &active)
}

#[derive(Serialize, serde::Deserialize)]
struct Saved {
    dir: String,
}

/// Açılışta: kayıtlı profil hâlâ varsa seçilir.
pub fn init(app_data: &Path) {
    let Some(saved) = std::fs::read(app_data.join(FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<Saved>(&b).ok())
    else {
        return;
    };
    let dir = PathBuf::from(saved.dir);
    if dir.is_dir() {
        paths::set_active(Some(dir));
    }
}

/// Ayarlardan seçim: yalnızca algılanan profillerden biri kabul edilir.
pub fn select(app_data: &Path, dir: &str) -> Result<(), String> {
    let p = list()
        .into_iter()
        .find(|p| p.dir == dir)
        .ok_or("unknown profile")?;
    let default = paths::default_claude_dir();
    let dir = PathBuf::from(&p.dir);
    if default.as_deref().is_some_and(|d| same(d, &dir)) {
        let _ = std::fs::remove_file(app_data.join(FILE));
        paths::set_active(None);
    } else {
        std::fs::create_dir_all(app_data).map_err(|e| e.to_string())?;
        let body = serde_json::to_vec(&Saved { dir: p.dir }).map_err(|e| e.to_string())?;
        std::fs::write(app_data.join(FILE), body).map_err(|e| e.to_string())?;
        paths::set_active(Some(dir));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_aliases() {
        let home = Path::new("/Users/me");
        let rc = r#"
alias ll='ls -la'
alias claude-personal='CLAUDE_CONFIG_DIR="$HOME/.claude" claude'
alias claude-work='CLAUDE_CONFIG_DIR="$HOME/.claude-work" claude'
  alias cw="CLAUDE_CONFIG_DIR=~/.claude-client claude --model opus"
# alias old='CLAUDE_CONFIG_DIR=/x claude'
alias other='FOO=1 bar'
"#;
        let a = aliases(rc, home);
        assert_eq!(
            a,
            vec![
                ("claude-personal".into(), home.join(".claude")),
                ("claude-work".into(), home.join(".claude-work")),
                ("cw".into(), home.join(".claude-client")),
            ]
        );
    }

    #[test]
    fn ids_and_file_names() {
        let home = Path::new("/Users/me");
        assert_eq!(id_for(&home.join(".claude"), home), "");
        assert_eq!(id_for(&home.join(".claude-work"), home), "claude-work");
        assert_eq!(id_for(Path::new("/opt/My Claude"), home), "My-Claude");
        assert_eq!(file_name("claude-limits.json", ""), "claude-limits.json");
        assert_eq!(
            file_name("claude-limits.json", "claude-work"),
            "claude-limits-claude-work.json"
        );
    }

    #[test]
    fn detects_profiles_from_aliases_and_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        for d in [".claude/projects", ".claude-work/projects", ".claude-empty"] {
            std::fs::create_dir_all(home.join(d)).unwrap();
        }
        std::fs::write(home.join(".claude.json"), "{}").unwrap();
        let rc = vec![
            "alias claude-work='CLAUDE_CONFIG_DIR=\"$HOME/.claude-work\" claude'\n\
             alias claude-gone='CLAUDE_CONFIG_DIR=\"$HOME/.claude-gone\" claude'\n"
                .to_string(),
        ];
        let default = home.join(".claude");
        let list = detect(home, &default, &rc, &home.join(".claude-work"));
        let names: Vec<_> = list.iter().map(|p| (p.name.as_str(), p.active)).collect();
        // Boş klasör (projects/settings yok) ve var olmayan klasör listede yok; .claude.json dosya
        assert_eq!(names, vec![("default", false), ("work", true)]);
        assert_eq!(list[1].commands, vec!["claude-work".to_string()]);
        assert_eq!(list[1].id, "claude-work");
        assert_eq!(list[0].commands, vec!["claude".to_string()]);
    }
}
