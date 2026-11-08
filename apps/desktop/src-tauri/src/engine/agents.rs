//! Terminalde çalışan diğer yapay zekâ araçları (OZELLIKLER.md §1.1, MIMARI.md §3.3): süreç tablosundan bilinen
//! yürütülebilir dosya adlarıyla bulunur. Durum CPU'dan ve çalıştırdığı alt süreçten çıkarılır.
//!
//! - Yalnızca etkileşimli (terminali olan) ve kullanıcıya ait süreçler.
//! - Node/Bun/Python ile çalışan CLI'lar betik adından tanınır.
//! - npm başlatıcısı yalnızca asıl programı bekler: aynı araçtan bir çocuğu olan süreç atlanır.
//! - Açılışta başlatılan uzun ömürlü alt süreçler (MCP sunucuları) "iş" sayılmaz.

use crate::procs::ProcInfo;
use serde::Serialize;
use std::collections::HashMap;

/// (araç, tanıyan adlar)
const KNOWN: &[(&str, &[&str])] = &[
    ("codex", &["codex"]),
    ("gemini", &["gemini"]),
    ("antigravity", &["agy", "antigravity"]),
    ("opencode", &["opencode"]),
    ("aider", &["aider"]),
    ("cursor-agent", &["cursor-agent"]),
    ("copilot", &["copilot", "github-copilot-cli"]),
    ("amp", &["amp"]),
    ("goose", &["goose"]),
    ("qwen", &["qwen"]),
];

const RUNTIMES: &[&str] = &["node", "bun", "deno", "python", "python3", "uv", "npx"];
/// Bu oranın üstünde CPU = çalışıyor.
const BUSY_CPU: f64 = 0.03;
/// Aracın açılışından bu kadar sonra başlayan alt süreç "iş" sayılır (öncekiler MCP sunucusu).
const CHILD_GRACE_SECS: i64 = 30;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProc {
    pub pid: u32,
    pub start_secs: i64,
    pub tool: &'static str,
    pub cwd: String,
    pub busy: bool,
    /// Çalıştırdığı komut (varsa)
    pub child: Option<String>,
}

fn basename(s: &str) -> &str {
    let b = s.rsplit(['/', '\\']).next().unwrap_or(s);
    b.strip_suffix(".js")
        .or_else(|| b.strip_suffix(".mjs"))
        .or_else(|| b.strip_suffix(".exe"))
        .unwrap_or(b)
}

/// Süreç hangi araç? Doğrudan ad ya da çalışma zamanı + betik adı.
pub fn identify(name: &str, args: Option<&[String]>) -> Option<&'static str> {
    let by = |n: &str| {
        KNOWN
            .iter()
            .find(|(_, names)| names.contains(&n))
            .map(|(t, _)| *t)
    };
    if let Some(t) = by(basename(name)) {
        return Some(t);
    }
    if RUNTIMES.contains(&basename(name)) {
        // node /usr/local/bin/gemini …  ya da  node …/node_modules/@google/gemini-cli/dist/index.js
        for a in args?.iter().skip(1).take(3) {
            if a.starts_with('-') {
                continue;
            }
            if let Some(t) = by(basename(a)) {
                return Some(t);
            }
            for (tool, names) in KNOWN {
                if names
                    .iter()
                    .any(|n| a.contains(&format!("/{n}-cli/")) || a.contains(&format!("/{n}/")))
                {
                    return Some(tool);
                }
            }
        }
    }
    None
}

#[derive(Default)]
pub struct Detector {
    prev: HashMap<(u32, i64), (i64, u64)>,
}

