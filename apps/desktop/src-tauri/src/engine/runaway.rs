//! Kaçak süreç tespiti (MIMARI.md §3.7, OZELLIKLER.md §2.5 "DANGER").
//!
//! Aday: ebeveyni launchd (pid 1; Windows'ta ebeveyni ölmüş), terminali yok, uygulama paketi değil, kullanıcıya ait.
//! 2 dakika boyunca ortalama ≥ %50 çekirdek kullanan aday işaretlenir.
//! Hariç: sistem ve XPC servisleri, canlı bir sohbetin klasöründe başlatılmış süreçler.

use crate::procs::ProcInfo;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};

pub const WINDOW_MS: i64 = 2 * 60_000;
pub const THRESHOLD: f64 = 0.5;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Runaway {
    pub pid: u32,
    pub name: String,
    pub path: Option<String>,
    /// Son 2 dakikanın ortalaması, çekirdek yüzdesi (100 = bir çekirdek)
    pub cpu_pct: f64,
    pub since: i64,
}

#[derive(Default)]
pub struct Detector {
    /// pid → (zaman ms, CPU ns) örnekleri; başlangıç zamanı pid yeniden kullanımını ayırt eder
    samples: HashMap<(u32, i64), VecDeque<(i64, u64)>>,
    flagged_since: HashMap<(u32, i64), i64>,
}

/// Sistem bileşeni mi (yol öneki) ya da uygulama paketi mi.
fn excluded_path(path: &str) -> bool {
    const SYSTEM: [&str; 7] = [
        "/System/",
        "/usr/libexec/",
        "/usr/sbin/",
        "/usr/bin/",
        "/sbin/",
        "/Library/Apple/",
        "/Library/Developer/",
    ];
    SYSTEM.iter().any(|p| path.starts_with(p)) || path.contains(".app/") || path.contains(".xpc/")
}

impl Detector {
    /// `table`: anlık süreç tablosu; `path`/`cwd`: yol sorguları (testte sahte);
    /// `chat_cwds`: canlı sohbetlerin klasörleri.
    pub fn scan(
        &mut self,
        now_ms: i64,
        table: &[ProcInfo],
        uid: u32,
        path: &dyn Fn(u32) -> Option<String>,
        cwd: &dyn Fn(u32) -> Option<String>,
        chat_cwds: &[String],
    ) -> Vec<Runaway> {
        let alive: HashMap<u32, &ProcInfo> = table.iter().map(|p| (p.pid, p)).collect();
        let orphan = |p: &ProcInfo| {
            if cfg!(windows) {
                !alive.contains_key(&p.ppid)
            } else {
                p.ppid == 1
            }
        };
        let mut seen = Vec::new();
        for p in table {
            if p.pid <= 1 || p.uid != uid || p.has_tty || !orphan(p) {
                continue;
            }
            let key = (p.pid, p.start_secs);
            seen.push(key);
            let s = self.samples.entry(key).or_default();
            s.push_back((now_ms, p.cpu_ns));
            while s.len() > 2
                && s.front()
                    .is_some_and(|(t, _)| now_ms - t > WINDOW_MS + 15_000)
            {
                s.pop_front();
            }
        }
        self.samples.retain(|k, _| seen.contains(k));
        self.flagged_since.retain(|k, _| seen.contains(k));

        let mut out = Vec::new();
        for (&(pid, start), s) in &self.samples {
            let (Some(&(t0, c0)), Some(&(t1, c1))) = (s.front(), s.back()) else {
                continue;
            };
            // Ancak 2 dakikalık örnek birikince karar verilir.
            if t1 - t0 < WINDOW_MS {
                continue;
            }
            let cores = (c1.saturating_sub(c0)) as f64 / 1e6 / (t1 - t0) as f64;
            if cores < THRESHOLD {
                self.flagged_since.remove(&(pid, start));
                continue;
            }
            let path = path(pid);
            if path.as_deref().is_some_and(excluded_path) {
                continue;
            }
            if let Some(c) = cwd(pid)
                && chat_cwds.iter().any(|chat| {
                    !chat.is_empty() && (c == *chat || c.starts_with(&format!("{chat}/")))
                })
            {
                continue;
            }
            let since = *self.flagged_since.entry((pid, start)).or_insert(now_ms);
            out.push(Runaway {
                pid,
                name: alive.get(&pid).map(|p| p.name.clone()).unwrap_or_default(),
                path,
                cpu_pct: (cores * 1000.0).round() / 10.0,
                since,
            });
        }
        out.sort_by(|a, b| b.cpu_pct.total_cmp(&a.cpu_pct));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, ppid: u32, uid: u32, tty: bool, cpu_ns: u64) -> ProcInfo {
        ProcInfo {
            pid,
            ppid,
            pgid: pid,
            uid,
            has_tty: tty,
            name: format!("p{pid}"),
            start_secs: 100,
            cpu_ns,
        }
    }

    #[test]
    fn flags_orphan_burning_cpu_after_two_minutes() {
        let mut d = Detector::default();
        let path = |pid: u32| -> Option<String> {
            Some(match pid {
                20 => "/System/Library/thing".into(),
                21 => "/Applications/Foo.app/Contents/MacOS/foo".into(),
                _ => format!("/Users/me/bin/p{pid}"),
            })
        };
        let cwd = |pid: u32| -> Option<String> {
            Some(if pid == 22 {
                "/w/chat/sub".into()
            } else {
                "/tmp".into()
            })
        };
        let chats = vec!["/w/chat".to_string()];
        let sec = 1_000_000_000u64;
        // t=0
        let t0 = vec![
            proc(10, 1, 501, false, 0),  // kaçak: %80
            proc(11, 1, 501, false, 0),  // %20: eşik altı
            proc(12, 50, 501, false, 0), // ebeveyni yaşıyor
            proc(13, 1, 501, true, 0),   // terminali var
            proc(14, 1, 0, false, 0),    // root
            proc(20, 1, 501, false, 0),  // sistem
            proc(21, 1, 501, false, 0),  // uygulama paketi
            proc(22, 1, 501, false, 0),  // sohbetin klasöründe
        ];
        assert!(d.scan(0, &t0, 501, &path, &cwd, &chats).is_empty());
        // 1 dk: henüz karar yok
        let busy = |ms: i64, frac: f64| (frac * (ms as f64 / 1000.0) * sec as f64) as u64;
        let at = |ms: i64| {
            vec![
                proc(10, 1, 501, false, busy(ms, 0.8)),
                proc(11, 1, 501, false, busy(ms, 0.2)),
                proc(12, 50, 501, false, busy(ms, 1.0)),
                proc(13, 1, 501, true, busy(ms, 1.0)),
                proc(14, 1, 0, false, busy(ms, 1.0)),
                proc(20, 1, 501, false, busy(ms, 1.0)),
                proc(21, 1, 501, false, busy(ms, 1.0)),
                proc(22, 1, 501, false, busy(ms, 1.0)),
            ]
        };
        assert!(
            d.scan(60_000, &at(60_000), 501, &path, &cwd, &chats)
                .is_empty()
        );
        let r = d.scan(121_000, &at(121_000), 501, &path, &cwd, &chats);
        assert_eq!(r.len(), 1, "{r:?}");
        assert_eq!(r[0].pid, 10);
        assert!((r[0].cpu_pct - 80.0).abs() < 1.0);
        assert_eq!(r[0].since, 121_000);

        // süreç bitti → listeden düşer
        let gone: Vec<ProcInfo> = at(130_000).into_iter().filter(|p| p.pid != 10).collect();
        assert!(d.scan(130_000, &gone, 501, &path, &cwd, &chats).is_empty());
    }
}
