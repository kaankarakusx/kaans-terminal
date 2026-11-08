//! Çalışan uygulamanın oturum sunucusunu sınar: kabuk aç, komut çalıştır, çıktıyı oku, End Session.
//!   cargo run --example host_probe
use kaans_terminal_lib::host::{
    self,
    client::{Attachment, Control},
    protocol::CreateSpec,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn main() {
    let socket = host::default_socket();
    host::client::probe(&socket).expect("sunucu çalışmıyor (uygulama açık mı?)");
    let ctrl = Control::connect(&socket, |_| {}, || {}).unwrap();
    println!("bağlı; mevcut oturum: {}", ctrl.snapshot().sessions.len());
    let id = ctrl
        .create(CreateSpec {
            cwd: "/tmp".into(),
            command: vec![],
            cols: 100,
            rows: 30,
            env: vec![],
        })
        .unwrap();
    println!("oturum: {id}");
    let out = Arc::new(Mutex::new(Vec::<u8>::new()));
    let o = Arc::clone(&out);
    let att = Attachment::open(&socket, &id, move |b| o.lock().unwrap().extend(b), || {}).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    att.input(b"echo probe-$((40+2)); pwd\r").unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let text = String::from_utf8_lossy(&out.lock().unwrap()).to_string();
        if text.contains("probe-42") && text.contains("/tmp") {
            println!("kabuk çalışıyor: komut çıktısı alındı ✓");
            break;
        }
        if Instant::now() > deadline {
            println!("ZAMAN AŞIMI. çıktı:\n{text}");
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(2500));
    let s = ctrl.snapshot();
    let me = s.sessions.iter().find(|x| x.id == id);
    println!(
        "listede: {:?}",
        me.map(|m| (&m.cwd, &m.command, m.busy, &m.fg_name, m.attached))
    );
    drop(att);
    ctrl.kill(&id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    while ctrl.snapshot().sessions.iter().any(|x| x.id == id) {
        if Instant::now() > deadline {
            println!("End Session sonrası oturum hâlâ listede!");
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("End Session: oturum kapandı ✓");
}
