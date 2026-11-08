//! Claude Code ayarlarından okunanlar (salt okunur). Şimdilik yalnızca `/autocompact` penceresi.

use serde_json::Value;
use std::path::Path;

/// "500k", "1M", "200000", 200000 → token sayısı.
pub fn parse_window(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64().filter(|&n| n > 0),
        Value::String(s) => parse_window_str(s),
        _ => None,
    }
}

fn parse_window_str(s: &str) -> Option<u64> {
    let s = s.trim().replace(['_', ','], "");
    let (num, mult) = match s.chars().last()? {
        'k' | 'K' => (&s[..s.len() - 1], 1_000.0),
        'm' | 'M' => (&s[..s.len() - 1], 1_000_000.0),
        _ => (s.as_str(), 1.0),
    };
    let n: f64 = num.trim().parse().ok()?;
    (n > 0.0).then(|| (n * mult).round() as u64)
}

fn from_file(path: &Path) -> Option<u64> {
    let v: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    v.get("autoCompactWindow").and_then(parse_window)
}

/// Öncelik: `CLAUDE_CODE_AUTO_COMPACT_WINDOW` → proje `settings.local.json` → proje `settings.json`
/// → kullanıcı `settings.json`.
pub fn auto_compact_window(env: Option<&str>, claude_dir: &Path, cwd: &str) -> Option<u64> {
    if let Some(w) = env.and_then(parse_window_str) {
        return Some(w);
    }
    if !cwd.is_empty() {
        let project = Path::new(cwd).join(".claude");
        if let Some(w) = from_file(&project.join("settings.local.json"))
            .or_else(|| from_file(&project.join("settings.json")))
        {
            return Some(w);
        }
    }
    from_file(&claude_dir.join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn parses_window_formats() {
        assert_eq!(parse_window(&json!("500k")), Some(500_000));
        assert_eq!(parse_window(&json!("1M")), Some(1_000_000));
        assert_eq!(parse_window(&json!("1.5m")), Some(1_500_000));
        assert_eq!(parse_window(&json!("200000")), Some(200_000));
        assert_eq!(parse_window(&json!(200000)), Some(200_000));
        assert_eq!(parse_window(&json!("abc")), None);
        assert_eq!(parse_window(&json!("0")), None);
        assert_eq!(parse_window(&json!(null)), None);
    }

    #[test]
    fn precedence() {
        let tmp = tempfile::tempdir().unwrap();
        let user = tmp.path().join("claude");
        let proj = tmp.path().join("proj");
        fs::create_dir_all(&user).unwrap();
        fs::create_dir_all(proj.join(".claude")).unwrap();
        let cwd = proj.to_str().unwrap();

        assert_eq!(auto_compact_window(None, &user, cwd), None);
        fs::write(
            user.join("settings.json"),
            r#"{"autoCompactWindow":"300k"}"#,
        )
        .unwrap();
        assert_eq!(auto_compact_window(None, &user, cwd), Some(300_000));
        fs::write(
            proj.join(".claude/settings.json"),
            r#"{"autoCompactWindow":"400k"}"#,
        )
        .unwrap();
        assert_eq!(auto_compact_window(None, &user, cwd), Some(400_000));
        fs::write(
            proj.join(".claude/settings.local.json"),
            r#"{"autoCompactWindow":"500k"}"#,
        )
        .unwrap();
        assert_eq!(auto_compact_window(None, &user, cwd), Some(500_000));
        assert_eq!(auto_compact_window(Some("1M"), &user, cwd), Some(1_000_000));
        // bozuk dosya atlanır, alttaki kaynağa düşülür
        fs::write(proj.join(".claude/settings.local.json"), b"{ bozuk").unwrap();
        assert_eq!(auto_compact_window(None, &user, cwd), Some(400_000));
    }
}
