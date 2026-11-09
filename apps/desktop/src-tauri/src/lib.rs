pub mod activity;
pub mod ai;
pub mod e2e;
pub mod engine;
pub mod gesture;
pub mod git;
pub mod host;
pub mod integrations;
pub mod live;
pub mod power;
pub mod procs;
pub mod savers;
pub mod social;
pub mod term;
pub mod updates;
pub mod watchdog;

use engine::ledger::{Ledger, PastTotals};
use engine::pricing::PriceTable;
use engine::runaway::Detector;
use engine::{Engine, EngineEvent, Snapshot, paths, proc};
use power::PowerState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

/// Uygulamanın görünen adı. Logo, pencere başlığı ve AppSupport klasörü bundan türetilir.
pub const APP_NAME: &str = "Kaan's Terminal";

/// Engine turu sıklığı.
const TICK: Duration = Duration::from_secs(1);
/// Aktif dakika örneği (heartbeat aralığı)
const ACTIVITY_POLL: Duration = Duration::from_secs(60);
const EV_ACTIVITY: &str = "xp://activity";
/// Güç durumu (pil / düşük güç) sorgu sıklığı.
const POWER_POLL: Duration = Duration::from_secs(15);
/// Ledger: dün dondurulmuş mu, gün değişti mi kontrolü.
const LEDGER_POLL: Duration = Duration::from_secs(60);
/// Kaçak süreç taraması sıklığı (karar 2 dakikalık ortalamaya bakar).
const RUNAWAY_POLL: Duration = Duration::from_secs(10);
/// Süreç tablosundan yapay zekâ araçlarını bulma sıklığı.
const AGENTS_POLL: Duration = Duration::from_secs(3);

/// Rust → sayfa olay adları (web/src/lib/engine.svelte.ts ve power.svelte.ts ile aynı).
pub const EV_STATE: &str = "engine://state";
pub const EV_EVENTS: &str = "engine://events";
pub const EV_POWER: &str = "power://state";

#[derive(Default)]
struct Shared {
    snapshot: Option<Snapshot>,
    events: Vec<EngineEvent>,
    power: PowerState,
    /// kart → son cevabın tamamı (⇧⌘C)
    replies: std::collections::HashMap<String, String>,
    /// Bugün sohbetlerin yazdığı dosyalar (✦)
    edited: Vec<String>,
    /// Günlük aktif süre (XP)
    activity: Vec<activity::DayActivity>,
    /// Sayfanın seçili teması (heartbeat ile sunucuya; sıralamada "in their theme")
    theme: Option<String>,
}

type SharedState = Arc<Mutex<Shared>>;

fn lock(s: &SharedState) -> MutexGuard<'_, Shared> {
    s.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Serialize)]
struct Initial {
    snapshot: Option<Snapshot>,
    events: Vec<EngineEvent>,
    power: PowerState,
}

/// Sayfa açılınca (ya da yeniden yüklenince) son durumu, son olayları ve güç durumunu alır.
#[tauri::command]
fn engine_initial(state: State<'_, SharedState>) -> Initial {
    let s = lock(&state);
    Initial {
        snapshot: s.snapshot.clone(),
        events: s.events.clone(),
        power: s.power,
    }
}

fn app_data(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok()
}

fn load_prices(app: &AppHandle) -> PriceTable {
    // Kullanıcı (ya da ileride güncelleme) uygulama veri klasörüne yeni bir tablo koyarsa o kullanılır.
    app_data(app)
        .map(|d| PriceTable::load_or_bundled(&d.join("pricing.json")))
        .unwrap_or_else(PriceTable::bundled)
}

/// Bir terminaldeki sohbetin son cevabının tamamı (ekrana sığmayan kısmı dahil).
#[tauri::command]
fn chat_last_reply(state: State<'_, SharedState>, terminal: String) -> Option<String> {
    let s = lock(&state);
    let card = s
        .snapshot
        .as_ref()?
        .sessions
        .iter()
        .find(|c| c.terminal.as_deref() == Some(&terminal))?
        .id
        .clone();
    s.replies.get(&card).cloned()
}

fn integration_paths(app: &AppHandle) -> Result<integrations::Paths, String> {
    Ok(integrations::Paths {
        claude_settings: paths::claude_dir().ok_or("home yok")?.join("settings.json"),
        app_data: app.path().app_data_dir().map_err(|e| e.to_string())?,
        exe: std::env::current_exe().map_err(|e| e.to_string())?,
    })
}

