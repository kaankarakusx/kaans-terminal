//! Harici araçların ayarlarına dokunan entegrasyonlar (MIMARI.md §7). Kurallar:
//! 1. İzinsiz değişiklik yok: yalnızca kullanıcı Entegrasyonlar ekranında açınca.
//! 2. Her değişiklikten önce yedek: `<app_data>/backups/<dosya>.<zaman>`.
//! 3. Var olan ayar korunur: kullanıcının kendi statusLine'ı `statusline-next.json`'a taşınır ve çağrılmaya devam eder.
//! 4. JSON olarak okunamayan dosyaya dokunulmaz.
//! 5. Hepsi geri alınabilir ("bütün entegrasyonları kaldır").
//!
//! Claude limitleri (statusLine): Claude Code statusLine komutuna stdin'den JSON verir; içindeki `rate_limits`
//! uygulamanın kendi ikili dosyası (`--statusline` kipi) tarafından yerel bir dosyaya yazılır. Ağ erişimi yoktur.

use serde::Serialize;
use serde_json::{Map, Value};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const STATUSLINE_ARG: &str = "--statusline";
const NEXT_FILE: &str = "statusline-next.json";
pub const LIMITS_FILE: &str = "claude-limits.json";

/// Tauri'nin `app_data_dir()`'i ile aynı klasör (statusLine kipinde Tauri yok).
pub fn app_data_dir() -> Option<PathBuf> {
    const ID: &str = "com.kaankarakus.kaans-terminal";
    #[cfg(target_os = "macos")]
    {
        std::env::home_dir().map(|h| h.join("Library/Application Support").join(ID))
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join(ID))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::home_dir().map(|h| h.join(".local/share")))
            .map(|d| d.join(ID))
    }
}

impl Paths {
    fn own(&self, base: &str) -> PathBuf {
        self.app_data
            .join(crate::profiles::file_name(base, &self.profile))
    }
}

pub struct Paths {
    /// `$CLAUDE_DIR/settings.json`
    pub claude_settings: PathBuf,
    pub app_data: PathBuf,
    /// Uygulamanın ikili dosyası
    pub exe: PathBuf,
    /// Claude profili soneki (`profiles::id_for`; varsayılan `~/.claude` için boş): uygulamanın bu profile ait
    /// dosyaları (`statusline-next-<id>.json`, `claude-theme-prev-<id>.json`) ayrı tutulur.
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub id: &'static str,
    pub enabled: bool,
    /// Zincirlenen (kullanıcının önceki) statusLine komutu
    pub chained: Option<String>,
    /// Dosya okunamıyorsa: dokunulmadı
    pub error: Option<String>,
    /// Geliştirme derlemesine işaret ediyor (target/debug)
    pub dev_build: bool,
}

fn read_json(path: &Path) -> Result<Map<String, Value>, String> {
    match std::fs::read(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(format!("{} okunamadı: {e}", path.display())),
        Ok(b) if b.iter().all(u8::is_ascii_whitespace) => Ok(Map::new()),
        Ok(b) => match serde_json::from_slice::<Value>(&b) {
            Ok(Value::Object(m)) => Ok(m),
            _ => Err(format!(
                "{} geçerli bir JSON nesnesi değil; dokunulmadı",
                path.display()
            )),
        },
    }
}

