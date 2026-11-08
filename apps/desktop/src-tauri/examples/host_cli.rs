//! Oturum sunucusuna küçük bir komut satırı (geliştirme ve elle test için).
//!   cargo run --example host_cli -- list
//!   cargo run --example host_cli -- create <cwd> "<komut>"      (komut kabukta çalışır, kabuk açık kalır)
//!   cargo run --example host_cli -- replay <id>                  (geçmişi yazdırır)
//!   cargo run --example host_cli -- kill <id>
//!   cargo run --example host_cli -- kill-leftover <pid>
use kaans_terminal_lib::host::{
    self,
    client::{Attachment, Control},
    protocol::CreateSpec,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let socket = host::default_socket();
    if host::client::probe(&socket).is_err() {
        eprintln!("sunucu çalışmıyor");
        std::process::exit(1);
    }
    let ctrl = Control::connect(&socket, |_| {}, || {}).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    match args.first().map(String::as_str) {
        Some("create") => {
            let cwd = args.get(1).cloned().unwrap_or_else(|| "/tmp".into());
            let id = ctrl
                .create(CreateSpec {
                    cwd,
                    command: vec![],
                    cols: 100,
                    rows: 30,
                    env: vec![],
                })
                .unwrap();
            if let Some(cmd) = args.get(2) {
                let att = Attachment::open(&socket, &id, |_| {}, || {}).unwrap();
                std::thread::sleep(Duration::from_millis(1200));
                att.input(format!("{cmd}\r").as_bytes()).unwrap();
                std::thread::sleep(Duration::from_millis(500));
            }
            println!("{id}");
        }
        Some("replay") => {
            let out = Arc::new(Mutex::new(Vec::<u8>::new()));
            let o = Arc::clone(&out);
            let _att = Attachment::open(
                &socket,
                &args[1],
                move |b| o.lock().unwrap().extend(b),
                || {},
            )
            .unwrap();
            std::thread::sleep(Duration::from_millis(800));
            println!("{}", String::from_utf8_lossy(&out.lock().unwrap()));
        }
        Some("kill") => ctrl.kill(&args[1]).unwrap(),
        Some("kill-leftover") => ctrl.kill_leftover(args[1].parse().unwrap()).unwrap(),
        _ => {
            let s = ctrl.snapshot();
            for x in s.sessions {
                println!(
                    "{}  {}  busy={} fg={:?} attached={}",
                    x.id, x.cwd, x.busy, x.fg_name, x.attached
                );
            }
            for l in s.leftovers {
                println!("leftover {} {} ({})", l.pid, l.name, l.session);
            }
        }
    }
}
