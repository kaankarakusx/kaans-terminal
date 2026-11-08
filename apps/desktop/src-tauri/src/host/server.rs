//! Oturum sunucusu (MIMARI.md §4): PTY'leri uygulamadan bağımsız tutar. Uygulama kapanınca ya da
//! güncellenince terminaller çalışmaya devam eder; uygulama yeniden bağlanınca geçmiş yeniden oynatılır.
//!
//! Aynı ikili dosya `--session-host <soket>` ile başlatılınca bu modül çalışır (ayrı bir sidecar yok).

use super::protocol::{
    CONTROL, CreateSpec, Event, INPUT, Leftover, PROTOCOL_VERSION, Request, SessionInfo,
    read_frame, write_frame, write_json,
};
use crate::procs;
use interprocess::local_socket::{ListenerOptions, Stream, prelude::*};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::{HashMap, VecDeque};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;
use std::time::{Duration, Instant};

/// Oturum başına yeniden oynatma geçmişi.
const SCROLLBACK: usize = 2 * 1024 * 1024;
/// Hiç oturum ve bağlı uygulama kalmazsa sunucu bu süre sonra kapanır.
const IDLE_EXIT: Duration = Duration::from_secs(60);
const MAINTENANCE: Duration = Duration::from_secs(2);

/// İstemciye giden kuyruk. Sokete yazmak istemcinin kendi iş parçacığında olur: okumayan (takılmış ya da
/// okumayı bırakmış) bir istemci Host kilidini tutup bütün sunucuyu kilitleyemez. Kuyruk dolarsa istemci
/// düşürülür.
#[derive(Clone)]
struct Writer(mpsc::SyncSender<Vec<u8>>);
/// İstemci başına bekleyen çerçeve sınırı (çıktı parçaları 64 KB'a kadar)
const OUT_QUEUE: usize = 512;

struct Session {
    info: SessionInfo,
    master: Box<dyn MasterPty + Send>,
    /// PTY girdisi: kilit dışında yazılır (girdiyi okumayan bir süreç sunucuyu kilitlemesin)
    input: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn Child + Send + Sync>,
    scrollback: VecDeque<u8>,
    truncated: bool,
    /// Bağlı istemciler (yalnızca bunlara çıktı gider)
    attached: Vec<u64>,
    /// Son bilinen alt süreçler (pid, başlangıç): kabuk kapanınca geride kalanları bulmak için
    descendants: Vec<(u32, i64)>,
    /// End Session istendi
    ended: bool,
}

#[derive(Default)]
struct Host {
    sessions: HashMap<String, Session>,
    clients: HashMap<u64, Writer>,
    leftovers: Vec<(Leftover, i64)>,
    next_session: u64,
    idle_since: Option<Instant>,
}

fn lock(h: &Mutex<Host>) -> MutexGuard<'_, Host> {
    h.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Çerçeveyi istemcinin kuyruğuna koyar; asla beklemez. false: istemci gitmiş ya da kuyruğu dolu.
fn send(w: &Writer, kind: u8, id: &str, payload: &[u8]) -> bool {
    let mut buf = Vec::with_capacity(payload.len() + id.len() + 8);
    write_frame(&mut buf, kind, id, payload).is_ok() && w.0.try_send(buf).is_ok()
}

fn send_event(w: &Writer, ev: &Event) -> bool {
    let mut buf = Vec::new();
    write_json(&mut buf, ev).is_ok() && w.0.try_send(buf).is_ok()
}

impl Host {
    fn sessions_event(&self) -> Event {
        let mut list: Vec<SessionInfo> = self.sessions.values().map(|s| s.info.clone()).collect();
        list.sort_by_key(|s| s.started);
        Event::Sessions {
            list,
            leftovers: self.leftovers.iter().map(|(l, _)| l.clone()).collect(),
        }
    }

    fn broadcast(&mut self, ev: &Event) {
        let dead: Vec<u64> = self
            .clients
            .iter()
            .filter(|(_, w)| !send_event(w, ev))
            .map(|(id, _)| *id)
            .collect();
        for id in dead {
            self.drop_client(id);
        }
    }

