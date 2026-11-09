//! Terminal komutları (sayfa → Rust) ve terminal pencereleri. Oturumları `host` sunucusu tutar.

use crate::host::client::{Attachment, Control, HostState};
use crate::host::protocol::{CreateSpec, Event, Leftover, SessionInfo};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

/// Rust → sayfa olayları (web/src/lib/term/*.ts ile aynı).
pub const EV_SESSIONS: &str = "host://sessions";
pub const EV_EXIT: &str = "host://exit";
pub const EV_ENDED: &str = "term://ended";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TermState {
    pub sessions: Vec<SessionInfo>,
    pub leftovers: Vec<Leftover>,
}

impl From<HostState> for TermState {
    fn from(s: HostState) -> Self {
        Self {
            sessions: s.sessions,
            leftovers: s.leftovers,
        }
    }
}

pub struct Terminals {
    socket: PathBuf,
    control: Mutex<Option<Arc<Control>>>,
    attachments: Mutex<HashMap<u64, Attachment>>,
    next_attach: AtomicU64,
    next_window: AtomicU64,
}

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Terminals {
    pub fn new(socket: PathBuf) -> Self {
        Self {
            socket,
            control: Mutex::new(None),
            attachments: Mutex::new(HashMap::new()),
            next_attach: AtomicU64::new(1),
            next_window: AtomicU64::new(1),
        }
    }

    /// Kontrol bağlantısı; yoksa ya da koptuysa bağlanır (sunucu yoksa başlatır).
    pub fn control(&self, app: &AppHandle) -> Result<Arc<Control>, String> {
        let mut c = guard(&self.control);
        if let Some(ctrl) = c.as_ref().filter(|c| c.is_connected()) {
            return Ok(Arc::clone(ctrl));
        }
        let app_ev = app.clone();
        let ctrl = Control::connect(
            &self.socket,
            move |ev| match ev {
                Event::Sessions { list, leftovers } => {
                    let _ = app_ev.emit(
                        EV_SESSIONS,
                        TermState {
                            sessions: list.clone(),
                            leftovers: leftovers.clone(),
                        },
                    );
                }
                Event::Exit { id, code, ended } => {
                    let _ = app_ev.emit(
                        EV_EXIT,
                        serde_json::json!({ "id": id, "code": code, "ended": ended }),
                    );
                }
                _ => {}
            },
            || {},
        )
        .map_err(|e| format!("oturum sunucusuna bağlanılamadı: {e}"))?;
        *c = Some(Arc::clone(&ctrl));
        Ok(ctrl)
    }

    /// Engine için son bilinen durum (bağlantı yoksa boş).
    pub fn state(&self) -> HostState {
        guard(&self.control)
            .as_ref()
            .map(|c| c.snapshot())
            .unwrap_or_default()
    }
}

fn home() -> String {
    std::env::home_dir()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".into())
}

/// Sync komutlar Tauri'de ana (UI) iş parçacığında çalışır. Soket/süreç/dosya bekleyen her şey buradan geçer:
/// bekleme sırasında bütün pencereler donmasın.
async fn off_main<T: Send + 'static>(
    name: &'static str,
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || crate::watchdog::timed(name, f))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn term_state(app: AppHandle) -> Result<TermState, String> {
    off_main("term_state", move || {
        Ok(app.state::<Terminals>().control(&app)?.snapshot().into())
    })
    .await
}