/// Değiştirmeden önce yedek alır (dosya yoksa yedek yok), sonra geçici dosya + taşıma ile yazar.
fn write_json_with_backup(
    path: &Path,
    map: &Map<String, Value>,
    app_data: &Path,
) -> Result<(), String> {
    if path.exists() {
        let dir = app_data.join("backups");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S%3f");
        std::fs::copy(path, dir.join(format!("{name}.{stamp}")))
            .map_err(|e| format!("yedek alınamadı: {e}"))?;
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut text =
        serde_json::to_string_pretty(&Value::Object(map.clone())).map_err(|e| e.to_string())?;
    text.push('\n');
    let tmp = path.with_extension("json.kaans-tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Kabukta güvenle çalışacak komut (uygulama adında tırnak var: "Kaan's Terminal").
fn command_for(exe: &Path) -> String {
    let p = exe.to_string_lossy();
    if cfg!(windows) {
        format!("\"{p}\" {STATUSLINE_ARG}")
    } else {
        format!("'{}' {STATUSLINE_ARG}", p.replace('\'', r"'\''"))
    }
}

fn is_ours(statusline: &Value) -> bool {
    statusline
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(STATUSLINE_ARG) && c.contains("kaans-terminal"))
}

pub fn statusline_status(p: &Paths) -> Status {
    let dev_build = p.exe.to_string_lossy().contains("/target/debug/");
    let chained = read_json(&p.own(NEXT_FILE))
        .ok()
        .and_then(|m| m.get("command").and_then(Value::as_str).map(str::to_string));
    match read_json(&p.claude_settings) {
        Ok(m) => Status {
            id: "claude-statusline",
            enabled: m.get("statusLine").is_some_and(is_ours),
            chained,
            error: None,
            dev_build,
        },
        Err(e) => Status {
            id: "claude-statusline",
            enabled: false,
            chained,
            error: Some(e),
            dev_build,
        },
    }
}

pub fn enable_statusline(p: &Paths) -> Result<(), String> {
    let mut m = read_json(&p.claude_settings)?;
    if m.get("statusLine").is_some_and(is_ours) {
        return Ok(());
    }
    // Kullanıcının kendi statusLine'ı saklanır ve bizimkinden çağrılmaya devam eder.
    if let Some(prev) = m.get("statusLine").cloned() {
        std::fs::create_dir_all(&p.app_data).map_err(|e| e.to_string())?;
        let prev_obj = match prev {
            Value::Object(o) => o,
            other => {
                let mut o = Map::new();
                o.insert("value".into(), other);
                o
            }
        };
        write_json_with_backup(&p.own(NEXT_FILE), &prev_obj, &p.app_data)?;
    }
    let mut ours = Map::new();
    ours.insert("type".into(), Value::String("command".into()));
    ours.insert("command".into(), Value::String(command_for(&p.exe)));
    m.insert("statusLine".into(), Value::Object(ours));
    write_json_with_backup(&p.claude_settings, &m, &p.app_data)
}

pub fn disable_statusline(p: &Paths) -> Result<(), String> {
    let mut m = read_json(&p.claude_settings)?;
    if !m.get("statusLine").is_some_and(is_ours) {
        return Ok(());
    }
    let next_path = p.own(NEXT_FILE);
    match read_json(&next_path) {
        Ok(prev) if !prev.is_empty() => {
            // Önceki ayar geri gelir (sarmalanmış düz değer ise açılır).
            let restored = prev
                .get("value")
                .cloned()
                .filter(|_| prev.len() == 1)
                .unwrap_or(Value::Object(prev));
            m.insert("statusLine".into(), restored);
        }
        _ => {
            m.remove("statusLine");
        }
    }
    write_json_with_backup(&p.claude_settings, &m, &p.app_data)?;
    let _ = std::fs::remove_file(next_path);
    Ok(())
}

// ───────────────────────── Claude teması ─────────────────────────
// Claude Code'un `theme` anahtarı (`~/.claude/settings.json`); `dark-ansi` terminalin 16 rengini kullanır, böylece
// uygulamada tema değişince Claude'un yazıları da değişir. Kapatılınca önceki tema (ya da hiç olmaması) geri gelir.

const THEME_PREV: &str = "claude-theme-prev.json";
pub const CLAUDE_ANSI_THEME: &str = "dark-ansi";

pub fn theme_status(p: &Paths) -> Status {
    match read_json(&p.claude_settings) {
        Ok(m) => Status {
            id: "claude-theme",
            enabled: m.get("theme").and_then(Value::as_str) == Some(CLAUDE_ANSI_THEME)
                && p.own(THEME_PREV).exists(),
            chained: m.get("theme").and_then(Value::as_str).map(str::to_string),
            error: None,
            dev_build: false,
        },
        Err(e) => Status {
            id: "claude-theme",
            enabled: false,
            chained: None,
            error: Some(e),
            dev_build: false,
        },
    }
}

pub fn enable_theme(p: &Paths) -> Result<(), String> {
    let mut m = read_json(&p.claude_settings)?;
    if p.own(THEME_PREV).exists()
        && m.get("theme").and_then(Value::as_str) == Some(CLAUDE_ANSI_THEME)
    {
        return Ok(());
    }
    // Önceki değer (yoksa null) saklanır.
    std::fs::create_dir_all(&p.app_data).map_err(|e| e.to_string())?;
    let mut prev = Map::new();
    prev.insert(
        "theme".into(),
        m.get("theme").cloned().unwrap_or(Value::Null),
    );
    write_json_with_backup(&p.own(THEME_PREV), &prev, &p.app_data)?;
    m.insert("theme".into(), Value::String(CLAUDE_ANSI_THEME.into()));
    write_json_with_backup(&p.claude_settings, &m, &p.app_data)
}

pub fn disable_theme(p: &Paths) -> Result<(), String> {
    let prev_path = p.own(THEME_PREV);
    let Ok(prev) = read_json(&prev_path) else {
        return Ok(());
    };
    if prev.is_empty() {
        return Ok(());
    }
    let mut m = read_json(&p.claude_settings)?;
    // Kullanıcı bu arada temayı kendisi değiştirdiyse onun seçimi kalır.
    if m.get("theme").and_then(Value::as_str) == Some(CLAUDE_ANSI_THEME) {
        match prev.get("theme") {
            Some(Value::Null) | None => {
                m.remove("theme");
            }
            Some(v) => {
                m.insert("theme".into(), v.clone());
            }
        }
        write_json_with_backup(&p.claude_settings, &m, &p.app_data)?;
    }
    let _ = std::fs::remove_file(prev_path);
    Ok(())
}

/// Bütün entegrasyonları kaldır (ayar dosyası tabanlılar; rtk/caveman `savers`'ta).
pub fn disable_all(p: &Paths) -> Result<(), String> {
    let a = disable_statusline(p);
    let b = disable_theme(p);
    a.and(b)
}

/// `--statusline` kipi: stdin'deki JSON'dan yalnızca `rate_limits`'i yerel dosyaya yazar, sonra kullanıcının önceki
/// statusLine komutunu aynı girdiyle çalıştırıp çıktısını basar. Hiçbir zaman hata vermez (status satırı bozulmasın).
/// Profil, Claude'un bu sürece aktardığı `CLAUDE_CONFIG_DIR`'den çıkar (`claude-work` → `claude-limits-claude-work.json`).
pub fn run_statusline(app_data: &Path) -> i32 {
    let mut input = Vec::new();
    let _ = io::stdin().read_to_end(&mut input);
    let profile = std::env::home_dir()
        .map(|home| {
            let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
                .filter(|d| !d.is_empty())
                .map_or_else(|| home.join(".claude"), PathBuf::from);
            crate::profiles::id_for(&dir, &home)
        })
        .unwrap_or_default();
    statusline(&input, app_data, &profile, &mut io::stdout())
}

fn statusline(input: &[u8], app_data: &Path, profile: &str, out: &mut impl Write) -> i32 {
    let limits_file = crate::profiles::file_name(LIMITS_FILE, profile);
    if let Ok(v) = serde_json::from_slice::<Value>(input)
        && let Some(rl) = v.get("rate_limits")
    {
        let body = serde_json::json!({ "updated": chrono::Utc::now().timestamp_millis(), "rate_limits": rl });
        let _ = std::fs::create_dir_all(app_data);
        let tmp = app_data.join(format!("{limits_file}.tmp"));
        if std::fs::write(&tmp, body.to_string()).is_ok() {
            let _ = std::fs::rename(&tmp, app_data.join(&limits_file));
        }
    }
    let Some(cmd) = read_json(&app_data.join(crate::profiles::file_name(NEXT_FILE, profile)))
        .ok()
        .and_then(|m| m.get("command").and_then(Value::as_str).map(str::to_string))
    else {
        return 0;
    };
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(&cmd);
        c
    } else {
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(&cmd);
        c
    };
    let Ok(mut child) = c
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return 0;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input);
    }
    match child.wait_with_output() {
        Ok(res) => {
            let _ = out.write_all(&res.stdout);
            res.status.code().unwrap_or(0)
        }
        Err(_) => 0,
    }
}