impl Detector {
    pub fn scan(
        &mut self,
        now_ms: i64,
        table: &[ProcInfo],
        uid: u32,
        args: &dyn Fn(u32) -> Option<Vec<String>>,
        cwd: &dyn Fn(u32) -> Option<String>,
    ) -> Vec<AgentProc> {
        let mut kids: HashMap<u32, Vec<&ProcInfo>> = HashMap::new();
        for p in table {
            kids.entry(p.ppid).or_default().push(p);
        }
        let rate = |prev: &HashMap<(u32, i64), (i64, u64)>, p: &ProcInfo| -> f64 {
            match prev.get(&(p.pid, p.start_secs)) {
                Some(&(t, c)) if now_ms > t => {
                    p.cpu_ns.saturating_sub(c) as f64 / 1e6 / (now_ms - t) as f64
                }
                _ => 0.0,
            }
        };

        let mut found: Vec<(&ProcInfo, &'static str)> = Vec::new();
        for p in table {
            if p.uid != uid || !p.has_tty {
                continue;
            }
            let quick = identify(&p.name, None);
            let tool = match quick {
                Some(t) => Some(t),
                None if RUNTIMES.contains(&basename(&p.name)) => {
                    identify(&p.name, args(p.pid).as_deref())
                }
                None => None,
            };
            if let Some(t) = tool {
                found.push((p, t));
            }
        }
        // Başlatıcı: aynı araçtan bir çocuğu varsa asıl program o çocuktur.
        let pids: Vec<(u32, &'static str)> = found.iter().map(|(p, t)| (p.pid, *t)).collect();
        found.retain(|(p, t)| {
            !pids
                .iter()
                .any(|(c, ct)| ct == t && table.iter().any(|x| x.pid == *c && x.ppid == p.pid))
        });

        let mut out = Vec::new();
        for (p, tool) in found {
            let own = rate(&self.prev, p);
            let child = kids.get(&p.pid).and_then(|cs| {
                cs.iter()
                    .filter(|c| {
                        c.start_secs > p.start_secs + CHILD_GRACE_SECS
                            && rate(&self.prev, c) >= BUSY_CPU
                    })
                    .max_by_key(|c| c.start_secs)
                    .map(|c| c.name.clone())
            });
            out.push(AgentProc {
                pid: p.pid,
                start_secs: p.start_secs,
                tool,
                cwd: cwd(p.pid).unwrap_or_default(),
                busy: own >= BUSY_CPU || child.is_some(),
                child,
            });
        }
        self.prev = table
            .iter()
            .map(|p| ((p.pid, p.start_secs), (now_ms, p.cpu_ns)))
            .collect();
        out.sort_by_key(|a| a.start_secs);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, ppid: u32, name: &str, tty: bool, start: i64, cpu_ns: u64) -> ProcInfo {
        ProcInfo {
            pid,
            ppid,
            pgid: pid,
            uid: 501,
            has_tty: tty,
            name: name.into(),
            start_secs: start,
            cpu_ns,
        }
    }

    #[test]
    fn identifies_native_and_script_cli() {
        assert_eq!(identify("codex", None), Some("codex"));
        assert_eq!(identify("agy", None), Some("antigravity"));
        assert_eq!(identify("zsh", None), None);
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            identify("node", Some(&a(&["node", "/usr/local/bin/gemini"]))),
            Some("gemini")
        );
        assert_eq!(
            identify(
                "node",
                Some(&a(&[
                    "node",
                    "--no-warnings",
                    "/x/node_modules/@google/gemini-cli/dist/index.js"
                ]))
            ),
            Some("gemini")
        );
        assert_eq!(
            identify("python3", Some(&a(&["python3", "/opt/bin/aider"]))),
            Some("aider")
        );
        assert_eq!(identify("node", Some(&a(&["node", "server.js"]))), None);
    }

    #[test]
    fn busy_from_cpu_or_new_child_and_skips_launchers() {
        let args = |pid: u32| -> Option<Vec<String>> {
            match pid {
                10 => Some(vec![
                    "node".into(),
                    "/usr/lib/node_modules/@openai/codex/bin/codex.js".into(),
                ]),
                30 => Some(vec!["node".into(), "/usr/local/bin/gemini".into()]),
                _ => None,
            }
        };
        let cwd = |pid: u32| Some(format!("/w/{pid}"));
        let t0 = vec![
            p(10, 1, "node", true, 100, 0),     // codex npm başlatıcısı
            p(11, 10, "codex", true, 101, 0),   // asıl program
            p(30, 1, "node", true, 100, 0),     // gemini
            p(31, 30, "node", false, 101, 0),   // MCP sunucusu (açılışta)
            p(40, 1, "aider", false, 100, 0),   // terminali yok: atlanır
            p(50, 1, "opencode", true, 100, 0), // boşta
        ];
        let mut d = Detector::default();
        let first = d.scan(0, &t0, 501, &args, &cwd);
        assert_eq!(
            first.iter().map(|a| (a.pid, a.tool)).collect::<Vec<_>>(),
            [(30, "gemini"), (50, "opencode"), (11, "codex")],
            "açılış zamanına göre"
        );
        assert!(first.iter().all(|a| !a.busy), "ilk örnekte oran yok");

        let s = 1_000_000_000u64;
        let t1 = vec![
            p(10, 1, "node", true, 100, 0),
            p(11, 10, "codex", true, 101, s / 2), // %50 → çalışıyor
            p(30, 1, "node", true, 100, 0),
            p(31, 30, "node", false, 101, s), // MCP yanıyor ama sayılmaz
            p(32, 30, "cargo", false, 200, 0), // yeni komut (henüz CPU yok)
            p(50, 1, "opencode", true, 100, 0),
        ];
        let r = d.scan(1000, &t1, 501, &args, &cwd);
        let get = |pid| r.iter().find(|a| a.pid == pid).unwrap();
        assert!(get(11).busy);
        assert!(!get(30).busy, "MCP sunucusu iş sayılmaz");
        assert!(!get(50).busy);

        let t2 = vec![
            p(11, 10, "codex", true, 101, s / 2),
            p(10, 1, "node", true, 100, 0),
            p(30, 1, "node", true, 100, 0),
            p(31, 30, "node", false, 101, s),
            p(32, 30, "cargo", false, 200, s), // yeni komut yanıyor → gemini çalışıyor
        ];
        let r = d.scan(2000, &t2, 501, &args, &cwd);
        let g = r.iter().find(|a| a.pid == 30).unwrap();
        assert!(g.busy);
        assert_eq!(g.child.as_deref(), Some("cargo"));
        assert_eq!(g.cwd, "/w/30");
    }
}
