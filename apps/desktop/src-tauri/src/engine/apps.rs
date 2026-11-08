//! Çalışan uygulama komutları (dev sunucuları vb.): orbit.map'te araba olarak gezer.
//! Süreç tablosundan bilinen komutlar tanınır (`npm run dev`, `vite`, `next dev`, `cargo run`, `uvicorn`, …). Zincirde
//! (npm → sh → node vite) yalnızca en dıştaki gösterilir; CPU, bütün alt süreçlerin toplamıdır (tek çekirdek = %100).
//! Süreç bilgisi kernel'den okunur; harici komut çalıştırılmaz.
use crate::procs::ProcInfo;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppProc {
    pub pid: u32,
    pub start_secs: i64,
    /// Görünen komut: `npm run dev`, `vite`, `cargo run`, …
    pub label: String,
    pub cwd: String,
    /// Alt süreçlerle birlikte CPU (%; bir çekirdek = 100)
    pub cpu_pct: f64,
}

/// Paket yöneticisi betikleri: bunlar "uygulama çalıştırma" sayılır
const SCRIPTS: &[&str] = &[
    "dev",
    "start",
    "serve",
    "preview",
    "watch",
    "storybook",
    "tauri",
];

/// Argüman istenecek adaylar (yalnızca bunlar için argv okunur)
pub fn is_candidate(name: &str) -> bool {
    // npm süreç adını da yeniden yazar ("npm run tauri de", 16 karakterde kesik): ilk kelimeye bakılır
    let n = base(name.split_whitespace().next().unwrap_or(name));
    matches!(
        n,
        "npm"
            | "pnpm"
            | "yarn"
            | "bun"
            | "node"
            | "deno"
            | "cargo"
            | "go"
            | "air"
            | "uvicorn"
            | "gunicorn"
            | "flask"
            | "rails"
            | "puma"
            | "ruby"
            | "php"
            | "hugo"
            | "next-server"
            | "vite"
    ) || n.starts_with("python")
}

fn base(s: &str) -> &str {
    s.rsplit(['/', '\\'])
        .next()
        .unwrap_or(s)
        .trim_end_matches(".exe")
}

/// Komut uygulama çalıştırıyorsa görünen etiketi
pub fn classify(name: &str, args: &[String]) -> Option<String> {
    // npm/pnpm süreç başlığını yeniden yazar ("npm run dev"): argv tek parça metin + boş parçalar olarak gelir
    let nonempty: Vec<&String> = args.iter().filter(|a| !a.is_empty()).collect();
    let split: Vec<String>;
    let args = if nonempty.len() == 1 && nonempty[0].contains(' ') {
        split = nonempty[0].split_whitespace().map(String::from).collect();
        &split[..]
    } else {
        args
    };
    // Başlıktaki ilk kelime bir paket yöneticisiyse süreç adı ("node") yerine o
    let first = args.first().map(|a| base(a));
    let n = match first {
        Some(f @ ("npm" | "pnpm" | "yarn" | "bun")) => f,
        _ => base(name),
    };
    // argv[0] yerine sürecin adını kullan; geri kalan argümanlar
    let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
    let has = |w: &str| rest.iter().any(|a| base(a) == w || *a == w);
    match n {
        "npm" | "pnpm" | "yarn" | "bun" => {
            // npm run dev · pnpm dev · yarn start · bun run dev
            let script = match rest.first() {
                Some(&"run") | Some(&"run-script") => rest.get(1).copied(),
                other => other.copied(),
            }?;
            let s = script.split(':').next().unwrap_or(script);
            SCRIPTS.contains(&s).then(|| {
                if rest.first() == Some(&"run") {
                    format!("{n} run {script}")
                } else {
                    format!("{n} {script}")
                }
            })
        }
        "node" | "deno" => {
            // Paket yöneticisi node betiği olarak çalışır: node …/npm-cli.js run dev
            if let Some(first) = rest.first() {
                let b = base(first);
                let pm = match b.split('.').next().unwrap_or(b) {
                    "npm-cli" | "npm" => Some("npm"),
                    "pnpm" => Some("pnpm"),
                    "yarn" => Some("yarn"),
                    _ => None,
                };
                if let Some(pm) = pm {
                    let mut argv = vec![pm.to_string()];
                    argv.extend(rest[1..].iter().map(|a| a.to_string()));
                    return classify(pm, &argv);
                }
            }
            const TOOLS: &[(&str, &str)] = &[
                ("tauri", "tauri dev"),
                ("vite", "vite"),
                ("next", "next dev"),
                ("nuxt", "nuxt dev"),
                ("astro", "astro dev"),
                ("webpack", "webpack serve"),
                ("nodemon", "nodemon"),
                ("remix", "remix dev"),
                ("parcel", "parcel"),
                ("react-scripts", "react-scripts start"),
                ("expo", "expo start"),
                ("svelte-kit", "svelte-kit dev"),
                ("tsx", "tsx watch"),
            ];
            for a in &rest {
                let b = base(a);
                let b = b
                    .trim_end_matches(".js")
                    .trim_end_matches(".mjs")
                    .trim_end_matches(".cjs");
                if let Some((_, label)) = TOOLS.iter().find(|(t, _)| b == *t) {
                    // tsx yalnızca izleme kipinde, tauri yalnızca dev kipinde uygulama sayılır
                    if (*label == "tsx watch" && !has("watch"))
                        || (*label == "tauri dev" && !has("dev"))
                    {
                        continue;
                    }
                    return Some((*label).to_string());
                }
            }
            if n == "deno" && rest.first() == Some(&"task") {
                return rest.get(1).map(|t| format!("deno task {t}"));
            }
            None
        }
        "vite" => Some("vite".into()),
        "next-server" => Some("next dev".into()),
        "cargo" => match rest.first().copied() {
            Some("run") => Some("cargo run".into()),
            Some("watch") => Some("cargo watch".into()),
            Some("tauri") if has("dev") => Some("cargo tauri dev".into()),
            _ => None,
        },
        "go" => (rest.first() == Some(&"run")).then(|| "go run".into()),
        "air" => Some("air".into()),
        "uvicorn" | "gunicorn" | "puma" | "hugo" => Some(n.to_string()),
        "flask" => has("run").then(|| "flask run".into()),
        "rails" => (has("server") || has("s")).then(|| "rails server".into()),
        "php" => (has("artisan") && has("serve")).then(|| "artisan serve".into()),
        "ruby" => (has("rails") && (has("server") || has("s"))).then(|| "rails server".into()),
        _ if n.starts_with("python") => {
            if has("runserver") {
                Some("manage.py runserver".into())
            } else if let Some(i) = rest.iter().position(|a| *a == "-m") {
                match rest.get(i + 1).copied() {
                    Some(m @ ("uvicorn" | "gunicorn" | "flask" | "streamlit" | "http.server")) => {
                        Some(m.to_string())
                    }
                    _ => None,
                }
            } else if has("streamlit") {
                Some("streamlit".into())
            } else {
                None
            }
        }
        _ => None,
    }
}