/// `claude-limits.json` → (5 saatlik, haftalık). `resets_at` saniye, milisaniye ya da RFC3339 olabilir.
pub fn read_claude_limits(app_data: &Path, profile: &str) -> Option<crate::engine::ToolLimits> {
    use crate::engine::transcript::LimitWindow;
    let file = crate::profiles::file_name(LIMITS_FILE, profile);
    let v: Value = serde_json::from_slice(&std::fs::read(app_data.join(file)).ok()?).ok()?;
    let rl = v.get("rate_limits")?;
    let window = |key: &str, minutes: u64| -> Option<LimitWindow> {
        let w = rl.get(key)?;
        let used = w
            .get("used_percentage")
            .or_else(|| w.get("used_percent"))?
            .as_f64()?;
        let resets_at = match w.get("resets_at") {
            Some(Value::Number(n)) => {
                let x = n.as_f64().unwrap_or(0.0);
                if x > 1e12 {
                    (x / 1000.0) as i64
                } else {
                    x as i64
                }
            }
            Some(Value::String(s)) => chrono::DateTime::parse_from_rfc3339(s)
                .map(|d| d.timestamp())
                .unwrap_or(0),
            _ => 0,
        };
        Some(LimitWindow {
            used_pct: used,
            window_minutes: minutes,
            resets_at,
        })
    };
    Some(crate::engine::ToolLimits {
        tool: "claude".into(),
        five_hour: window("five_hour", 300),
        weekly: window("seven_day", 7 * 24 * 60),
        updated: v.get("updated").and_then(Value::as_i64).unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn setup() -> (tempfile::TempDir, Paths) {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths {
            claude_settings: tmp.path().join("claude/settings.json"),
            app_data: tmp.path().join("app"),
            exe: PathBuf::from("/Applications/Kaan's Terminal.app/Contents/MacOS/kaans-terminal"),
            profile: String::new(),
        };
        (tmp, p)
    }

    fn backups(p: &Paths) -> usize {
        fs::read_dir(p.app_data.join("backups"))
            .map(|d| d.count())
            .unwrap_or(0)
    }

    #[test]
    fn enable_chains_existing_statusline_and_disable_restores_it() {
        let (_t, p) = setup();
        fs::create_dir_all(p.claude_settings.parent().unwrap()).unwrap();
        let original = r#"{
  "model": "opus",
  "statusLine": { "type": "command", "command": "~/bin/my-status" },
  "zeta": 1,
  "alpha": 2
}
"#;
        fs::write(&p.claude_settings, original).unwrap();

        enable_statusline(&p).unwrap();
        let st = statusline_status(&p);
        assert!(st.enabled);
        assert_eq!(st.chained.as_deref(), Some("~/bin/my-status"));
        let m = read_json(&p.claude_settings).unwrap();
        let cmd = m["statusLine"]["command"].as_str().unwrap();
        assert_eq!(
            cmd,
            r"'/Applications/Kaan'\''s Terminal.app/Contents/MacOS/kaans-terminal' --statusline"
        );
        assert_eq!(
            m.keys().collect::<Vec<_>>(),
            ["model", "statusLine", "zeta", "alpha"],
            "anahtar sırası korunur"
        );
        assert_eq!(backups(&p), 1, "değişiklikten önce yedek");

        enable_statusline(&p).unwrap();
        assert_eq!(backups(&p), 1, "zaten açıksa dosyaya dokunulmaz");

        disable_statusline(&p).unwrap();
        let m = read_json(&p.claude_settings).unwrap();
        assert_eq!(
            m["statusLine"],
            json!({"type":"command","command":"~/bin/my-status"})
        );
        assert!(!statusline_status(&p).enabled);
        assert!(statusline_status(&p).chained.is_none());
        assert_eq!(m["model"], "opus");
    }

    #[test]
    fn enable_without_existing_and_disable_removes_key() {
        let (_t, p) = setup();
        enable_statusline(&p).unwrap();
        assert!(statusline_status(&p).enabled);
        assert_eq!(backups(&p), 0, "dosya yoktu: yedek yok");
        disable_all(&p).unwrap();
        let m = read_json(&p.claude_settings).unwrap();
        assert!(!m.contains_key("statusLine"));
    }

    #[test]
    fn never_touches_unparseable_settings() {
        let (_t, p) = setup();
        fs::create_dir_all(p.claude_settings.parent().unwrap()).unwrap();
        fs::write(&p.claude_settings, "{ // yorumlu json\n }").unwrap();
        assert!(enable_statusline(&p).is_err());
        assert_eq!(
            fs::read_to_string(&p.claude_settings).unwrap(),
            "{ // yorumlu json\n }"
        );
        assert!(statusline_status(&p).error.is_some());
    }

    #[test]
    fn statusline_saves_only_rate_limits_and_runs_chained_command() {
        let (_t, p) = setup();
        fs::create_dir_all(&p.app_data).unwrap();
        let input = json!({"model":{"id":"claude-opus-5-5"},"cwd":"/secret/project",
            "rate_limits":{"five_hour":{"used_percentage":12,"resets_at":1}}})
        .to_string();
        // Zincir yok: hiçbir şey basılmaz
        let mut out = Vec::new();
        assert_eq!(statusline(input.as_bytes(), &p.app_data, "", &mut out), 0);
        assert!(out.is_empty());
        let saved = fs::read_to_string(p.app_data.join(LIMITS_FILE)).unwrap();
        assert!(
            saved.contains("used_percentage") && !saved.contains("/secret/project"),
            "yalnızca rate_limits"
        );
        // Zincir: önceki komut aynı girdiyi alır, çıktısı basılır
        fs::write(
            p.app_data.join(NEXT_FILE),
            json!({"type":"command","command":"head -c 9; printf ' ok'"}).to_string(),
        )
        .unwrap();
        let mut out = Vec::new();
        statusline(input.as_bytes(), &p.app_data, "", &mut out);
        assert_eq!(String::from_utf8_lossy(&out), "{\"model\": ok");
    }

    #[test]
    fn theme_switches_to_ansi_and_restores_previous_or_user_choice() {
        let (_t, p) = setup();
        fs::create_dir_all(p.claude_settings.parent().unwrap()).unwrap();
        fs::write(&p.claude_settings, r#"{"theme":"light","model":"opus"}"#).unwrap();
        enable_theme(&p).unwrap();
        assert!(theme_status(&p).enabled);
        assert_eq!(read_json(&p.claude_settings).unwrap()["theme"], "dark-ansi");
        disable_theme(&p).unwrap();
        assert_eq!(read_json(&p.claude_settings).unwrap()["theme"], "light");
        assert!(!theme_status(&p).enabled);

        // Önceden tema yoktu → kapatınca anahtar kalkar.
        fs::write(&p.claude_settings, r#"{"model":"opus"}"#).unwrap();
        enable_theme(&p).unwrap();
        disable_theme(&p).unwrap();
        assert!(!read_json(&p.claude_settings).unwrap().contains_key("theme"));

        // Kullanıcı arada kendisi değiştirdiyse dokunulmaz.
        enable_theme(&p).unwrap();
        let mut m = read_json(&p.claude_settings).unwrap();
        m.insert("theme".into(), json!("dark-daltonized"));
        fs::write(&p.claude_settings, serde_json::to_string(&m).unwrap()).unwrap();
        disable_theme(&p).unwrap();
        assert_eq!(
            read_json(&p.claude_settings).unwrap()["theme"],
            "dark-daltonized"
        );
    }

    #[test]
    fn reads_limits_in_any_reset_format() {
        let (_t, p) = setup();
        fs::create_dir_all(&p.app_data).unwrap();
        fs::write(
            p.app_data.join(LIMITS_FILE),
            json!({"updated": 5, "rate_limits": {
                "five_hour": {"used_percentage": 42.5, "resets_at": 1794150000},
                "seven_day": {"used_percentage": 18, "resets_at": "2026-11-12T09:00:00Z"}}})
            .to_string(),
        )
        .unwrap();
        let l = read_claude_limits(&p.app_data, "").unwrap();
        assert_eq!(l.five_hour.unwrap().resets_at, 1_794_150_000);
        assert!((l.five_hour.unwrap().used_pct - 42.5).abs() < 1e-9);
        assert_eq!(l.weekly.unwrap().window_minutes, 10080);
        assert_eq!(l.weekly.unwrap().resets_at, 1_794_474_000);
        assert_eq!(l.updated, 5);
    }
}