    fn drop_client(&mut self, id: u64) {
        self.clients.remove(&id);
        for s in self.sessions.values_mut() {
            s.attached.retain(|c| *c != id);
            s.info.attached = s.attached.len() as u32;
        }
    }
}

/// PTY'nin ön plandaki süreç grubunun lideri (o an çalışan komut). Windows'ta bu kavram yok.
fn fg_leader(master: &dyn MasterPty) -> Option<u32> {
    #[cfg(unix)]
    {
        master
            .process_group_leader()
            .and_then(|p| u32::try_from(p).ok())
    }
    #[cfg(not(unix))]
    {
        let _ = master;
        None
    }
}

fn default_shell() -> Vec<String> {
    #[cfg(windows)]
    {
        vec![std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".into())]
    }
    #[cfg(not(windows))]
    {
        // Login shell: Finder'dan açılan uygulamanın eksik PATH'i kullanıcının profiliyle tamamlanır.
        vec![
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()),
            "-l".into(),
        ]
    }
}

fn spawn_session(host: &Arc<Mutex<Host>>, spec: CreateSpec) -> io::Result<String> {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: spec.rows.max(2),
            cols: spec.cols.max(10),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(io::Error::other)?;
    let argv = if spec.command.is_empty() {
        default_shell()
    } else {
        spec.command.clone()
    };
    let mut cmd = CommandBuilder::new(&argv[0]);
    cmd.args(&argv[1..]);
    cmd.cwd(&spec.cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "KaansTerminal");
    // Uygulama bir Claude oturumunun içinden başlatıldıysa (geliştirme) iç içe oturum işaretleri sızmasın: aksi halde
    // terminalde açılan Claude kendini alt oturum sanar, kayıt tutmaz ve uygulama onu sohbet olarak göremez.
    for (k, _) in std::env::vars_os() {
        if let Some(k) = k.to_str()
            && is_claude_internal(k)
        {
            cmd.env_remove(k);
        }
    }
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    let child = pty.slave.spawn_command(cmd).map_err(io::Error::other)?;
    drop(pty.slave);
    let reader = pty.master.try_clone_reader().map_err(io::Error::other)?;
    let input = pty.master.take_writer().map_err(io::Error::other)?;

    let id = {
        let mut h = lock(host);
        h.next_session += 1;
        let id = format!("t{}-{}", std::process::id(), h.next_session);
        let info = SessionInfo {
            id: id.clone(),
            cwd: spec.cwd.clone(),
            command: argv,
            pid: child.process_id(),
            fg_pid: child.process_id(),
            fg_name: None,
            busy: false,
            started: now_ms(),
            cols: spec.cols,
            rows: spec.rows,
            attached: 0,
        };
        h.sessions.insert(
            id.clone(),
            Session {
                info,
                master: pty.master,
                input: Arc::new(Mutex::new(input)),
                child,
                scrollback: VecDeque::new(),
                truncated: false,
                attached: Vec::new(),
                descendants: Vec::new(),
                ended: false,
            },
        );
        id
    };

    let host2 = Arc::clone(host);
    let sid = id.clone();
    thread::Builder::new()
        .name(format!("pty-{sid}"))
        .spawn(move || pump_output(host2, sid, reader))?;
    Ok(id)
}

/// PTY çıktısını geçmişe ve bağlı istemcilere aktarır; kabuk kapanınca oturumu kaldırır.
/// Claude Code'un kendi alt süreçlerine koyduğu iç değişkenler. Kullanıcı ayarları (`CLAUDE_CONFIG_DIR`,
/// `ANTHROPIC_*` vb.) korunur.
pub fn is_claude_internal(key: &str) -> bool {
    key == "CLAUDECODE"
        || key == "CLAUDE_PID"
        || key == "CLAUDE_EFFORT"
        || (key.starts_with("CLAUDE_CODE_")
            && !matches!(
                key,
                "CLAUDE_CODE_USE_BEDROCK"
                    | "CLAUDE_CODE_USE_VERTEX"
                    | "CLAUDE_CODE_MAX_OUTPUT_TOKENS"
            ))
}