fn savers_env(app: &AppHandle) -> Result<savers::Env, String> {
    let home = std::env::home_dir().ok_or("home yok")?;
    Ok(savers::Env {
        claude_dir: paths::claude_dir().ok_or("home yok")?,
        codex_dir: engine::codex::codex_dir().unwrap_or_else(|| home.join(".codex")),
        app_data: app.path().app_data_dir().map_err(|e| e.to_string())?,
        bundled_rtk: savers::bundled_rtk_rel()
            .and_then(|rel| app.path().resource_dir().ok().map(|d| d.join(rel)))
            .filter(|p| p.exists()),
        home,
        home_override: false,
    })
}

/// Entegrasyonların durumu (Entegrasyonlar ekranı).
#[tauri::command]
fn integrations_status(app: AppHandle) -> Result<Vec<integrations::Status>, String> {
    let p = integration_paths(&app)?;
    Ok(vec![
        integrations::statusline_status(&p),
        integrations::theme_status(&p),
    ])
}

/// Token savers (rtk, caveman) durumu. Kullanıcının PATH'ini sorduğu için ana iş parçacığını tutmasın.
/// Git komutları bekleyebilir (ağ, büyük repo): arayüz iş parçacığını tutmasınlar.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn git_repos(cwds: Vec<String>) -> Result<Vec<git::RepoRef>, String> {
    blocking(move || Ok(git::repos_of(&cwds))).await
}

#[tauri::command]
async fn git_status(root: String) -> Result<git::RepoStatus, String> {
    blocking(move || git::status(&root)).await
}

/// Source Control ✨: kullanıcının kendi Claude Code'u (`claude -p`, aboneliği) ile commit mesajı.
#[tauri::command]
async fn git_ai_message(root: String) -> Result<String, String> {
    blocking(move || ai::commit_message(&root)).await
}

#[tauri::command]
async fn git_log(root: String, limit: usize) -> Result<Vec<git::Commit>, String> {
    blocking(move || git::log(&root, limit)).await
}

/// stage / unstage / discard / commit / push / sync
#[tauri::command]
async fn git_run(
    root: String,
    op: String,
    paths: Vec<String>,
    message: String,
) -> Result<(), String> {
    blocking(move || match op.as_str() {
        "stage" => git::stage(&root, &paths),
        "unstage" => git::unstage(&root, &paths),
        "discard" => git::discard(&root, &paths),
        "commit" => git::commit(&root, &message),
        "push" => git::push(&root),
        "sync" => git::sync(&root),
        _ => Err(format!("unknown git op: {op}")),
    })
    .await
}

#[tauri::command]
async fn git_branches(root: String) -> Result<Vec<git::Branch>, String> {
    blocking(move || git::branches(&root)).await
}

/// Dala geç ya da dal oluştur (`create`, `from`); `stash`: önce değişiklikleri stash'e koy.
#[tauri::command]
async fn git_checkout(
    root: String,
    name: String,
    create: bool,
    from: Option<String>,
    stash: bool,
) -> Result<(), String> {
    blocking(move || git::checkout(&root, &name, create, from.as_deref(), stash)).await
}

#[tauri::command]
async fn git_diff(
    root: String,
    path: String,
    orig: Option<String>,
    staged: bool,
) -> Result<git::DiffView, String> {
    blocking(move || git::diff(&root, &path, orig.as_deref(), staged)).await
}

#[tauri::command]
async fn git_file(root: String, path: String) -> Result<git::FileText, String> {
    blocking(move || git::file(&root, &path)).await
}

#[tauri::command]
async fn git_blame(
    root: String,
    path: String,
    line: u32,
) -> Result<Option<git::BlameLine>, String> {
    blocking(move || git::blame(&root, &path, line)).await
}

/// Günlük aktif süreler (XP, level ve rütbe sayfada `@kaans/shared/xp` ile hesaplanır).
#[tauri::command]
fn activity_days(shared: State<'_, SharedState>) -> Vec<activity::DayActivity> {
    lock(&shared).activity.clone()
}

type SocialHandle = Arc<social::Social>;

#[tauri::command]
fn social_state(social: State<'_, SocialHandle>) -> social::SocialState {
    social.state()
}