#[tauri::command]
pub async fn term_create(
    app: AppHandle,
    t: State<'_, Terminals>,
    cwd: Option<String>,
    command: Option<Vec<String>>,
    cols: u16,
    rows: u16,
) -> Result<String, String> {
    // "~" ve "~/x" ev klasörüne çözülür; olmayan klasör yerine ev klasörü.
    let cwd = cwd
        .map(|c| match c.strip_prefix('~') {
            Some(rest) => format!("{}{}", home(), rest),
            None => c,
        })
        .filter(|c| std::path::Path::new(c).is_dir())
        .unwrap_or_else(home);
    let ctrl = t.control(&app)?;
    // rtk entegrasyonu uygulamanın kopyasını kurduysa terminallerin PATH'inde olsun (hook çıplak `rtk` çağırır).
    let mut env = vec![];
    if let Some(bin) = app
        .path()
        .app_data_dir()
        .ok()
        .and_then(|d| crate::savers::managed_bin_dir_from(&d))
    {
        let sep = if cfg!(windows) { ';' } else { ':' };
        let path = std::env::var("PATH").unwrap_or_default();
        env.push(("PATH".to_string(), format!("{}{sep}{path}", bin.display())));
    }
    // create sunucunun cevabını bekler; async komut ana iş parçacığını tutmasın.
    tauri::async_runtime::spawn_blocking(move || {
        ctrl.create(CreateSpec {
            cwd,
            command: command.unwrap_or_default(),
            cols,
            rows,
            env,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Bölmeyi oturuma bağlar: önce geçmiş, sonra canlı çıktı ham bayt olarak `on_output`'a akar.
#[tauri::command]
pub async fn term_attach(
    app: AppHandle,
    id: String,
    on_output: Channel<InvokeResponseBody>,
) -> Result<u64, String> {
    off_main("term_attach", move || {
        let t = app.state::<Terminals>();
        t.control(&app)?;
        let attach = t.next_attach.fetch_add(1, Ordering::Relaxed);
        let app_end = app.clone();
        let a = Attachment::open(
            &t.socket,
            &id,
            move |bytes| {
                let _ = on_output.send(InvokeResponseBody::Raw(bytes));
            },
            move || {
                let _ = app_end.emit(EV_ENDED, attach);
            },
        )
        .map_err(|e| e.to_string())?;
        guard(&t.attachments).insert(attach, a);
        Ok(attach)
    })
    .await
}

#[tauri::command]
pub fn term_input(t: State<'_, Terminals>, attach: u64, data: String) -> Result<(), String> {
    match guard(&t.attachments).get(&attach) {
        Some(a) => a.input(data.as_bytes()).map_err(|e| e.to_string()),
        None => Err("bölme bağlı değil".into()),
    }
}

#[tauri::command]
pub fn term_resize(
    t: State<'_, Terminals>,
    attach: u64,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    match guard(&t.attachments).get(&attach) {
        Some(a) => a.resize(cols, rows).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Bölme kapandı: oturum yaşamaya devam eder (⌘W, ×).
#[tauri::command]
pub fn term_detach(t: State<'_, Terminals>, attach: u64) {
    guard(&t.attachments).remove(&attach);
}

/// End Session: oturumu ve alt süreçlerini bitirir.
#[tauri::command]
pub fn term_kill(app: AppHandle, t: State<'_, Terminals>, id: String) -> Result<(), String> {
    t.control(&app)?.kill(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn term_kill_leftover(app: AppHandle, t: State<'_, Terminals>, pid: u32) -> Result<(), String> {
    t.control(&app)?
        .kill_leftover(pid)
        .map_err(|e| e.to_string())
}

/// Terminal penceresi açar (ya da varsa öne getirir). `session` verilirse o oturum bir sekmede açılır.
#[tauri::command]
pub async fn term_open_window(
    app: AppHandle,
    t: State<'_, Terminals>,
    label: Option<String>,
    session: Option<String>,
    title: Option<String>,
) -> Result<String, String> {
    if let Some(l) = &label
        && let Some(w) = app.get_webview_window(l)
    {
        if let Some(s) = &session {
            let _ = w.emit_to(l.as_str(), "term://open", s);
        }
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(l.clone());
    }
    let label = label.unwrap_or_else(|| {
        let n = t.next_window.fetch_add(1, Ordering::Relaxed);
        format!("term-{}-{n}", chrono::Utc::now().timestamp_millis())
    });
    let mut hash = format!("term={label}");
    if let Some(s) = &session {
        hash.push_str("&open=");
        hash.push_str(s);
    }
    let builder = WebviewWindowBuilder::new(
        &app,
        &label,
        WebviewUrl::App(format!("index.html#{hash}").into()),
    )
    .title(title.unwrap_or_else(|| crate::APP_NAME.into()))
    .inner_size(1000.0, 640.0)
    .min_inner_size(420.0, 260.0)
    // Pencereler yarı saydam (OZELLIKLER.md §3.4): pencere saydam, zemin sayfada rgba ile boyanır.
    .transparent(true);
    // Mac: sistem başlık çubuğu yok; trafik ışıkları pencerenin kendi başlık satırının içinde (TASARIM.md §4.2).
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    crate::watchdog::timed("term_open_window", || builder.build()).map_err(|e| e.to_string())?;
    Ok(label)
}

/// ⌘-tık: dosyayı kendi uygulamasında açar. Göreli yol terminalin klasörüne göre; `:12:3` gibi satır eki atılır.
#[tauri::command]
pub async fn open_path(app: AppHandle, path: String, cwd: Option<String>) -> Result<(), String> {
    off_main("open_path", move || open_path_sync(&app, path, cwd)).await
}

fn open_path_sync(app: &AppHandle, path: String, cwd: Option<String>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let mut p = path
        .trim()
        .trim_matches(|c| c == '\'' || c == '"')
        .to_string();
    // dosya.rs:12:3 → dosya.rs
    while let Some((head, tail)) = p.rsplit_once(':') {
        if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) {
            p = head.to_string();
        } else {
            break;
        }
    }
    let expanded = if let Some(rest) = p.strip_prefix("~/") {
        std::env::home_dir()
            .map(|h| h.join(rest))
            .unwrap_or_else(|| PathBuf::from(&p))
    } else {
        let pb = PathBuf::from(&p);
        if pb.is_absolute() {
            pb
        } else {
            PathBuf::from(cwd.unwrap_or_else(home)).join(pb)
        }
    };
    if !expanded.exists() {
        return Err(format!("dosya yok: {}", expanded.display()));
    }
    app.opener()
        .open_path(expanded.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// iTerm2 ya da Terminal'de çalışan bir sohbeti öne getirir (tty eşleştirmesi, AppleScript).
/// Yalnızca kullanıcı tıklayınca çalışır; uygulama kapalıysa onu açmaz. İlk seferde macOS otomasyon izni ister.
#[tauri::command]
pub async fn focus_external(pid: u32) -> Result<bool, String> {
    off_main("focus_external", move || focus_external_sync(pid)).await
}

fn focus_external_sync(pid: u32) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        let Some(tty) = crate::procs::tty(pid) else {
            return Ok(false);
        };
        if !tty.starts_with("/dev/ttys") {
            return Ok(false);
        }
        let iterm = format!(
            r#"if application "iTerm2" is running then
  tell application "iTerm2"
    repeat with w in windows
      repeat with t in tabs of w
        repeat with s in sessions of t
          if tty of s is "{tty}" then
            select w
            tell t to select
            tell s to select
            activate
            return "ok"
          end if
        end repeat
      end repeat
    end repeat
  end tell
end if
if application "Terminal" is running then
  tell application "Terminal"
    repeat with w in windows
      repeat with t in tabs of w
        if tty of t is "{tty}" then
          set selected of t to true
          set index of w to 1
          activate
          return "ok"
        end if
      end repeat
    end repeat
  end tell
end if
return "no""#
        );
        let out = std::process::Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(iterm)
            .output()
            .map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&out.stdout).trim() == "ok")
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        Ok(false)
    }
}

/// Klasörün git dalı: `.git/HEAD` okunur (yukarı doğru aranır; worktree'de `.git` dosyası gitdir'i gösterir).
/// git çalıştırılmaz. Dal değilse (detached) kısa commit.
#[tauri::command]
pub async fn git_branch(cwd: String) -> Option<String> {
    off_main("git_branch", move || {
        Ok(git_branch_of(std::path::Path::new(&cwd)))
    })
    .await
    .ok()
    .flatten()
}

pub fn git_branch_of(start: &std::path::Path) -> Option<String> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let dot = d.join(".git");
        let git_dir = if dot.is_dir() {
            Some(dot)
        } else if dot.is_file() {
            std::fs::read_to_string(&dot)
                .ok()
                .and_then(|t| t.trim().strip_prefix("gitdir:").map(|p| d.join(p.trim())))
        } else {
            None
        };
        if let Some(g) = git_dir {
            let head = std::fs::read_to_string(g.join("HEAD")).ok()?;
            let head = head.trim();
            return Some(match head.strip_prefix("ref: refs/heads/") {
                Some(b) => b.to_string(),
                None => head.chars().take(7).collect(),
            });
        }
        dir = d.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::git_branch_of;
    use std::fs;

    #[test]
    fn reads_branch_from_head_upwards_and_worktrees() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("src/deep")).unwrap();
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        assert_eq!(
            git_branch_of(&repo.join("src/deep")).as_deref(),
            Some("feature/x")
        );
        fs::write(repo.join(".git/HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(git_branch_of(&repo).as_deref(), Some("0123456"));
        // worktree: .git dosyası
        let wt = tmp.path().join("wt");
        fs::create_dir_all(tmp.path().join("gd")).unwrap();
        fs::create_dir_all(&wt).unwrap();
        fs::write(tmp.path().join("gd/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            wt.join(".git"),
            format!("gitdir: {}\n", tmp.path().join("gd").display()),
        )
        .unwrap();
        assert_eq!(git_branch_of(&wt).as_deref(), Some("main"));
        assert_eq!(git_branch_of(&tmp.path().join("nowhere")), None);
    }
}
