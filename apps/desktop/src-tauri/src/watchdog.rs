//! Yerel hata günlüğü ve donma bekçisi (OZELLIKLER.md §10). Hiçbir şey cihazdan çıkmaz.
//!
//! - `errors.log` (`<app_data>`, ~512 KB'ta `errors.old.log`'a döner): sayfa hataları (`report_error`), Rust
//!   panikleri (`install_panic_hook`, backtrace'li) ve donmalar. Her metin `redact()`'ten geçer.
//! - Nabızlar: iş parçacıkları/pencereler `beat(ad)` çağırır; bekçi 2 sn'de bir bakar, eşik aşılınca bir kez yazar,
//!   düzelince "recovered after Ns" yazar.
//!   - `engine`: engine turu (eşik 30 sn; ilk açılışta geçmiş okunurken uzun sürebilir)
//!   - `main`: Rust ana (UI) iş parçacığı; bekçi `run_on_main_thread` ile yoklar (eşik 8 sn). Mac'te donma anında
//!     `/usr/bin/sample` ile 3 sn'lik yığın örneği `<app_log>/hang-<zaman>.txt`'ye alınır (olay başına bir kez).
//!   - `ui:<pencere>`: sayfanın JS iş parçacığı (`ui_beat` komutu, 2 sn'de bir; eşik 10 sn). Yalnızca pencere
//!     görünür ve öndeyken sayılır (arka plandaki pencerede zamanlayıcılar kısılır). Release derlemede ana pencerenin
//!     sayfası 30 sn donarsa yeniden yüklenir (terminaller oturum sunucusunda yaşar, kaybolmaz).

use crate::engine::redact::redact;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const LOG_MAX: u64 = 512 * 1024;
const TICK: Duration = Duration::from_secs(2);
const ENGINE_LIMIT: Duration = Duration::from_secs(30);
const MAIN_LIMIT: Duration = Duration::from_secs(8);
const UI_LIMIT: Duration = Duration::from_secs(10);
const UI_RELOAD: Duration = Duration::from_secs(30);

static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

struct Pulse {
    last: Instant,
    /// Sayılır mı (ör. pencere arkadaysa hayır)
    watched: bool,
    /// Bu donma zaten yazıldı mı (başlangıç anı)
    stalled_since: Option<Instant>,
    reloaded: bool,
}

static PULSES: LazyLock<Mutex<HashMap<String, Pulse>>> = LazyLock::new(Default::default);

/// Günlüğe tek kayıt. `dir` verilmezse kurulumdaki klasör.
pub fn log(kind: &str, text: &str) {
    if let Some(dir) = LOG_DIR.get() {
        log_to(dir, kind, text);
    }
}

pub fn log_to(dir: &Path, kind: &str, text: &str) {
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("errors.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > LOG_MAX) {
        let _ = std::fs::rename(&path, dir.join("errors.old.log"));
    }
    let body: String = text.chars().take(12_000).collect();
    let line = format!(
        "{} [{}] {}\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        kind,
        redact(&body)
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Paniği (mesaj, konum, iş parçacığı, backtrace) günlüğe yazar; varsayılan davranış (stderr) korunur.
pub fn install_panic_hook(dir: PathBuf) {
    let _ = LOG_DIR.set(dir);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        let at = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let bt = std::backtrace::Backtrace::force_capture();
        log(
            "panic",
            &format!(
                "thread '{}' panicked at {at}: {msg}\n{bt}",
                thread.name().unwrap_or("?")
            ),
        );
        previous(info);
    }));
}

/// Nabız: `watched` false ise (pencere arkada) o ad sayılmaz.
pub fn beat(name: &str, watched: bool) {
    let Ok(mut map) = PULSES.lock() else { return };
    let now = Instant::now();
    let p = map.entry(name.to_string()).or_insert(Pulse {
        last: now,
        watched,
        stalled_since: None,
        reloaded: false,
    });
    if let Some(since) = p.stalled_since.take() {
        let secs = now.duration_since(since).as_secs() + limit_of(name).as_secs();
        drop(map);
        log("hang", &format!("{name} recovered after ~{secs}s"));
        let Ok(mut map) = PULSES.lock() else { return };
        if let Some(p) = map.get_mut(name) {
            p.last = now;
            p.watched = watched;
            p.reloaded = false;
        }
        return;
    }
    p.last = now;
    p.watched = watched;
    p.reloaded = false;
}

