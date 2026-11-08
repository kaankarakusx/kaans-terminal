//! Uygulama tarafı: oturum sunucusuna bağlanır (yoksa başlatır).
//!
//! - `Control`: tek, paylaşılan bağlantı — liste, oluşturma, sonlandırma ve olaylar.
//! - `Attachment`: ekrandaki her terminal bölmesi için ayrı bağlantı; geçmiş yalnızca ona oynatılır.

use super::protocol::{
    CONTROL, CreateSpec, Event, INPUT, Leftover, OUTPUT, PROTOCOL_VERSION, Request, SessionInfo,
    read_frame, write_frame, write_json,
};
use interprocess::local_socket::{Stream, prelude::*};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Result<String, String>>>>>;

fn connect(socket: &Path) -> io::Result<Stream> {
    Stream::connect(super::socket_name(socket)?)
}

/// Sunucu ayakta mı (bağlanıp hemen kapatır).
pub fn probe(socket: &Path) -> io::Result<()> {
    connect(socket).map(drop)
}

/// Bağlanır; sunucu yoksa başlatıp 3 sn'ye kadar bekler.
fn connect_or_spawn(socket: &Path) -> io::Result<Stream> {
    if let Ok(s) = connect(socket) {
        return Ok(s);
    }
    super::spawn_host(socket)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match connect(socket) {
            Ok(s) => return Ok(s),
            Err(e) if Instant::now() >= deadline => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HostState {
    pub sessions: Vec<SessionInfo>,
    pub leftovers: Vec<Leftover>,
}

pub struct Control {
    socket: PathBuf,
    writer: Mutex<Box<dyn Write + Send>>,
    pending: Pending,
    next_req: AtomicU64,
    connected: Arc<AtomicBool>,
    pub state: Arc<Mutex<HostState>>,
}

impl Control {
    /// `on_event`: Sessions / Exit olayları; `on_close`: bağlantı koptu (sunucu öldü).
    pub fn connect(
        socket: &Path,
        on_event: impl Fn(&Event) + Send + 'static,
        on_close: impl FnOnce() + Send + 'static,
    ) -> io::Result<Arc<Self>> {
        let mut stream = connect_or_spawn(socket)?;
        write_json(
            &mut stream,
            &Request::Hello {
                version: PROTOCOL_VERSION,
            },
        )?;
        let (kind, _, body) = read_frame(&mut stream)?;
        let hello = (kind == CONTROL)
            .then(|| serde_json::from_slice::<Event>(&body).ok())
            .flatten();
        if let Some(Event::Hello { version, .. }) = hello
            && version != PROTOCOL_VERSION
        {
            // Güncellemeden kalma eski sunucu: oturumu yoksa kapanır ve yenisi başlatılır.
            write_json(&mut stream, &Request::ShutdownIfIdle)?;
            drop(stream);
            std::thread::sleep(Duration::from_millis(300));
            stream = connect_or_spawn(socket)?;
            write_json(
                &mut stream,
                &Request::Hello {
                    version: PROTOCOL_VERSION,
                },
            )?;
            let _ = read_frame(&mut stream)?;
        }

        let tx = super::SharedStream(Arc::new(stream));
        let mut rx = tx.clone();
        let pending: Pending = Arc::default();
        let state: Arc<Mutex<HostState>> = Arc::default();
        let connected = Arc::new(AtomicBool::new(true));
        let control = Arc::new(Self {
            socket: socket.to_path_buf(),
            writer: Mutex::new(Box::new(tx)),
            pending: Arc::clone(&pending),
            next_req: AtomicU64::new(1),
            connected: Arc::clone(&connected),
            state: Arc::clone(&state),
        });

        std::thread::Builder::new()
            .name("host-control".into())
            .spawn(move || {
                while let Ok((kind, _, body)) = read_frame(&mut rx) {
                    if kind != CONTROL {
                        continue;
                    }
                    let Ok(ev) = serde_json::from_slice::<Event>(&body) else {
                        continue;
                    };
                    match &ev {
                        Event::Created { req, id } => {
                            if let Some(tx) = pending
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .remove(req)
                            {
                                let _ = tx.send(Ok(id.clone()));
                            }
                        }
                        Event::Error {
                            req: Some(req),
                            message,
                        } => {
                            if let Some(tx) = pending
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .remove(req)
                            {
                                let _ = tx.send(Err(message.clone()));
                            }
                        }
                        Event::Sessions { list, leftovers } => {
                            let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                            s.sessions.clone_from(list);
                            s.leftovers.clone_from(leftovers);
                        }
                        _ => {}
                    }
                    on_event(&ev);
                }
                connected.store(false, Ordering::Relaxed);
                on_close();
            })?;
        Ok(control)
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    fn send(&self, req: &Request) -> io::Result<()> {
        let mut w = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        write_json(&mut *w, req)
    }

    /// Yeni terminal oturumu; kimliğini döndürür.
    pub fn create(&self, spec: CreateSpec) -> Result<String, String> {
        let req = self.next_req.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(req, tx);
        self.send(&Request::Create { req, spec })
            .map_err(|e| e.to_string())?;
        let res = rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "oturum sunucusu cevap vermedi".to_string());
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&req);
        res?
    }

    pub fn kill(&self, id: &str) -> io::Result<()> {
        self.send(&Request::Kill { id: id.into() })
    }

    pub fn kill_leftover(&self, pid: u32) -> io::Result<()> {
        self.send(&Request::KillLeftover { pid })
    }

    pub fn snapshot(&self) -> HostState {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Ekrandaki bir terminal bölmesinin oturuma bağlantısı.
pub struct Attachment {
    id: String,
    writer: Mutex<Box<dyn Write + Send>>,
}

impl Attachment {
    /// Önce geçmiş, sonra canlı çıktı `on_output`'a gelir. Oturum biter ya da bağlantı koparsa `on_end`.
    pub fn open(
        socket: &Path,
        id: &str,
        on_output: impl Fn(Vec<u8>) + Send + 'static,
        on_end: impl FnOnce() + Send + 'static,
    ) -> io::Result<Self> {
        let mut stream = connect(socket)?;
        write_json(
            &mut stream,
            &Request::Hello {
                version: PROTOCOL_VERSION,
            },
        )?;
        write_json(&mut stream, &Request::Attach { id: id.into() })?;
        let tx = super::SharedStream(Arc::new(stream));
        let mut rx = tx.clone();
        let sid = id.to_string();
        std::thread::Builder::new()
            .name(format!("attach-{id}"))
            .spawn(move || {
                // Oturum bitse de bağlantı kapanana (ya da Detached gelene) kadar okumaya devam edilir:
                // okunmayan soket dolarsa sunucu bu istemciye yazamaz ve onu düşürmek zorunda kalır.
                let mut on_end = Some(on_end);
                while let Ok((kind, fid, body)) = read_frame(&mut rx) {
                    if kind == OUTPUT && fid == sid {
                        if on_end.is_some() {
                            on_output(body);
                        }
                    } else if kind == CONTROL {
                        match serde_json::from_slice::<Event>(&body) {
                            Ok(Event::Exit { id, .. }) if id == sid => {
                                if let Some(f) = on_end.take() {
                                    f();
                                }
                            }
                            Ok(Event::Detached { id }) if id == sid => return,
                            Ok(Event::Error { req: None, .. }) => {
                                if let Some(f) = on_end.take() {
                                    f();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(f) = on_end.take() {
                    f();
                }
            })?;
        Ok(Self {
            id: id.into(),
            writer: Mutex::new(Box::new(tx)),
        })
    }

    pub fn input(&self, bytes: &[u8]) -> io::Result<()> {
        let mut w = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        write_frame(&mut *w, INPUT, &self.id, bytes)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        let mut w = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        write_json(
            &mut *w,
            &Request::Resize {
                id: self.id.clone(),
                cols,
                rows,
            },
        )
    }
}

impl Drop for Attachment {
    fn drop(&mut self) {
        // Sunucu "Detached" ile cevap verir; okuma iş parçacığı onu alınca biter ve bağlantı kapanır.
        let mut w = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        let _ = write_json(
            &mut *w,
            &Request::Detach {
                id: self.id.clone(),
            },
        );
    }
}
