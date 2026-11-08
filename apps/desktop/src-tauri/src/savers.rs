//! Token tasarrufu araçları (OZELLIKLER.md §7): rtk ve caveman. Varsayılan kapalı; Entegrasyonlar ekranında
//! kullanıcı açınca kurulur, kapatınca kaldırılır.
//!
//! rtk (rtk-oss, Apache-2.0, sabit sürüm + SHA-256 ile pakette):
//! - Kullanıcının kendi rtk'si (PATH'te, örn. Homebrew) varsa o kullanılır ve ona dokunulmaz.
//! - Yoksa paketteki kopya `~/.local/bin/rtk`'ya konur (rtk'nin hook'u çıplak `rtk` çağırır); uygulama yalnızca kendi
//!   koyduğu kopyayı günceller.
//! - Hook'lar rtk'nin kendi komutlarıyla: `rtk init -g --auto-patch` (Claude), `--codex`, `--agent antigravity`;
//!   kaldırma `--uninstall`. Önce bizim yedeğimiz alınır; telemetri kapalı tutulur.
//!
//! caveman (v4): kendi resmi eklenti komutlarıyla (`claude plugin …`, `gemini extensions …`); hiçbir şey gömülmez.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CAVEMAN_REPO: &str = "JuliusBrussee/caveman";
const CAVEMAN_PLUGIN: &str = "caveman@caveman";
const RTK_MANAGED: &str = "rtk-managed.json";
const CAVEMAN_STATE: &str = "caveman-state.json";

pub struct Env {
    pub home: PathBuf,
    pub claude_dir: PathBuf,
    pub codex_dir: PathBuf,
    pub app_data: PathBuf,
    /// Paketteki rtk (yoksa None)
    pub bundled_rtk: Option<PathBuf>,
    /// Testlerde alt süreçlerin HOME'u geçici klasöre çevrilir; uygulamada None.
    pub home_override: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaverStatus {
    pub id: &'static str,
    pub enabled: bool,
    /// Hangi araçlara uygulanır / uygulanmış
    pub agents: Vec<String>,
    /// Kullanılan kopya: "yours" (kullanıcının), "bundled" (uygulamanın), "missing"
    pub source: String,
    pub detail: Option<String>,
    pub error: Option<String>,
}

/// Uygulamanın koyduğu rtk kopyasının klasörü (kayıt dosyasından; Tauri dışında da çağrılabilir).
pub fn managed_bin_dir_from(app_data: &Path) -> Option<PathBuf> {
    let v: Value = serde_json::from_slice(&std::fs::read(app_data.join(RTK_MANAGED)).ok()?).ok()?;
    let p = PathBuf::from(v.get("path")?.as_str()?);
    p.exists()
        .then(|| p.parent().map(Path::to_path_buf))
        .flatten()
}

/// Paketteki rtk'nin göreli yolu (`resources/bin/<hedef>/rtk`).
pub fn bundled_rtk_rel() -> Option<String> {
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => return None,
    };
    let bin = if cfg!(windows) { "rtk.exe" } else { "rtk" };
    Some(format!("resources/bin/{target}/{bin}"))
}

fn sha256_file(p: &Path) -> Option<String> {
    let bytes = std::fs::read(p).ok()?;
    Some(format!("{:x}", Sha256::digest(&bytes)))
}

impl Env {
    fn command(&self, program: &Path, args: &[&str]) -> Command {
        let mut c = Command::new(program);
        c.args(args)
            .stdin(Stdio::null())
            .env("RTK_TELEMETRY_DISABLED", "1")
            .env("DO_NOT_TRACK", "1");
        if self.home_override {
            c.env("HOME", &self.home)
                .env("CODEX_HOME", &self.codex_dir)
                .env("CLAUDE_CONFIG_DIR", &self.claude_dir);
        }
        c
    }

    /// Kullanıcının giriş kabuğunda çalıştırır (Finder'dan açılan uygulamanın eksik PATH'i tamamlansın).
    fn login_shell(&self, script: &str) -> Command {
        #[cfg(unix)]
        {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
            let mut c = Command::new(shell);
            c.arg("-lc").arg(script).stdin(Stdio::null());
            if self.home_override {
                c.env("HOME", &self.home);
            }
            c
        }
        #[cfg(windows)]
        {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(script).stdin(Stdio::null());
            c
        }
    }

