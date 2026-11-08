//! Gerçek `$CLAUDE_DIR` üzerinde ledger doldurmayı ölçer. Hiçbir şey kaydetmez (yalnızca bellekte).
//!   cargo run --release --example ledger_dump
use kaans_terminal_lib::engine::ledger::{Ledger, RANGES};
use kaans_terminal_lib::engine::{paths, pricing::PriceTable};
use std::time::Instant;

fn main() {
    let root = paths::claude_dir().expect("home yok");
    let today = chrono::Local::now().date_naive();
    let mut ledger = Ledger::load(std::path::Path::new("/nonexistent"));
    let t = Instant::now();
    let stats = ledger.backfill(&root, today, &PriceTable::bundled());
    println!(
        "{} dosya, {} mesaj, {:?}",
        stats.files,
        stats.messages,
        t.elapsed()
    );
    let past = ledger.past_totals(today);
    for (i, (key, _)) in RANGES.iter().enumerate() {
        let cost: f64 = past.ranges[i].values().map(|e| e.cost).sum::<f64>() + 0.0;
        println!(
            "{key:>5} (bugün hariç): ${cost:.2}  satır={}",
            past.ranges[i].len()
        );
    }
}