#[derive(Default)]
pub struct Detector {
    /// (pid, başlangıç) → (zaman ms, toplam CPU ns)
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
    ) -> Vec<AppProc> {
        let mut found: Vec<(&ProcInfo, String)> = Vec::new();
        for p in table {
            if p.uid != uid || !is_candidate(&p.name) {
                continue;
            }
            let Some(argv) = args(p.pid) else { continue };
            if let Some(label) = classify(&p.name, &argv) {
                found.push((p, label));
            }
        }
        // Zincirde yalnızca en dıştaki: atalarından biri de bulunduysa atlanır
        let parent: HashMap<u32, u32> = table.iter().map(|p| (p.pid, p.ppid)).collect();
        let matched: HashSet<u32> = found.iter().map(|(p, _)| p.pid).collect();
        found.retain(|(p, _)| {
            let mut cur = p.ppid;
            for _ in 0..32 {
                if matched.contains(&cur) {
                    return false;
                }
                match parent.get(&cur) {
                    Some(&pp) if pp != cur && cur > 1 => cur = pp,
                    _ => break,
                }
            }
            true
        });

        let by_pid: HashMap<u32, &ProcInfo> = table.iter().map(|p| (p.pid, p)).collect();
        let mut out = Vec::new();
        let mut next_prev = HashMap::new();
        for (p, label) in found {
            // Alt süreçlerle toplam CPU zamanı
            let mut total = p.cpu_ns;
            for d in crate::procs::descendants(p.pid, table) {
                if let Some(c) = by_pid.get(&d) {
                    total += c.cpu_ns;
                }
            }
            let key = (p.pid, p.start_secs);
            let cpu_pct = match self.prev.get(&key) {
                Some(&(t, c)) if now_ms > t => {
                    let pct = total.saturating_sub(c) as f64 / 1e6 / (now_ms - t) as f64 * 100.0;
                    (pct * 10.0).round() / 10.0
                }
                _ => 0.0,
            };
            next_prev.insert(key, (now_ms, total));
            out.push(AppProc {
                pid: p.pid,
                start_secs: p.start_secs,
                label,
                cwd: cwd(p.pid).unwrap_or_default(),
                cpu_pct,
            });
        }
        self.prev = next_prev;
        out.sort_by_key(|a| a.start_secs);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Vec<String> {
        s.split(' ').map(String::from).collect()
    }

    #[test]
    fn recognises_common_dev_commands() {
        let cases = [
            ("npm", "npm run dev", Some("npm run dev")),
            ("npm", "npm run test", None),
            ("pnpm", "pnpm dev", Some("pnpm dev")),
            ("yarn", "yarn start", Some("yarn start")),
            ("bun", "bun run dev:web", Some("bun run dev:web")),
            ("node", "node /p/node_modules/.bin/vite", Some("vite")),
            (
                "node",
                "node /p/node_modules/next/dist/bin/next dev",
                Some("next dev"),
            ),
            ("node", "node /p/node_modules/.bin/tsx src/a.ts", None),
            (
                "node",
                "node /p/node_modules/.bin/tsx watch src/a.ts",
                Some("tsx watch"),
            ),
            ("node", "node server.js", None),
            ("node", "npm run tauri dev", Some("npm run tauri")),
            (
                "node",
                "node /usr/lib/node_modules/npm/bin/npm-cli.js run tauri -- dev",
                Some("npm run tauri"),
            ),
            ("node", "node /opt/pnpm/bin/pnpm.cjs dev", Some("pnpm dev")),
            (
                "node",
                "node /p/node_modules/.bin/tauri dev",
                Some("tauri dev"),
            ),
            ("node", "node /p/node_modules/.bin/tauri build", None),
            ("cargo", "cargo run --release", Some("cargo run")),
            ("cargo", "cargo test", None),
            (
                "python3.12",
                "python3 manage.py runserver",
                Some("manage.py runserver"),
            ),
            ("python3", "python3 -m uvicorn app:app", Some("uvicorn")),
            ("uvicorn", "uvicorn app:app --reload", Some("uvicorn")),
            ("go", "go run .", Some("go run")),
            ("rails", "rails s", Some("rails server")),
            ("git", "git status", None),
        ];
        for (name, args, want) in cases {
            assert_eq!(classify(name, &v(args)).as_deref(), want, "{args}");
        }
    }

    fn p(pid: u32, ppid: u32, name: &str, start: i64, cpu_ns: u64) -> ProcInfo {
        ProcInfo {
            pid,
            ppid,
            pgid: pid,
            uid: 501,
            has_tty: true,
            name: name.into(),
            start_secs: start,
            cpu_ns,
        }
    }

    #[test]
    fn rewritten_process_title_from_the_kernel() {
        // Gerçek veri: ad "node", argv = ["npm run tauri dev", "", "", …]
        let argv: Vec<String> = ["npm run tauri dev", "", "", "", "", ""]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(classify("node", &argv).as_deref(), Some("npm run tauri"));
    }

    #[test]
    fn rewritten_process_names_are_candidates() {
        assert!(is_candidate("npm run tauri de"));
        assert!(is_candidate("/usr/local/bin/node"));
        assert!(is_candidate("python3.12"));
        assert!(!is_candidate("zsh"));
    }

    #[test]
    fn keeps_the_outermost_of_a_chain_and_sums_cpu() {
        // zsh → npm run dev → sh → node vite → esbuild
        let argv = |pid: u32| -> Option<Vec<String>> {
            Some(match pid {
                10 => v("npm run dev"),
                12 => v("node /p/node_modules/.bin/vite"),
                20 => v("cargo run"),
                _ => v("x"),
            })
        };
        let cwd = |_| Some("/Users/me/Desktop/demo-app".to_string());
        let t0 = vec![
            p(1, 0, "zsh", 0, 0),
            p(10, 1, "npm", 1, 1_000_000_000),
            p(11, 10, "sh", 1, 0),
            p(12, 11, "node", 2, 2_000_000_000),
            p(13, 12, "esbuild", 2, 0),
            p(20, 1, "cargo", 5, 0),
        ];
        let mut d = Detector::default();
        let first = d.scan(0, &t0, 501, &argv, &cwd);
        assert_eq!(
            first.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(),
            ["npm run dev", "cargo run"]
        );
        assert_eq!(first[0].cpu_pct, 0.0, "ilk ölçümde oran yok");
        // 1 sn sonra: node 0,3 sn, esbuild 0,2 sn CPU → %50 (alt süreçler dahil)
        let mut t1 = t0.clone();
        t1[3].cpu_ns += 300_000_000;
        t1[4].cpu_ns += 200_000_000;
        let next = d.scan(1000, &t1, 501, &argv, &cwd);
        assert_eq!(next[0].cpu_pct, 50.0);
        assert_eq!(next[0].cwd, "/Users/me/Desktop/demo-app");
        // başka kullanıcının süreci sayılmaz
        let mut other = t1.clone();
        other[1].uid = 0;
        other[3].uid = 0;
        assert_eq!(d.scan(2000, &other, 501, &argv, &cwd).len(), 1);
    }
}