    fn run(&self, mut c: Command) -> Result<String, String> {
        let out = c.output().map_err(|e| e.to_string())?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if out.status.success() {
            Ok(text)
        } else {
            Err(text
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("başarısız")
                .trim()
                .to_string())
        }
    }

    fn which(&self, name: &str) -> Option<PathBuf> {
        let script = if cfg!(windows) {
            format!("where {name}")
        } else {
            format!("command -v {name}")
        };
        let out = self.login_shell(&script).output().ok()?;
        let p = String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()?
            .trim()
            .to_string();
        (out.status.success() && !p.is_empty()).then(|| PathBuf::from(p))
    }

    fn managed_path(&self) -> PathBuf {
        if cfg!(windows) {
            self.home
                .join("AppData/Local/Programs/kaans-terminal/bin/rtk.exe")
        } else {
            self.home.join(".local/bin/rtk")
        }
    }

    /// Uygulamanın koyduğu kopya (kaydı: yol + sha256).
    fn managed_record(&self) -> Option<(PathBuf, String)> {
        let v: Value =
            serde_json::from_slice(&std::fs::read(self.app_data.join(RTK_MANAGED)).ok()?).ok()?;
        Some((
            PathBuf::from(v.get("path")?.as_str()?),
            v.get("sha256")?.as_str()?.to_string(),
        ))
    }

    /// Kullanılacak rtk: kullanıcının kendi kopyası öncelikli; yoksa uygulamanınki (gerekirse kurulur/güncellenir).
    fn resolve_rtk(&self, install: bool) -> Result<(PathBuf, &'static str), String> {
        let managed = self.managed_path();
        let ours = self.managed_record().map(|(p, _)| p);
        if let Some(p) = self.which("rtk")
            && ours.as_deref() != Some(p.as_path())
            && p != managed
        {
            return Ok((p, "yours"));
        }
        let bundled = self
            .bundled_rtk
            .clone()
            .ok_or("bu platform için gömülü rtk yok")?;
        let bundled_sha = sha256_file(&bundled).ok_or("gömülü rtk okunamadı")?;
        let current = sha256_file(&managed);
        let recorded = self.managed_record().map(|(_, s)| s);
        if current.as_deref() == Some(bundled_sha.as_str()) {
            return Ok((managed, "bundled"));
        }
        // Yerinde bizim olmayan bir dosya varsa ezilmez.
        if current.is_some() && current != recorded {
            return Err(format!(
                "{} uygulamanın kopyası değil; dokunulmadı",
                managed.display()
            ));
        }
        if !install {
            return Ok((managed, "missing"));
        }
        // Eskimiş (ya da hiç olmayan) kendi kopyamız: paketteki sürümle değiştir.
        std::fs::create_dir_all(managed.parent().unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?;
        let tmp = managed.with_extension("kaans-tmp");
        std::fs::copy(&bundled, &tmp).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        std::fs::rename(&tmp, &managed).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&self.app_data).map_err(|e| e.to_string())?;
        std::fs::write(
            self.app_data.join(RTK_MANAGED),
            serde_json::json!({ "path": managed, "sha256": bundled_sha }).to_string(),
        )
        .map_err(|e| e.to_string())?;
        Ok((managed, "bundled"))
    }

    /// Uygulama açılınca: kendi koyduğumuz kopya eskiyse paketteki sürümle değiştirilir (kullanıcınınkine dokunulmaz).
    pub fn refresh_managed_rtk(&self) {
        if self.managed_record().is_some() {
            let _ = self.resolve_rtk(true);
        }
    }

    /// Uygulama terminallerinin PATH'ine eklenecek klasör (yalnızca kendi kopyamız kuruluysa).
    pub fn managed_bin_dir(&self) -> Option<PathBuf> {
        let (p, _) = self.managed_record()?;
        p.exists()
            .then(|| p.parent().map(Path::to_path_buf))
            .flatten()
    }

    fn rtk_agents(&self) -> Vec<(&'static str, Vec<&'static str>)> {
        let mut v = vec![("claude", vec![])];
        if self.codex_dir.exists() {
            v.push(("codex", vec!["--codex"]));
        }
        if self.home.join(".gemini/config").exists() {
            v.push(("antigravity", vec!["--agent", "antigravity"]));
        }
        v
    }

    fn backup(&self, files: &[PathBuf]) -> Result<(), String> {
        let dir = self.app_data.join("backups");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S%3f");
        for f in files.iter().filter(|f| f.exists()) {
            let name = f
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            std::fs::copy(f, dir.join(format!("{name}.{stamp}")))
                .map_err(|e| format!("yedek alınamadı: {e}"))?;
        }
        Ok(())
    }