#[tauri::command]
async fn social_register(
    social: State<'_, SocialHandle>,
    name: String,
) -> Result<social::SocialState, String> {
    let s = social.inner().clone();
    blocking(move || s.register(&name).map_err(|e| e.to_string())).await
}

#[tauri::command]
async fn social_rename(
    social: State<'_, SocialHandle>,
    name: String,
) -> Result<social::SocialState, String> {
    let s = social.inner().clone();
    blocking(move || s.rename(&name).map_err(|e| e.to_string())).await
}

/// URL sorgu değeri kodlama (name#tag'deki # gibi)
fn query_escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// İzin listesindeki okuma uçları (MIMARI.md §5: sayfa yalnızca bunları isteyebilir)
#[tauri::command]
async fn social_get(
    social: State<'_, SocialHandle>,
    what: String,
    with: Option<String>,
    before: Option<String>,
) -> Result<serde_json::Value, String> {
    let path = match what.as_str() {
        "status" => "me/status".to_string(),
        "leaderboard.week" => "leaderboard?scope=week".to_string(),
        "leaderboard.all" => "leaderboard?scope=all".to_string(),
        "friends" => "friends".to_string(),
        "notifications" => "notifications".to_string(),
        "messages" => {
            let w = with.ok_or("missing with")?;
            let mut p = format!("messages?with={}", query_escape(&w));
            if let Some(b) =
                before.filter(|b| !b.is_empty() && b.chars().all(|c| c.is_ascii_digit()))
            {
                p.push_str(&format!("&before={b}"));
            }
            p
        }
        _ => return Err(format!("not allowed: {what}")),
    };
    let s = social.inner().clone();
    blocking(move || s.get(&path).map_err(|e| e.to_string())).await
}

/// İzin listesindeki yazma uçları: friends.add/accept/remove, messages.send, presence
#[tauri::command]
async fn social_post(
    social: State<'_, SocialHandle>,
    what: String,
    body: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let s = social.inner().clone();
    blocking(move || s.post(&what, body).map_err(|e| e.to_string())).await
}

#[tauri::command]
async fn social_privacy(
    social: State<'_, SocialHandle>,
    hide_time: Option<bool>,
    hide_name: Option<bool>,
) -> Result<serde_json::Value, String> {
    let s = social.inner().clone();
    blocking(move || s.privacy(hide_time, hide_name).map_err(|e| e.to_string())).await
}

/// Bir arkadaşla sohbet kendi penceresinde (OZELLIKLER.md §6.2). Açıksa öne getirilir. Pencere farenin
/// bulunduğu ekranın ortasında açılır (her zaman kullanılan ekranda ve görünür alanda).
#[tauri::command]
fn chat_window(app: AppHandle, name_tag: String) -> Result<String, String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    let ok = name_tag.split_once('#').is_some_and(|(n, t)| {
        (3..=20).contains(&n.len())
            && n.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            && t.len() == 4
            && t.bytes().all(|b| b.is_ascii_digit())
    });
    if !ok {
        return Err("bad name#tag".into());
    }
    // Etiketlerde yalnızca belirli karakterler geçer: name#tag onaltılık
    let label = format!(
        "chat-{}",
        name_tag
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(label);
    }
    let (w, h) = (400.0, 580.0);
    let url = format!("index.html#chat={}", query_escape(&name_tag));
    let mut builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title(name_tag.clone())
        .inner_size(w, h)
        .min_inner_size(300.0, 360.0)
        .transparent(true);
    #[cfg(target_os = "macos")]
    {
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
    }
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten());
    builder = match monitor {
        Some(m) => {
            let s = m.scale_factor();
            let (px, py) = (f64::from(m.position().x) / s, f64::from(m.position().y) / s);
            let (mw, mh) = (
                f64::from(m.size().width) / s,
                f64::from(m.size().height) / s,
            );
            builder.position(px + (mw - w) / 2.0, py + (mh - h) / 2.0)
        }
        None => builder.center(),
    };
    builder.build().map_err(|e| e.to_string())?;
    Ok(label)
}

/// Sayfa tema değişince bildirir (sunucuya bir sonraki heartbeat'le gider)
#[tauri::command]
fn social_set_theme(shared: State<'_, SharedState>, theme: String) {
    lock(&shared).theme = Some(theme.chars().take(40).collect());
}

