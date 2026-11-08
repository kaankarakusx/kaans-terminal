//! Gerçek `$CLAUDE_DIR` üzerinde bir engine turu çalıştırır ve özetini basar.
//! İçerik (prompt, komut) basılmaz; yalnızca sayılar.
//!   cargo run --example engine_dump
use kaans_terminal_lib::engine::{Engine, paths, proc};
use std::time::Instant;

fn main() {
    let root = paths::claude_dir().expect("home yok");
    let mut engine = Engine::new(root, chrono::Local::now().date_naive())
        .with_codex(kaans_terminal_lib::engine::codex::codex_dir());
    let t = Instant::now();
    let (snap, _) = engine.tick(chrono::Local::now(), &proc::is_alive_since);
    println!("indeksleme: {:?}", t.elapsed());
    let t = Instant::now();
    engine.tick(chrono::Local::now(), &proc::is_alive_since);
    println!("ikinci tur: {:?}", t.elapsed());
    for s in &snap.sessions {
        println!(
            "{:<7} {:<9} {:>6} {:<22.22} model={:<18} ctx={:>7} tools={:>4} err={:>3} web={:>3} prompts={:>3} subs={}",
            s.tool,
            format!("{:?}", s.status),
            s.pid.map(|p| p.to_string()).unwrap_or_default(),
            s.project,
            s.model.as_deref().unwrap_or("-"),
            s.context_tokens,
            s.tool_calls,
            s.tool_errors,
            s.web_calls,
            s.prompts,
            s.subagents.len()
        );
    }
    for s in &snap.sessions {
        println!(
            "  {:<22.22} ${:>7.2} ctx%={:?} window={:?} activity={:?}",
            s.project,
            s.cost,
            s.context_pct.map(|p| p.round()),
            s.context_window,
            &s.activity[20..]
        );
    }
    let k = &snap.kpis;
    println!(
        "today=${:.2} burn=${:.2}/h saved=${:.2}",
        k.cost_today, k.burn_per_hour, k.cache_savings_today
    );
    println!(
        "live={} agents={} tools/min={:.1} tok/min={:.0} ok%={:?} cache%={:?} olay={}",
        k.sessions_live,
        k.agents_running,
        k.tools_per_min,
        k.tokens_per_min,
        k.tool_ok_pct.map(|v| v.round()),
        k.cache_hit_pct.map(|v| v.round()),
        engine.recent_events().len()
    );
}