    fn rtk_files(&self) -> Vec<PathBuf> {
        vec![
            self.claude_dir.join("settings.json"),
            self.claude_dir.join("CLAUDE.md"),
            self.codex_dir.join("hooks.json"),
            self.codex_dir.join("AGENTS.md"),
        ]
    }

    pub fn rtk_status(&self) -> SaverStatus {
        let settings =
            std::fs::read_to_string(self.claude_dir.join("settings.json")).unwrap_or_default();
        let codex = std::fs::read_to_string(self.codex_dir.join("hooks.json")).unwrap_or_default();
        let mut agents = Vec::new();
        if settings.contains("rtk hook claude") {
            agents.push("claude".to_string());
        }
        if codex.contains("rtk hook codex") {
            agents.push("codex".to_string());
        }
        if self.home.join(".gemini/config/plugins/rtk").exists() {
            agents.push("antigravity".to_string());
        }
        let (source, error) = match self.resolve_rtk(false) {
            Ok((_, s)) => (s.to_string(), None),
            Err(e) => ("missing".to_string(), Some(e)),
        };
        SaverStatus {
            id: "rtk",
            enabled: agents.iter().any(|a| a == "claude"),
            agents,
            source,
            detail: Some("rtk-oss v1.1.0 · Apache-2.0".into()),
            error,
        }
    }

    pub fn enable_rtk(&self) -> Result<(), String> {
        let (rtk, _) = self.resolve_rtk(true)?;
        self.backup(&self.rtk_files())?;
        for (_, flags) in self.rtk_agents() {
            let mut args = vec!["init", "-g"];
            args.extend(flags);
            args.push("--auto-patch");
            self.run(self.command(&rtk, &args))?;
        }
        Ok(())
    }

    pub fn disable_rtk(&self) -> Result<(), String> {
        let (rtk, _) = self.resolve_rtk(false)?;
        if !rtk.exists() {
            return Err("rtk bulunamadı; hook'lar elle kaldırılmalı".into());
        }
        self.backup(&self.rtk_files())?;
        for (_, flags) in self.rtk_agents() {
            let mut args = vec!["init", "-g"];
            args.extend(flags);
            args.push("--uninstall");
            self.run(self.command(&rtk, &args))?;
        }
        Ok(())
    }

    // ───────────────────────── caveman ─────────────────────────

    fn caveman_installed(&self) -> bool {
        std::fs::read(self.claude_dir.join("plugins/installed_plugins.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| {
                v.get("plugins")
                    .and_then(Value::as_object)
                    .map(|m| m.keys().any(|k| k.starts_with("caveman@")))
            })
            .unwrap_or(false)
    }

