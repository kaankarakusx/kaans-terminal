//! Gerçek Codex rollout'larını verilen günde okur (ayrıştırıcıyı gerçek veride sınamak için). Yalnızca sayılar.
//!   cargo run --example codex_dump -- 2026-10-10
use chrono::{Local, NaiveDate, TimeZone};
use kaans_terminal_lib::engine::{Engine, codex, paths};

fn main() {
    let day: NaiveDate = std::env::args()
        .nth(1)
        .expect("tarih")
        .parse()
        .expect("YYYY-MM-DD");
    let now = Local
        .from_local_datetime(&day.and_hms_opt(23, 59, 0).unwrap())
        .earliest()
        .unwrap();
    let mut e = Engine::new(paths::claude_dir().unwrap().join("__yok__"), day)
        .with_codex(codex::codex_dir());
    let (snap, _) = e.tick(now, &|_, _| false);
    for s in snap.sessions.iter().filter(|s| s.tool == "codex") {
        println!(
            "{:?} title={}ch model={:?} ctx={}/{:?} tokens(in={} cache={} out={}) tools={} err={} prompts={} edited={} cats={:?}",
            s.status,
            s.title.chars().count(),
            s.model,
            s.context_tokens,
            s.context_window,
            s.tokens.input,
            s.tokens.cache_read,
            s.tokens.output,
            s.tool_calls,
            s.tool_errors,
            s.prompts,
            s.files_edited,
            s.tools
        );
    }
    for l in &snap.limits {
        println!(
            "limits {} 5h={:?} week={:?}",
            l.tool,
            l.five_hour.map(|w| w.used_pct),
            l.weekly.map(|w| w.used_pct)
        );
    }
    println!("olay={} web={}", e.recent_events().len(), snap.web.len());
}