/// Sayfadaki yakalanmayan hatalar yerel günlüğe (`<app_data>/errors.log`, en fazla ~512 KB, maskeli). Hiçbir yere
/// gönderilmez (MIMARI.md §9); hata ayıklamak ve ileride "hata raporu" ekranı için.
#[tauri::command]
fn report_error(
    app: AppHandle,
    window: tauri::WebviewWindow,
    message: String,
    stack: Option<String>,
) {
    let Some(dir) = app_data(&app) else { return };
    let message: String = message.chars().take(2000).collect();
    let stack: String = stack.unwrap_or_default().chars().take(6000).collect();
    watchdog::log_to(&dir, window.label(), &format!("{message}\n{stack}"));
}

#[tauri::command]
fn update_state(updates: State<'_, updates::Updates>) -> updates::UpdateView {
    updates.view()
}

#[tauri::command]
async fn update_check(app: AppHandle, updates: State<'_, updates::Updates>) -> Result<(), String> {
    updates::check(app, updates.inner().clone()).await;
    Ok(())
}

#[tauri::command]
fn update_install(app: AppHandle, updates: State<'_, updates::Updates>) -> Result<(), String> {
    updates::install(&app, &updates)
}

#[tauri::command]
fn update_whats_new(app: AppHandle) -> Option<updates::WhatsNew> {
    updates::take_whats_new(&app)
}

