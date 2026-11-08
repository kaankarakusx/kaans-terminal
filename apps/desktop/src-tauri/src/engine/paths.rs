//! Claude Code klasör yolları (MIMARI.md §3.1).

use std::path::PathBuf;

/// `$CLAUDE_DIR`: `CLAUDE_CONFIG_DIR` tanımlıysa o, değilse `~/.claude`.
pub fn claude_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    std::env::home_dir().map(|home| home.join(".claude"))
}

/// Claude Code'un proje klasörü adı: cwd içindeki harf/rakam dışındaki her karakter `-` olur.
/// `/Users/me/my.app` → `-Users-me-my-app`
pub fn encode_cwd(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// `sessions/` içinde yalnızca `^\d+\.json$` okunur. `.key` dosyaları **asla** okunmaz.
pub fn is_registry_file(name: &str) -> bool {
    name.strip_suffix(".json")
        .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_digit()))
}

/// Klasör yolunun son parçası (kartta proje adı olarak gösterilir).
pub fn project_name(cwd: &str) -> String {
    cwd.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(cwd)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_cwd_like_claude_code() {
        assert_eq!(
            encode_cwd("/Users/kaan/Desktop/kaans-terminal"),
            "-Users-kaan-Desktop-kaans-terminal"
        );
        assert_eq!(encode_cwd("/a/my.app_v2"), "-a-my-app-v2");
        assert_eq!(encode_cwd(r"C:\Users\kaan"), "C--Users-kaan");
    }

    #[test]
    fn registry_filter_never_matches_key_files() {
        assert!(is_registry_file("4766.json"));
        assert!(!is_registry_file("4766.abcdef.key"));
        assert!(!is_registry_file("4766.json.key"));
        assert!(!is_registry_file("abc.json"));
        assert!(!is_registry_file(".json"));
        assert!(!is_registry_file("4766.json.tmp"));
    }

    #[test]
    fn project_name_is_last_segment() {
        assert_eq!(project_name("/Users/kaan/proj/"), "proj");
        assert_eq!(project_name(r"C:\work\api"), "api");
        assert_eq!(project_name("/"), "/");
    }
}
