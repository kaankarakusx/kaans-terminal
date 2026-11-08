//! Bu makinede çalışan uygulama komutlarını (dev sunucuları) listeler: `cargo run --example apps_dump`
use kaans_terminal_lib::{engine::apps::Detector, procs};

fn main() {
    let uid = procs::current_uid();
    let mut d = Detector::default();
    let now = || chrono::Utc::now().timestamp_millis();
    d.scan(now(), &procs::table(), uid, &procs::args, &procs::cwd);
    std::thread::sleep(std::time::Duration::from_secs(1));
    for a in d.scan(now(), &procs::table(), uid, &procs::args, &procs::cwd) {
        println!(
            "{:>6}  {:<22} {:>6.1}% cpu  {}",
            a.pid, a.label, a.cpu_pct, a.cwd
        );
    }
}