    fn marketplace_known(&self) -> bool {
        std::fs::read(self.claude_dir.join("plugins/known_marketplaces.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .is_some_and(|v| v.get("caveman").is_some())
    }

    pub fn caveman_status(&self) -> SaverStatus {
        let mut agents = Vec::new();
        if self.caveman_installed() {
            agents.push("claude".to_string());
        }
        SaverStatus {
            id: "caveman",
            enabled: !agents.is_empty(),
            agents,
            source: "plugin".into(),
            detail: Some(format!(
                "Claude plugin {CAVEMAN_PLUGIN} · github.com/{CAVEMAN_REPO}"
            )),
            error: None,
        }
    }

    pub fn enable_caveman(&self) -> Result<(), String> {
        let claude = self.which("claude").ok_or("claude komutu bulunamadı")?;
        let added = !self.marketplace_known();
        if added {
            self.run(self.command(&claude, &["plugin", "marketplace", "add", CAVEMAN_REPO]))?;
        }
        self.run(self.command(&claude, &["plugin", "install", CAVEMAN_PLUGIN]))?;
        std::fs::create_dir_all(&self.app_data).map_err(|e| e.to_string())?;
        let _ = std::fs::write(
            self.app_data.join(CAVEMAN_STATE),
            serde_json::json!({ "addedMarketplace": added }).to_string(),
        );
        if let Some(gemini) = self.which("gemini") {
            let url = format!("https://github.com/{CAVEMAN_REPO}");
            let _ = self.run(self.command(&gemini, &["extensions", "install", &url]));
        }
        Ok(())
    }

    pub fn disable_caveman(&self) -> Result<(), String> {
        let claude = self.which("claude").ok_or("claude komutu bulunamadı")?;
        if self.caveman_installed() {
            self.run(self.command(&claude, &["plugin", "uninstall", CAVEMAN_PLUGIN]))?;
        }
        let added = std::fs::read(self.app_data.join(CAVEMAN_STATE))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v.get("addedMarketplace").and_then(Value::as_bool))
            .unwrap_or(false);
        // Marketplace'i biz eklediysek kaldırırız; kullanıcınınkine dokunulmaz.
        if added {
            let _ =
                self.run(self.command(&claude, &["plugin", "marketplace", "remove", "caveman"]));
        }
        let _ = std::fs::remove_file(self.app_data.join(CAVEMAN_STATE));
        if let Some(gemini) = self.which("gemini") {
            let _ = self.run(self.command(&gemini, &["extensions", "uninstall", "caveman"]));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn env(tmp: &Path, bundled: Option<PathBuf>) -> Env {
        Env {
            home: tmp.join("home"),
            claude_dir: tmp.join("home/.claude"),
            codex_dir: tmp.join("home/.codex"),
            app_data: tmp.join("app"),
            bundled_rtk: bundled,
            home_override: true,
        }
    }

    fn repo_rtk() -> Option<PathBuf> {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(bundled_rtk_rel()?);
        p.exists().then_some(p)
    }

    #[test]
    fn caveman_status_reads_plugin_registry() {
        let tmp = tempfile::tempdir().unwrap();
        let e = env(tmp.path(), None);
        assert!(!e.caveman_status().enabled);
        fs::create_dir_all(e.claude_dir.join("plugins")).unwrap();
        fs::write(
            e.claude_dir.join("plugins/installed_plugins.json"),
            r#"{"version":2,"plugins":{"caveman@caveman":[{"scope":"user"}]}}"#,
        )
        .unwrap();
        assert!(e.caveman_status().enabled);
    }

    #[test]
    fn never_overwrites_a_foreign_rtk_at_the_managed_path() {
        let tmp = tempfile::tempdir().unwrap();
        let fake = tmp.path().join("bundled-rtk");
        fs::write(&fake, b"#!/bin/sh\necho bundled\n").unwrap();
        let e = env(tmp.path(), Some(fake));
        let managed = e.managed_path();
        fs::create_dir_all(managed.parent().unwrap()).unwrap();
        fs::write(&managed, b"someone else's rtk").unwrap();
        assert!(e.resolve_rtk(true).is_err());
        assert_eq!(fs::read(&managed).unwrap(), b"someone else's rtk");
    }

    /// Paketteki gerçek rtk-oss ile, geçici bir HOME içinde: kur → hook yazıldı → kaldır → hook gitti.
    /// (scripts/fetch-rtk.mjs çalıştırılmamışsa atlanır.)
    #[cfg(unix)]
    #[test]
    fn real_rtk_installs_and_uninstalls_in_a_sandbox_home() {
        let Some(bundled) = repo_rtk() else {
            eprintln!("gömülü rtk yok; atlandı");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let e = env(tmp.path(), Some(bundled.clone()));
        fs::create_dir_all(&e.claude_dir).unwrap();
        fs::write(
            e.claude_dir.join("settings.json"),
            "{\n  \"model\": \"opus\"\n}\n",
        )
        .unwrap();
        fs::write(e.claude_dir.join("CLAUDE.md"), "# my rules\n").unwrap();

        e.enable_rtk().expect("rtk kurulamadı");
        let st = e.rtk_status();
        assert!(st.enabled, "{st:?}");
        assert_eq!(st.source, "bundled");
        assert_eq!(
            sha256_file(&e.managed_path()),
            sha256_file(&bundled),
            "kopya paketteki sürüm"
        );
        let settings = fs::read_to_string(e.claude_dir.join("settings.json")).unwrap();
        assert!(
            settings.contains("rtk hook claude") && settings.contains("\"model\""),
            "{settings}"
        );
        assert!(
            fs::read_dir(e.app_data.join("backups")).unwrap().count() >= 2,
            "önce yedek"
        );

        e.disable_rtk().expect("rtk kaldırılamadı");
        assert!(!e.rtk_status().enabled);
        let settings = fs::read_to_string(e.claude_dir.join("settings.json")).unwrap();
        assert!(!settings.contains("rtk hook claude"), "{settings}");
        assert!(
            fs::read_to_string(e.claude_dir.join("CLAUDE.md"))
                .unwrap()
                .contains("# my rules")
        );
    }
}