/// Pencere kapanınca nabzı unut.
pub fn forget(name: &str) {
    if let Ok(mut map) = PULSES.lock() {
        map.remove(name);
    }
}

fn limit_of(name: &str) -> Duration {
    match name {
        "engine" => ENGINE_LIMIT,
        "main" => MAIN_LIMIT,
        _ => UI_LIMIT,
    }
}

/// Eşiği yeni aşan nabızlar (bir kez) ve yeniden yüklenecek pencereler.
fn check(now: Instant) -> (Vec<(String, Duration)>, Vec<String>) {
    let mut stalled = Vec::new();
    let mut reload = Vec::new();
    let Ok(mut map) = PULSES.lock() else {
        return (stalled, reload);
    };
    for (name, p) in map.iter_mut() {
        if !p.watched {
            continue;
        }
        let quiet = now.saturating_duration_since(p.last);
        if quiet >= limit_of(name) && p.stalled_since.is_none() {
            p.stalled_since = Some(now);
            stalled.push((name.clone(), quiet));
        }
        if name == "ui:main" && quiet >= UI_RELOAD && !p.reloaded {
            p.reloaded = true;
            reload.push(name.clone());
        }
    }
    (stalled, reload)
}

/// Bekçi iş parçacığı.
pub fn run(app: AppHandle) {
    let log_dir = app.path().app_log_dir().ok();
    beat("main", true);
    loop {
        std::thread::sleep(TICK);
        let _ = app.run_on_main_thread(|| beat("main", true));
        let (stalled, reload) = check(Instant::now());
        for (name, quiet) in stalled {
            log(
                "hang",
                &format!("{name} not responding for {}s", quiet.as_secs()),
            );
            if name == "main"
                && let Some(dir) = &log_dir
            {
                sample_stacks(dir);
            }
        }
        for name in reload {
            if cfg!(debug_assertions) {
                log(
                    "hang",
                    &format!("{name} frozen 30s (debug build: not reloading)"),
                );
                continue;
            }
            let label = name.trim_start_matches("ui:");
            if let Some(w) = app.get_webview_window(label) {
                log("hang", &format!("{name} frozen 30s: reloading the page"));
                let _ = w.reload();
            }
        }
    }
}

/// Mac: bütün iş parçacıklarının yığın örneği (yalnızca donma anında, olay başına bir kez).
fn sample_stacks(dir: &Path) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::fs::create_dir_all(dir);
        let file = dir.join(format!(
            "hang-{}.txt",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ));
        let pid = std::process::id().to_string();
        let out = file.clone();
        std::thread::spawn(move || {
            let ok = std::process::Command::new("/usr/bin/sample")
                .args([pid.as_str(), "3", "-file"])
                .arg(&out)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if ok {
                log("hang", &format!("stack sample: {}", out.display()));
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = dir;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stalls_once_and_only_when_watched() {
        let t0 = Instant::now();
        beat("ui:test-a", true);
        beat("ui:test-b", false);
        let (s, _) = check(t0 + Duration::from_secs(11));
        let names: Vec<_> = s.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"ui:test-a"));
        assert!(!names.contains(&"ui:test-b"), "arkadaki pencere sayılmaz");
        let (s, _) = check(t0 + Duration::from_secs(13));
        assert!(
            !s.iter().any(|(n, _)| n == "ui:test-a"),
            "aynı donma bir kez yazılır"
        );
        forget("ui:test-a");
        forget("ui:test-b");
    }

    #[test]
    fn writes_redacted_log() {
        let dir = tempfile::tempdir().unwrap();
        log_to(
            dir.path(),
            "test",
            "token sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        );
        let text = std::fs::read_to_string(dir.path().join("errors.log")).unwrap();
        assert!(text.contains("[test]"));
        assert!(!text.contains("AAAAAAAAAAAAAAAAAAAAAAAA"), "{text}");
    }
}
