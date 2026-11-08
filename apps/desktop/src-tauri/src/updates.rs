//! Otomatik güncelleme (OZELLIKLER.md §8): `tauri-plugin-updater` (sürüm sabit), GitHub Releases'teki
//! `latest.json`. Paket Ed25519 ile imzalı; açık anahtar `tauri.conf.json`'da, özel anahtar geliştiricinin
//! bilgisayarında (`~/.tauri/kaans-terminal.key`, şifresi Anahtar Zinciri'nde; bkz. `scripts/release.mjs`).
//!
//! Akış: açılıştan 20 sn sonra, sonra 15 dk'da bir ve uykudan dönünce kontrol → yeni sürüm varsa arka planda iner
//! (`update://state`, yüzde) → hazır. Kurulum: kullanıcı "restart to update" der ya da sistem 10 dk boştaysa
//! kendiliğinden. Terminaller oturum sunucusunda yaşadığı için yeniden başlatma sohbetleri kesmez. Kurmadan önce
//! sürüm notları `<app_data>/whats-new.json`'a yazılır; yeni sürüm açılınca bir kez gösterilir.
//! Debug derlemede kapalıdır (`KAANS_UPDATE_TEST=1` ile açılır).

use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const FIRST_CHECK: Duration = Duration::from_secs(20);
const EVERY: Duration = Duration::from_secs(15 * 60);
const AUTO_INSTALL_IDLE_SECS: f64 = 600.0;

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    /// disabled | idle | checking | downloading | ready | installing | error
    pub status: String,
    pub current: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub pct: Option<u8>,
    pub error: Option<String>,
    /// Son kontrol (ms)
    pub checked: Option<i64>,
}

#[derive(Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WhatsNew {
    pub version: String,
    pub notes: String,
}

#[derive(Default)]
struct Inner {
    view: UpdateView,
    ready: Option<(Update, Vec<u8>)>,
    busy: bool,
}

#[derive(Clone, Default)]
pub struct Updates(Arc<Mutex<Inner>>);

impl Updates {
    pub fn view(&self) -> UpdateView {
        self.0.lock().map(|i| i.view.clone()).unwrap_or_default()
    }

    fn set(&self, app: &AppHandle, f: impl FnOnce(&mut UpdateView)) {
        let view = {
            let Ok(mut i) = self.0.lock() else { return };
            f(&mut i.view);
            i.view.clone()
        };
        let _ = app.emit("update://state", view);
    }
}

fn enabled() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("KAANS_UPDATE_TEST").is_some()
}

fn whats_new_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("whats-new.json"))
}

/// Kontrol + indirme. Zaten indiriliyorsa ya da hazırsa bir şey yapmaz.
pub async fn check(app: AppHandle, updates: Updates) {
    {
        let Ok(mut i) = updates.0.lock() else { return };
        if i.busy || i.ready.is_some() || !enabled() {
            return;
        }
        i.busy = true;
    }
    updates.set(&app, |v| {
        v.status = "checking".into();
        v.error = None;
    });
    let result = async {
        let updater = app.updater().map_err(|e| e.to_string())?;
        let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
            return Ok::<_, String>(None);
        };
        let version = update.version.clone();
        let notes = update.body.clone().unwrap_or_default();
        updates.set(&app, |v| {
            v.status = "downloading".into();
            v.version = Some(version.clone());
            v.notes = Some(notes.clone());
            v.pct = Some(0);
        });
        let mut got: u64 = 0;
        let mut last_pct = 0u8;
        let progress_app = app.clone();
        let progress_updates = updates.clone();
        let bytes = update
            .download(
                move |chunk, total| {
                    got += chunk as u64;
                    if let Some(total) = total.filter(|t| *t > 0) {
                        let pct = ((got * 100) / total).min(100) as u8;
                        if pct != last_pct {
                            last_pct = pct;
                            progress_updates.set(&progress_app, |v| v.pct = Some(pct));
                        }
                    }
                },
                || {},
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(Some((update, bytes)))
    }
    .await;
    let now = chrono::Utc::now().timestamp_millis();
    match result {
        Ok(Some(ready)) => {
            if let Ok(mut i) = updates.0.lock() {
                i.ready = Some(ready);
                i.busy = false;
            }
            updates.set(&app, |v| {
                v.status = "ready".into();
                v.pct = Some(100);
                v.checked = Some(now);
            });
        }
        Ok(None) => {
            if let Ok(mut i) = updates.0.lock() {
                i.busy = false;
            }
            updates.set(&app, |v| {
                v.status = "idle".into();
                v.version = None;
                v.notes = None;
                v.pct = None;
                v.checked = Some(now);
            });
        }
        Err(e) => {
            if let Ok(mut i) = updates.0.lock() {
                i.busy = false;
            }
            crate::watchdog::log("update", &e);
            updates.set(&app, |v| {
                v.status = "error".into();
                v.error = Some(
                    crate::engine::redact::redact(&e)
                        .chars()
                        .take(300)
                        .collect(),
                );
                v.pct = None;
                v.checked = Some(now);
            });
        }
    }
}