fn pump_output(host: Arc<Mutex<Host>>, id: String, mut reader: Box<dyn Read + Send>) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let chunk = &buf[..n];
        let targets: Vec<(u64, Writer)> = {
            let mut h = lock(&host);
            let Host {
                sessions, clients, ..
            } = &mut *h;
            let Some(s) = sessions.get_mut(&id) else {
                break;
            };
            s.scrollback.extend(chunk);
            if s.scrollback.len() > SCROLLBACK {
                let cut = s.scrollback.len() - SCROLLBACK;
                s.scrollback.drain(..cut);
                s.truncated = true;
            }
            s.attached
                .iter()
                .filter_map(|c| clients.get(c).map(|w| (*c, w.clone())))
                .collect()
        };
        for (cid, w) in targets {
            if !send(&w, super::protocol::OUTPUT, &id, chunk) {
                lock(&host).drop_client(cid);
            }
        }
    }

    // Kabuk kapandı: oturumu kaldır, çıkış kodunu kilit dışında bekle (PTY kapanıp süreç henüz bitmemiş
    // olabilir), geride kalan süreçleri kaydet.
    let Some(mut s) = lock(&host).sessions.remove(&id) else {
        return;
    };
    let code = s.child.wait().ok().map(|st| st.exit_code());
    let table = procs::table();
    let mut h = lock(&host);
    let alive: HashMap<u32, &procs::ProcInfo> = table.iter().map(|p| (p.pid, p)).collect();
    for (pid, start) in &s.descendants {
        if let Some(p) = alive.get(pid)
            && p.start_secs == *start
        {
            let leftover = Leftover {
                pid: *pid,
                name: p.name.clone(),
                cwd: procs::cwd(*pid).unwrap_or_else(|| s.info.cwd.clone()),
                session: id.clone(),
                since: now_ms(),
            };
            h.leftovers.push((leftover, *start));
        }
    }
    let ev_exit = Event::Exit {
        id: id.clone(),
        code,
        ended: s.ended,
    };
    h.broadcast(&ev_exit);
    let ev = h.sessions_event();
    h.broadcast(&ev);
}

#[cfg(unix)]
fn signal(pid: u32, sig: libc::c_int) {
    if let Ok(p) = libc::pid_t::try_from(pid)
        && p > 1
    {
        // SAFETY: yalnızca sinyal gönderir.
        unsafe { libc::kill(p, sig) };
    }
}

#[cfg(unix)]
fn signal_group(pgid: u32, sig: libc::c_int) {
    if let Ok(p) = libc::pid_t::try_from(pgid)
        && p > 1
    {
        // SAFETY: yalnızca süreç grubuna sinyal gönderir.
        unsafe { libc::killpg(p, sig) };
    }
}

/// End Session: önce hangup, kısa bekleme, sonra TERM ve KILL. Hangup'ı umursamayan (port tutan dev server gibi)
/// alt süreçler de bitirilir.
fn kill_session(host: &Arc<Mutex<Host>>, id: &str) {
    let (shell, fg, mut targets) = {
        let mut h = lock(host);
        let Some(s) = h.sessions.get_mut(id) else {
            return;
        };
        s.ended = true;
        let s = &*s;
        let table = procs::table();
        let shell = s.info.pid.unwrap_or(0);
        let mut t: Vec<u32> = procs::descendants(shell, &table);
        for (pid, start) in &s.descendants {
            if !t.contains(pid)
                && table
                    .iter()
                    .any(|p| p.pid == *pid && p.start_secs == *start)
            {
                t.push(*pid);
            }
        }
        (shell, fg_leader(s.master.as_ref()), t)
    };
    targets.push(shell);
    let host = Arc::clone(host);
    let id = id.to_string();
    thread::spawn(move || {
        #[cfg(unix)]
        {
            if let Some(fg) = fg {
                signal_group(fg, libc::SIGHUP);
            }
            signal(shell, libc::SIGHUP);
            thread::sleep(Duration::from_millis(1200));
            for &p in &targets {
                signal(p, libc::SIGTERM);
            }
            thread::sleep(Duration::from_millis(1500));
            for &p in &targets {
                signal(p, libc::SIGKILL);
            }
        }
        #[cfg(not(unix))]
        {
            let _ = (fg, &targets);
        }
        // Windows'ta (ve garanti olsun diye her yerde) kabuğu doğrudan öldür.
        if let Some(s) = lock(&host).sessions.get_mut(&id) {
            let _ = s.child.kill();
        }
    });
}