/// ABOUT: paketteki üçüncü taraf lisans dosyasını varsayılan uygulamayla açar.
#[tauri::command]
fn open_licenses(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = app
        .path()
        .resolve(
            "resources/licenses/THIRD-PARTY.txt",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| e.to_string())?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// ABOUT: yerel hata günlüğünü Finder/Explorer'da gösterir (yoksa klasörü).
#[tauri::command]
fn reveal_error_log(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = app_data(&app).ok_or("no data folder")?;
    let log = dir.join("errors.log");
    if log.exists() {
        app.opener()
            .reveal_item_in_dir(log)
            .map_err(|e| e.to_string())
    } else {
        app.opener()
            .open_path(dir.to_string_lossy(), None::<&str>)
            .map_err(|e| e.to_string())
    }
}

/// Donma bekçisi: sayfa 2 sn'de bir çağırır. Pencere görünür ve öndeyse izlenir (`watchdog.rs`).
#[tauri::command]
fn ui_beat(window: tauri::WebviewWindow, visible: bool) {
    let watched = visible && window.is_focused().unwrap_or(false);
    watchdog::beat(&format!("ui:{}", window.label()), watched);
}

#[tauri::command]
fn git_edited(shared: State<'_, SharedState>) -> Vec<String> {
    lock(&shared).edited.clone()
}

#[tauri::command]
async fn savers_status(app: AppHandle) -> Result<Vec<savers::SaverStatus>, String> {
    let env = savers_env(&app)?;
    tauri::async_runtime::spawn_blocking(move || vec![env.rtk_status(), env.caveman_status()])
        .await
        .map_err(|e| e.to_string())
}

/// Kullanıcı bir entegrasyonu açtı/kapattı. Yalnızca bu komutla harici ayarlara yazılır.
/// rtk/caveman dış komut çalıştırır (ağ dahil); ana iş parçacığını tutmasın.
#[tauri::command]
async fn integration_set(app: AppHandle, id: String, enabled: bool) -> Result<(), String> {
    let p = integration_paths(&app)?;
    let env = savers_env(&app)?;
    tauri::async_runtime::spawn_blocking(move || match (id.as_str(), enabled) {
        ("claude-statusline", true) => integrations::enable_statusline(&p),
        ("claude-statusline", false) => integrations::disable_statusline(&p),
        ("claude-theme", true) => integrations::enable_theme(&p),
        ("claude-theme", false) => integrations::disable_theme(&p),
        ("rtk", true) => env.enable_rtk(),
        ("rtk", false) => env.disable_rtk(),
        ("caveman", true) => env.enable_caveman(),
        ("caveman", false) => env.disable_caveman(),
        _ => Err(format!("bilinmeyen entegrasyon: {id}")),
    })
    .await
    .map_err(|e| e.to_string())?
}

/// "Bütün entegrasyonları kaldır": her şey önceki haline döner.
#[tauri::command]
async fn integrations_remove_all(app: AppHandle) -> Result<(), String> {
    let p = integration_paths(&app)?;
    let env = savers_env(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut errors = Vec::new();
        if let Err(e) = integrations::disable_all(&p) {
            errors.push(e);
        }
        if env.rtk_status().enabled
            && let Err(e) = env.disable_rtk()
        {
            errors.push(format!("rtk: {e}"));
        }
        if env.caveman_status().enabled
            && let Err(e) = env.disable_caveman()
        {
            errors.push(format!("caveman: {e}"));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// DANGER panelinden "bitir": yalnızca şu an kaçak diye işaretli bir süreç öldürülebilir.
#[tauri::command]
/// Kaçak süreci ya da terminali olmayan bir uygulama sürecini (dev sunucusu) alt süreçleriyle birlikte sonlandırır.
/// Yalnızca o an uygulamanın kendi listelerindeki pid'ler kabul edilir (rastgele pid öldürülemez).
fn danger_kill(state: State<'_, SharedState>, pid: u32) -> Result<(), String> {
    let allowed = lock(&state).snapshot.as_ref().is_some_and(|s| {
        s.runaways.iter().any(|r| r.pid == pid)
            || s.apps.iter().any(|a| a.pid == pid && a.terminal.is_none())
    });
    if !allowed {
        return Err("bu süreç kaçak ya da terminalsiz uygulama olarak işaretli değil".into());
    }
    #[cfg(unix)]
    {
        let mut pids = vec![pid];
        pids.extend(procs::descendants(pid, &procs::table()));
        let list: Vec<libc::pid_t> = pids
            .iter()
            .filter_map(|p| libc::pid_t::try_from(*p).ok())
            .collect();
        for p in &list {
            // SAFETY: yalnızca sinyal gönderir; kök pid az önce listede doğrulandı, diğerleri onun alt süreçleri.
            unsafe { libc::kill(*p, libc::SIGTERM) };
        }
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            for p in list {
                // SAFETY: aynı süreçler; hâlâ yaşıyorsa zorla.
                unsafe { libc::kill(p, libc::SIGKILL) };
            }
        });
    }
    Ok(())
}

fn run_engine(
    app: AppHandle,
    shared: SharedState,
    past_rx: Receiver<PastTotals>,
    beats: Sender<bool>,
) {
    let Some(root) = paths::claude_dir() else {
        eprintln!("[engine] home klasörü bulunamadı; engine çalışmıyor");
        return;
    };
    let mut engine = Engine::new(root, chrono::Local::now().date_naive())
        .with_prices(load_prices(&app))
        .with_codex(engine::codex::codex_dir());
    let mut last_power: Option<(Instant, PowerState)> = None;
    let mut detector = Detector::default();
    let mut agent_detector = engine::agents::Detector::default();
    let mut dev_apps = engine::apps::Detector::default();
    let mut last_runaway: Option<Instant> = None;
    let mut last_agents: Option<Instant> = None;
    let mut last_limits: Option<Instant> = None;
    let app_data = app.path().app_data_dir().ok();
    let mut tracker = app_data.as_deref().map(activity::Tracker::load);
    if let Some(t) = &tracker {
        lock(&shared).activity = t.days();
    }
    let mut last_activity: Option<Instant> = None;
    let uid = procs::current_uid();
    loop {
        watchdog::beat("engine", true);
        while let Ok(past) = past_rx.try_recv() {
            engine.set_past(past);
        }
        engine.set_host(app.state::<term::Terminals>().state());
        // Claude limitleri: statusLine entegrasyonu açıksa Claude Code her güncellemede dosyayı yazar.
        if last_limits.is_none_or(|t| t.elapsed() >= Duration::from_secs(5)) {
            last_limits = Some(Instant::now());
            engine.set_claude_limits(
                app_data
                    .as_deref()
                    .and_then(integrations::read_claude_limits),
            );
        }
        let need_agents = last_agents.is_none_or(|t| t.elapsed() >= AGENTS_POLL);
        let need_runaway = last_runaway.is_none_or(|t| t.elapsed() >= RUNAWAY_POLL);
        let table = if need_agents || need_runaway {
            procs::table()
        } else {
            Vec::new()
        };
        if need_agents {
            last_agents = Some(Instant::now());
            engine.set_agents(agent_detector.scan(
                chrono::Utc::now().timestamp_millis(),
                &table,
                uid,
                &procs::args,
                &procs::cwd,
            ));
            // Çalışan uygulama komutları (dev sunucuları): aynı tablo, aynı aralık
            engine.set_apps(dev_apps.scan(
                chrono::Utc::now().timestamp_millis(),
                &table,
                uid,
                &procs::args,
                &procs::cwd,
            ));
        }
        if need_runaway {
            last_runaway = Some(Instant::now());
            let list = detector.scan(
                chrono::Utc::now().timestamp_millis(),
                &table,
                uid,
                &procs::path,
                &procs::cwd,
                &engine.chat_cwds(),
            );
            engine.set_runaways(list);
        }
        let (snapshot, events) = engine.tick(chrono::Local::now(), &proc::is_alive_since);

        let power = match last_power {
            Some((at, p)) if at.elapsed() < POWER_POLL => p,
            _ => {
                let p = power::read();
                if last_power.is_none_or(|(_, old)| old != p) {
                    let _ = app.emit(EV_POWER, p);
                }
                last_power = Some((Instant::now(), p));
                p
            }
        };
        {
            let mut s = lock(&shared);
            s.snapshot = Some(snapshot.clone());
            s.power = power;
            s.replies.extend(engine.take_reply_updates());
            s.edited = engine.edited_paths();
            if !events.is_empty() || s.events.is_empty() {
                s.events = engine.recent_events();
            }
        }
        let _ = app.emit(EV_STATE, &snapshot);
        if !events.is_empty() {
            let _ = app.emit(EV_EVENTS, &events);
        }
        // Aktif dakika: AFK değil + çalışma sinyali (OZELLIKLER.md §6.3)
        if let Some(t) = tracker.as_mut()
            && last_activity.is_none_or(|at| at.elapsed() >= ACTIVITY_POLL)
        {
            last_activity = Some(Instant::now());
            let now = chrono::Local::now();
            let present = activity::system_idle_secs().is_some_and(|s| s < activity::WINDOW_SECS);
            let active = present && activity::working(&snapshot, now.timestamp_millis());
            // Sunucuya dakikalık atış (kayıtlıysa; ayrı iş parçacığında, ağ beklenmez)
            let _ = beats.send(active);
            if t.sample(
                now.timestamp_millis(),
                &now.format("%Y-%m-%d").to_string(),
                active,
            ) {
                let days = t.days();
                let _ = app.emit(EV_ACTIVITY, &days);
                lock(&shared).activity = days;
            }
        }
        std::thread::sleep(TICK);
    }
}

/// Dakikalık heartbeat'leri sunucuya yollar; cevap (sunucunun XP özeti) `social://status` olarak sayfaya gider.
/// Sunucuya ulaşılamazsa atış atlanır (yerel sayaç zaten tutuluyor).
fn run_social(
    app: AppHandle,
    social: Arc<social::Social>,
    shared: SharedState,
    beats: Receiver<bool>,
) {
    // Uçtan uca şifreleme: açık anahtar sunucuda olsun (önceden kayıtlı cihazlar için de; çevrimdışıysa sonra)
    let _ = social.ensure_key();
    for active in beats {
        let _ = social.ensure_key();
        let theme = lock(&shared).theme.clone();
        match social.heartbeat(active, social::tz_minutes(), theme.as_deref()) {
            Ok(Some(v)) => {
                let _ = app.emit("social://status", v);
            }
            Ok(None) => {}
            Err(e) => {
                let _ = app.emit("social://error", e.to_string());
            }
        }
    }
}

/// Geçmiş günleri transcript'lerden bir kez okuyup dondurur, `ledger.json`'a yazar ve engine'e geçmiş
/// toplamları gönderir. İlk çalıştırmada bütün transcript'ler okunur (arka planda); sonra yalnızca yeni günler.
fn run_ledger(app: AppHandle, tx: Sender<PastTotals>) {
    let (Some(root), Some(data)) = (paths::claude_dir(), app_data(&app)) else {
        return;
    };
    let path = data.join("ledger.json");
    let prices = load_prices(&app);
    let mut ledger = Ledger::load(&path);
    let mut sent_for = None;
    loop {
        let today = chrono::Local::now().date_naive();
        if ledger.needs_freeze(today) {
            let started = Instant::now();
            let stats = ledger.backfill(&root, today, &prices);
            eprintln!(
                "[ledger] {} dosya, {} mesaj, {:?}",
                stats.files,
                stats.messages,
                started.elapsed()
            );
            if let Err(e) = ledger.save(&path) {
                eprintln!("[ledger] kaydedilemedi: {e}");
            }
            sent_for = None;
        }
        if sent_for != Some(today) {
            if tx.send(ledger.past_totals(today)).is_err() {
                return;
            }
            sent_for = Some(today);
        }
        std::thread::sleep(LEDGER_POLL);
    }
}

/// Uygulama menüsü. Varsayılan menüdeki "Close Window ⌘W" yok: ⌘W terminalde yalnızca bölmeyi kapatır
/// (oturum yaşar). Kopyala/yapıştır kısayollarının webview'da çalışması için Edit menüsü durur.
fn app_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, PredefinedMenuItem, Submenu};
    let app_menu = Submenu::with_items(
        app,
        APP_NAME,
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &edit, &window])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shared: SharedState = Arc::default();
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Updates::default())
        .menu(app_menu)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                watchdog::forget(&format!("ui:{}", window.label()));
            }
        })
        .manage(shared.clone())
        .manage(term::Terminals::new(host::default_socket()))
        .invoke_handler(tauri::generate_handler![
            engine_initial,
            danger_kill,
            chat_last_reply,
            integrations_status,
            integration_set,
            integrations_remove_all,
            savers_status,
            term::term_state,
            term::term_create,
            term::term_attach,
            term::term_input,
            term::term_resize,
            term::term_detach,
            term::term_kill,
            term::term_kill_leftover,
            term::term_open_window,
            term::open_path,
            term::focus_external,
            term::git_branch,
            git_repos,
            git_status,
            git_log,
            git_ai_message,
            git_run,
            git_branches,
            git_checkout,
            git_edited,
            activity_days,
            report_error,
            ui_beat,
            update_state,
            update_check,
            update_install,
            update_whats_new,
            open_licenses,
            reveal_error_log,
            social_state,
            social_register,
            social_rename,
            social_get,
            social_privacy,
            social_post,
            social_set_theme,
            chat_window,
            git_diff,
            git_file,
            git_blame,
        ])
        .setup(move |app| {
            // Panikler ve donmalar yerel günlüğe (errors.log); bekçi iş parçacığı nabızlara bakar.
            if let Ok(dir) = app.path().app_data_dir() {
                watchdog::install_panic_hook(dir);
            }
            let handle = app.handle().clone();
            let ups = app.state::<updates::Updates>().inner().clone();
            std::thread::Builder::new()
                .name("updates".into())
                .spawn(move || updates::run(handle, ups))?;
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("watchdog".into())
                .spawn(move || watchdog::run(handle))?;
            // Dört parmak tıklaması: ana pencere öne gelir ve kısayol çarkı açılır.
            let gesture_app = app.handle().clone();
            gesture::install(move || {
                if let Some(w) = gesture_app.get_webview_window("main") {
                    let _ = w.set_focus();
                    let _ = w.emit("wheel://open", ());
                }
            });
            // Uygulamanın kendi koyduğu rtk kopyası eskiyse güncellenir (kullanıcınınkine dokunulmaz).
            if let Ok(env) = savers_env(app.handle()) {
                std::thread::spawn(move || env.refresh_managed_rtk());
            }
            // Oturum sunucusuna bağlan (yoksa başlatılır); önceki çalıştırmadan kalan terminaller burada.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                if let Err(e) = handle.state::<term::Terminals>().control(&handle) {
                    eprintln!("[host] {e}");
                }
            });
            // Sosyal sunucu istemcisi: token yalnızca Rust'ta
            let data = app.path().app_data_dir()?;
            let social = Arc::new(social::Social::new(&data, social::server_url()));
            app.manage(social.clone());
            let (beat_tx, beat_rx) = mpsc::channel::<bool>();
            let handle = app.handle().clone();
            let shared_social = shared.clone();
            let live_social = social.clone();
            std::thread::Builder::new()
                .name("social".into())
                .spawn(move || run_social(handle, social, shared_social, beat_rx))?;
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("live".into())
                .spawn(move || live::run(handle, live_social))?;

            let (tx, rx) = mpsc::channel();
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("engine".into())
                .spawn(move || run_engine(handle, shared, rx, beat_tx))?;
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("ledger".into())
                .spawn(move || run_ledger(handle, tx))?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