/// İndirilmiş güncellemeyi kurar ve yeniden başlatır.
pub fn install(app: &AppHandle, updates: &Updates) -> Result<(), String> {
    let (update, bytes) = {
        let mut i = updates.0.lock().map_err(|_| "busy")?;
        i.ready.take().ok_or("no update is ready")?
    };
    if let Some(path) = whats_new_path(app) {
        let wn = WhatsNew {
            version: update.version.clone(),
            notes: update.body.clone().unwrap_or_default(),
        };
        if let Ok(text) = serde_json::to_string(&wn) {
            let _ = std::fs::write(path, text);
        }
    }
    updates.set(app, |v| v.status = "installing".into());
    crate::watchdog::log("update", &format!("installing {}", update.version));
    if let Err(e) = update.install(bytes) {
        updates.set(app, |v| {
            v.status = "error".into();
            v.error = Some(e.to_string());
        });
        return Err(e.to_string());
    }
    app.restart();
}

/// Yeni sürümün ilk açılışında bir kez: sürüm notları (dosya okununca silinir).
pub fn take_whats_new(app: &AppHandle) -> Option<WhatsNew> {
    let path = whats_new_path(app)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let wn: WhatsNew = serde_json::from_str(&text).ok()?;
    (wn.version == app.package_info().version.to_string()).then_some(wn)
}

/// Arka plan: kontroller ve boştayken kurulum.
pub fn run(app: AppHandle, updates: Updates) {
    let current = app.package_info().version.to_string();
    updates.set(&app, |v| {
        v.current = current;
        v.status = if enabled() { "idle" } else { "disabled" }.into();
    });
    if !enabled() {
        return;
    }
    std::thread::sleep(FIRST_CHECK);
    let mut last_check: Option<Instant> = None;
    let mut last_wall = chrono::Utc::now();
    loop {
        // Uykudan dönüş: duvar saati monoton saatten çok ilerlediyse hemen kontrol
        let wall = chrono::Utc::now();
        let woke = (wall - last_wall).num_seconds() > 120;
        last_wall = wall;
        if woke || last_check.is_none_or(|t| t.elapsed() >= EVERY) {
            last_check = Some(Instant::now());
            tauri::async_runtime::block_on(check(app.clone(), updates.clone()));
        }
        let ready = updates.0.lock().is_ok_and(|i| i.ready.is_some());
        if ready && crate::activity::system_idle_secs().is_some_and(|s| s >= AUTO_INSTALL_IDLE_SECS)
        {
            crate::watchdog::log("update", "user away 10 min: installing");
            let _ = install(&app, &updates);
        }
        std::thread::sleep(Duration::from_secs(30));
    }
}