fn kill_leftover(host: &Arc<Mutex<Host>>, pid: u32) {
    let known = {
        let mut h = lock(host);
        let before = h.leftovers.len();
        h.leftovers.retain(|(l, _)| l.pid != pid);
        let ev = h.sessions_event();
        h.broadcast(&ev);
        before != h.leftovers.len()
    };
    // Yalnızca bizim takip ettiğimiz kalıntılar öldürülür; rastgele bir pid değil.
    if !known {
        return;
    }
    #[cfg(unix)]
    thread::spawn(move || {
        signal(pid, libc::SIGTERM);
        thread::sleep(Duration::from_millis(2000));
        signal(pid, libc::SIGKILL);
    });
    #[cfg(not(unix))]
    let _ = pid;
}

/// Her 2 sn: ön plan süreci, alt süreç anlık görüntüsü, ölmüş kalıntılar, boşta kapanma.
fn maintenance(host: Arc<Mutex<Host>>, socket: std::path::PathBuf) {
    loop {
        thread::sleep(MAINTENANCE);
        let table = procs::table();
        let mut h = lock(&host);
        for s in h.sessions.values_mut() {
            let shell = s.info.pid.unwrap_or(0);
            s.descendants = procs::descendants(shell, &table)
                .into_iter()
                .filter_map(|pid| {
                    table
                        .iter()
                        .find(|p| p.pid == pid)
                        .map(|p| (pid, p.start_secs))
                })
                .collect();
            let fg = fg_leader(s.master.as_ref()).or(s.info.pid);
            s.info.fg_pid = fg;
            s.info.fg_name = fg
                .and_then(|p| table.iter().find(|x| x.pid == p))
                .map(|p| p.name.clone());
            s.info.busy = fg.is_some() && fg != s.info.pid;
        }
        h.leftovers.retain(|(l, start)| {
            table
                .iter()
                .any(|p| p.pid == l.pid && p.start_secs == *start)
        });
        let ev = h.sessions_event();
        h.broadcast(&ev);

        if h.sessions.is_empty() && h.clients.is_empty() {
            let since = *h.idle_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= IDLE_EXIT {
                let _ = std::fs::remove_file(&socket);
                std::process::exit(0);
            }
        } else {
            h.idle_since = None;
        }
    }
}

static NEXT_CLIENT: AtomicU64 = AtomicU64::new(1);

