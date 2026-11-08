//! Oturum sunucusu uçtan uca: gerçek PTY, gerçek yerel soket (geçici klasörde).
use super::client::{Attachment, Control};
use super::protocol::{CreateSpec, Event};
use super::server;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn start_server() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("h.sock");
    let s = socket.clone();
    std::thread::spawn(move || server::run(&s));
    // Sunucu ayağa kalkana kadar bekle; Control bağlanamazsa test ikilisini sunucu diye başlatmasın.
    let deadline = Instant::now() + Duration::from_secs(3);
    while super::client::probe(&socket).is_err() {
        assert!(Instant::now() < deadline, "sunucu başlamadı");
        std::thread::sleep(Duration::from_millis(20));
    }
    (dir, socket)
}

fn spec(cmd: &[&str]) -> CreateSpec {
    CreateSpec {
        cwd: std::env::temp_dir().to_string_lossy().into_owned(),
        command: cmd.iter().map(|s| s.to_string()).collect(),
        cols: 80,
        rows: 24,
        env: vec![],
    }
}

fn wait_for(buf: &Arc<Mutex<Vec<u8>>>, needle: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if String::from_utf8_lossy(&buf.lock().unwrap()).contains(needle) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn create_attach_replay_input_and_kill() {
    let (_dir, socket) = start_server();
    let (ev_tx, ev_rx) = mpsc::channel::<Event>();
    let ev_tx = Mutex::new(ev_tx);
    let control = Control::connect(
        &socket,
        move |e| {
            let _ = ev_tx.lock().unwrap().send(e.clone());
        },
        || {},
    )
    .unwrap();

    // Geçmiş: bağlanmadan önce yazılan çıktı bağlanınca yeniden oynatılır.
    let id = control
        .create(spec(&["/bin/sh", "-c", "printf 'early-bird\\n'; exec cat"]))
        .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let out = Arc::new(Mutex::new(Vec::new()));
    let o = Arc::clone(&out);
    let att = Attachment::open(&socket, &id, move |b| o.lock().unwrap().extend(b), || {}).unwrap();
    assert!(wait_for(&out, "early-bird"), "geçmiş oynatılmadı");

    // Girdi → cat → çıktı
    att.input(b"ping-123\n").unwrap();
    assert!(wait_for(&out, "ping-123"), "girdi yankılanmadı");
    att.resize(100, 30).unwrap();

    // Oturum listesi
    let deadline = Instant::now() + Duration::from_secs(5);
    while !control
        .snapshot()
        .sessions
        .iter()
        .any(|s| s.id == id && s.cols == 100)
    {
        assert!(Instant::now() < deadline, "liste güncellenmedi");
        std::thread::sleep(Duration::from_millis(50));
    }

    // End Session → çıkış olayı
    control.kill(&id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match ev_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Exit { id: x, ended, .. }) if x == id => {
                assert!(ended, "End Session ile bitti");
                break;
            }
            _ => assert!(Instant::now() < deadline, "oturum bitmedi"),
        }
    }
    drop(att);
}

#[test]
fn shell_exit_removes_session_and_create_errors_are_reported() {
    let (_dir, socket) = start_server();
    let control = Control::connect(&socket, |_| {}, || {}).unwrap();
    let id = control.create(spec(&["/bin/sh", "-c", "exit 3"])).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let s = control.snapshot();
        if !s.sessions.iter().any(|x| x.id == id) && s.sessions.is_empty() {
            break;
        }
        assert!(Instant::now() < deadline, "biten oturum listeden düşmedi");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(control.create(spec(&["/definitely/not/a/binary"])).is_err());
}

/// Okumayı bırakan bir istemci (soketi dolu) sunucuyu kilitlememeli: diğer istemciler çalışmaya, End Session
/// çalışmaya devam eder. Eskiden yayın Host kilidi altında sokete yazıyordu ve bütün sunucu donuyordu.
#[test]
fn a_client_that_stops_reading_does_not_freeze_the_host() {
    use super::protocol::{PROTOCOL_VERSION, Request, write_json};
    use interprocess::local_socket::{Stream, prelude::*};

    let (_dir, socket) = start_server();
    let control = Control::connect(&socket, |_| {}, || {}).unwrap();
    // Durmadan çıktı üreten oturum + ona bağlanıp hiç okumayan istemci
    let noisy = control.create(spec(&["/usr/bin/yes", "flood"])).unwrap();
    let mut stuck = Stream::connect(super::socket_name(&socket).unwrap()).unwrap();
    write_json(
        &mut stuck,
        &Request::Hello {
            version: PROTOCOL_VERSION,
        },
    )
    .unwrap();
    write_json(&mut stuck, &Request::Attach { id: noisy.clone() }).unwrap();
    std::thread::sleep(Duration::from_millis(800));

    // Yeni oturumlar açılıp (yayın tetikler) kullanılabilmeli
    for _ in 0..3 {
        let id = control.create(spec(&["/bin/cat"])).unwrap();
        let out = Arc::new(Mutex::new(Vec::new()));
        let o = Arc::clone(&out);
        let att =
            Attachment::open(&socket, &id, move |b| o.lock().unwrap().extend(b), || {}).unwrap();
        att.input(b"still-alive\n").unwrap();
        assert!(wait_for(&out, "still-alive"), "sunucu donmuş");
        control.kill(&id).unwrap();
    }
    // Gürültülü oturum da sonlandırılabilmeli
    control.kill(&noisy).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while control.snapshot().sessions.iter().any(|s| s.id == noisy) {
        assert!(Instant::now() < deadline, "End Session takıldı");
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(stuck);
}

#[test]
fn strips_only_claude_code_internals_from_terminal_env() {
    use super::server::is_claude_internal;
    for k in [
        "CLAUDECODE",
        "CLAUDE_CODE_CHILD_SESSION",
        "CLAUDE_CODE_SESSION_ID",
        "CLAUDE_CODE_ENTRYPOINT",
        "CLAUDE_PID",
        "CLAUDE_EFFORT",
    ] {
        assert!(is_claude_internal(k), "{k}");
    }
    for k in [
        "CLAUDE_CONFIG_DIR",
        "ANTHROPIC_API_KEY",
        "CLAUDE_CODE_USE_BEDROCK",
        "PATH",
        "HOME",
    ] {
        assert!(!is_claude_internal(k), "{k}");
    }
}