fn serve_client(host: Arc<Mutex<Host>>, conn: Stream) {
    let cid = NEXT_CLIENT.fetch_add(1, Ordering::Relaxed);
    let tx = super::SharedStream(Arc::new(conn));
    let mut rx = tx.clone();
    let (queue, outbox) = mpsc::sync_channel::<Vec<u8>>(OUT_QUEUE);
    let mut out = tx;
    let spawned = thread::Builder::new()
        .name(format!("client-out-{cid}"))
        .spawn(move || {
            for buf in outbox {
                if out.write_all(&buf).and_then(|()| out.flush()).is_err() {
                    break;
                }
            }
        });
    if spawned.is_err() {
        return;
    }
    let writer = Writer(queue);
    lock(&host).clients.insert(cid, writer.clone());

    while let Ok((kind, id, payload)) = read_frame(&mut rx) {
        // Kuyruğu taştığı için düşürülen istemci: bağlantıyı bırak.
        if !lock(&host).clients.contains_key(&cid) {
            break;
        }
        if kind == INPUT {
            let input = lock(&host).sessions.get(&id).map(|s| Arc::clone(&s.input));
            if let Some(input) = input {
                let mut w = input.lock().unwrap_or_else(|e| e.into_inner());
                let _ = w.write_all(&payload);
                let _ = w.flush();
            }
            continue;
        }
        if kind != CONTROL {
            continue;
        }
        let Ok(req) = serde_json::from_slice::<Request>(&payload) else {
            send_event(
                &writer,
                &Event::Error {
                    req: None,
                    message: "geçersiz istek".into(),
                },
            );
            continue;
        };
        match req {
            Request::Hello { .. } => {
                send_event(
                    &writer,
                    &Event::Hello {
                        version: PROTOCOL_VERSION,
                        pid: std::process::id(),
                    },
                );
                let ev = lock(&host).sessions_event();
                send_event(&writer, &ev);
            }
            Request::List => {
                let ev = lock(&host).sessions_event();
                send_event(&writer, &ev);
            }
            Request::Create { req, spec } => match spawn_session(&host, spec) {
                Ok(id) => {
                    send_event(&writer, &Event::Created { req, id });
                    let mut h = lock(&host);
                    let ev = h.sessions_event();
                    h.broadcast(&ev);
                }
                Err(e) => {
                    send_event(
                        &writer,
                        &Event::Error {
                            req: Some(req),
                            message: e.to_string(),
                        },
                    );
                }
            },
            Request::Attach { id } => {
                // Geçmiş + "attached" işareti aynı kilit altında: arada canlı çıktı kaçmaz ya da iki kez gelmez.
                let mut h = lock(&host);
                let Some(s) = h.sessions.get_mut(&id) else {
                    send_event(
                        &writer,
                        &Event::Error {
                            req: None,
                            message: format!("oturum yok: {id}"),
                        },
                    );
                    continue;
                };
                let mut replay: Vec<u8> = s.scrollback.iter().copied().collect();
                if s.truncated {
                    // Kesilmiş geçmiş bir kaçış dizisinin ortasından başlamasın: ilk satır sonundan başlat.
                    if let Some(nl) = replay.iter().position(|&b| b == b'\n') {
                        replay.drain(..=nl);
                    }
                }
                if !s.attached.contains(&cid) {
                    s.attached.push(cid);
                }
                s.info.attached = s.attached.len() as u32;
                if !replay.is_empty() {
                    send(&writer, super::protocol::OUTPUT, &id, &replay);
                }
                send_event(&writer, &Event::Attached { id });
            }
            Request::Detach { id } => {
                if let Some(s) = lock(&host).sessions.get_mut(&id) {
                    s.attached.retain(|c| *c != cid);
                    s.info.attached = s.attached.len() as u32;
                }
                send_event(&writer, &Event::Detached { id });
            }
            Request::Resize { id, cols, rows } => {
                if let Some(s) = lock(&host).sessions.get_mut(&id) {
                    let size = PtySize {
                        rows: rows.max(2),
                        cols: cols.max(10),
                        pixel_width: 0,
                        pixel_height: 0,
                    };
                    if s.master.resize(size).is_ok() {
                        s.info.cols = cols;
                        s.info.rows = rows;
                    }
                }
            }
            Request::Kill { id } => kill_session(&host, &id),
            Request::KillLeftover { pid } => kill_leftover(&host, pid),
            Request::ShutdownIfIdle => {
                if lock(&host).sessions.is_empty() {
                    std::process::exit(0);
                }
            }
        }
    }
    lock(&host).drop_client(cid);
}

pub fn run(socket: &std::path::Path) -> io::Result<()> {
    let name = super::socket_name(socket)?;
    // Eski bir sunucudan kalan soket dosyası: bağlanılamıyorsa silinir.
    if Stream::connect(name.clone()).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "sunucu zaten çalışıyor",
        ));
    }
    #[cfg(unix)]
    let _ = std::fs::remove_file(socket);
    let listener = ListenerOptions::new().name(name).create_sync()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600));
    }

    let host = Arc::new(Mutex::new(Host::default()));
    {
        let host = Arc::clone(&host);
        let socket = socket.to_path_buf();
        thread::Builder::new()
            .name("maintenance".into())
            .spawn(move || maintenance(host, socket))?;
    }
    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let host = Arc::clone(&host);
        thread::Builder::new()
            .name("client".into())
            .spawn(move || serve_client(host, conn))?;
    }
    Ok(())
}
