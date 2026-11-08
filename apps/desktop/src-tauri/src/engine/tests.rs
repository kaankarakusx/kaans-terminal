//! Engine uçtan uca testleri. Gerçek `~/.claude` kullanılmaz; geçici klasörde sahte bir kök kurulur.

#![allow(clippy::too_many_arguments)] // fixture yardımcıları okunaklı kalsın

use super::*;
use chrono::Duration;
use serde_json::json;
use std::io::Write;
use std::path::Path;

const CWD: &str = "/tmp/demo-app";

struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    now: DateTime<Local>,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        fs::create_dir_all(root.join("sessions")).unwrap();
        fs::create_dir_all(root.join("projects").join(paths::encode_cwd(CWD))).unwrap();
        // Günün ortası: test gece yarısına denk gelse de "bugün" sabit kalsın.
        let now = Local::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .and_then(|d| Local.from_local_datetime(&d).earliest())
            .unwrap();
        Self {
            _tmp: tmp,
            root,
            now,
        }
    }

    fn ts(&self, secs_ago: i64) -> String {
        (self.now - Duration::seconds(secs_ago)).to_rfc3339()
    }

    fn record(&self, pid: u32, v: serde_json::Value) {
        fs::write(
            self.root.join("sessions").join(format!("{pid}.json")),
            serde_json::to_vec(&v).unwrap(),
        )
        .unwrap();
    }

    fn transcript_path(&self, sid: &str) -> PathBuf {
        self.root
            .join("projects")
            .join(paths::encode_cwd(CWD))
            .join(format!("{sid}.jsonl"))
    }

    fn append(&self, path: &Path, lines: &[serde_json::Value]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        for l in lines {
            writeln!(f, "{l}").unwrap();
        }
    }

    fn prompt(&self, uuid: &str, ago: i64, text: &str) -> serde_json::Value {
        json!({"type":"user","uuid":uuid,"timestamp":self.ts(ago),"cwd":CWD,"entrypoint":"cli",
               "message":{"role":"user","content":text}})
    }

    fn tool(
        &self,
        uuid: &str,
        ago: i64,
        msg: &str,
        id: &str,
        name: &str,
        input: serde_json::Value,
        out: u64,
    ) -> serde_json::Value {
        json!({"type":"assistant","uuid":uuid,"timestamp":self.ts(ago),"cwd":CWD,
               "message":{"id":msg,"model":"claude-opus-5-5",
                          "content":[{"type":"tool_use","id":id,"name":name,"input":input}],
                          "usage":{"input_tokens":10,"output_tokens":out,"cache_read_input_tokens":1000,"cache_creation_input_tokens":100}}})
    }

    fn result(&self, uuid: &str, ago: i64, id: &str, err: bool) -> serde_json::Value {
        json!({"type":"user","uuid":uuid,"timestamp":self.ts(ago),
               "message":{"role":"user","content":[{"type":"tool_result","tool_use_id":id,"is_error":err,"content":"x"}]}})
    }
}

fn find<'a>(s: &'a Snapshot, id: &str) -> &'a SessionView {
    s.sessions
        .iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("kart yok: {id}"))
}

#[test]
fn indexes_live_session_and_counts_today() {
    let fx = Fixture::new();
    fx.record(100, json!({"pid":100,"sessionId":"s1","cwd":CWD,"status":"busy","kind":"interactive","updatedAt":1}));
    // .key dosyası okunamaz bir çöp; engine buna dokunursa parse hatası bile vermemeli.
    fs::write(fx.root.join("sessions/100.abc.key"), b"\xff\xfeSECRET").unwrap();

    let p = fx.transcript_path("s1");
    fx.append(
        &p,
        &[
            json!({"type":"ai-title","aiTitle":"Fix login bug","sessionId":"s1"}),
            // dün: sayılmaz
            fx.prompt("u0", 36 * 3600, "eski prompt"),
            fx.prompt("u1", 120, "deploy with password=hunter22"),
            // aynı mesaj iki satıra bölünmüş: kullanım bir kez sayılmalı
            fx.tool(
                "u2",
                110,
                "m1",
                "t1",
                "Bash",
                json!({"command":"cargo test"}),
                40,
            ),
            fx.tool(
                "u3",
                110,
                "m1",
                "t2",
                "WebSearch",
                json!({"query":"tauri events"}),
                50,
            ),
            fx.result("u4", 100, "t1", true),
            fx.result("u5", 100, "t2", false),
            fx.tool(
                "u6",
                90,
                "m2",
                "t3",
                "Edit",
                json!({"file_path":"/tmp/demo-app/src/a.rs"}),
                5,
            ),
        ],
    );

    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    let (snap, events) = eng.tick(fx.now, &|_, _| true);
    assert!(events.is_empty(), "ilk tur (indeksleme) olay yaymaz");

    let s = find(&snap, "s1");
    assert_eq!(s.title, "Fix login bug");
    assert_eq!(s.project, "demo-app");
    assert_eq!(s.status, Status::Busy);
    assert_eq!(s.pid, Some(100));
    assert_eq!(s.prompts, 1, "dünkü prompt sayılmaz");
    assert_eq!(s.tool_calls, 3);
    assert_eq!(s.tool_errors, 1);
    assert_eq!(s.web_calls, 1);
    assert_eq!(s.files_edited, 1);
    assert_eq!(s.tools.get("bash"), Some(&1));
    // m1: max(40,50) çıktı + 10 + 1000 + 100 ; m2: 5 + 1110
    assert_eq!(s.tokens.output, 55);
    assert_eq!(s.tokens.input, 20);
    assert_eq!(s.context_tokens, 1110);
    assert_eq!(s.model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(
        s.last_prompt.as_deref(),
        Some("deploy with password=‹•••›"),
        "prompt maskelenmeli"
    );

    assert_eq!(snap.kpis.sessions_live, 1);
    assert_eq!(snap.kpis.tool_calls, 3);
    assert!((snap.kpis.tool_ok_pct.unwrap() - 50.0).abs() < 1e-9);
    assert!(snap.kpis.tools_per_min > 0.0);

    let ring = eng.recent_events();
    assert!(
        ring.iter()
            .all(|e| e.target.as_deref() != Some("eski prompt"))
    );
    assert!(
        ring.iter()
            .any(|e| e.kind == EventKind::ToolError && e.tool.as_deref() == Some("Bash"))
    );

    // Yeni satır: ikinci tur olay yayar.
    fx.append(
        &p,
        &[fx.tool(
            "u7",
            5,
            "m3",
            "t4",
            "Read",
            json!({"file_path":"/tmp/demo-app/b.rs"}),
            1,
        )],
    );
    let (snap, events) = eng.tick(fx.now, &|_, _| true);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].category, Some("read"));
    assert_eq!(events[0].target.as_deref(), Some("/tmp/demo-app/b.rs"));
    assert_eq!(find(&snap, "s1").files_read, 1);
}

#[test]
fn status_follows_record_and_process_liveness() {
    let fx = Fixture::new();
    fx.record(
        7,
        json!({"pid":7,"sessionId":"w","cwd":CWD,"status":"waiting","waitingFor":"permission"}),
    );
    fx.append(&fx.transcript_path("w"), &[fx.prompt("a", 600, "hi")]);
    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());

    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    let s = find(&snap, "w");
    assert_eq!(s.status, Status::Waiting);
    assert_eq!(s.waiting_for.as_deref(), Some("permission"));

    // Süreç öldü, son kayıt 10 dk önce: ended (ama bugün aktif olduğu için listede).
    let (snap, _) = eng.tick(fx.now, &|_, _| false);
    assert_eq!(find(&snap, "w").status, Status::Ended);
    assert_eq!(snap.kpis.sessions_live, 0);
}

/// End Session: Claude çalışırken süreci kapatılır, kayıt dosyası silinir, transcript birkaç saniye önce yazılmıştır.
/// Oturum 90 sn "working" kalmamalı, hemen bitmeli. Kaydı hiç olmamış oturum ise son etkinliğe göre çalışıyor sayılır.
#[test]
fn ended_right_away_when_a_seen_record_disappears() {
    let fx = Fixture::new();
    fx.record(
        9,
        json!({"pid":9,"sessionId":"k","cwd":CWD,"status":"busy"}),
    );
    fx.append(&fx.transcript_path("k"), &[fx.prompt("a", 5, "build it")]);
    fx.append(
        &fx.transcript_path("n"),
        &[fx.prompt("b", 5, "no registry here")],
    );
    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    assert_eq!(find(&snap, "k").status, Status::Busy);
    assert_eq!(
        find(&snap, "n").status,
        Status::Busy,
        "kaydı hiç olmayan: yakın etkinlik = çalışıyor"
    );

    std::fs::remove_file(fx.root.join("sessions/9.json")).unwrap();
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    assert_eq!(
        find(&snap, "k").status,
        Status::Ended,
        "kaydı kayboldu: hemen bitti"
    );
    assert_eq!(find(&snap, "n").status, Status::Busy);
}

#[test]
fn fork_does_not_double_count_copied_history() {
    let fx = Fixture::new();
    let shared = [
        fx.prompt("p1", 300, "first"),
        fx.tool("x1", 290, "m1", "t1", "Bash", json!({"command":"ls"}), 3),
    ];
    fx.append(&fx.transcript_path("aaa-parent"), &shared);
    let mut fork = shared.to_vec();
    fork.push(fx.prompt("p2", 30, "second"));
    fx.append(&fx.transcript_path("bbb-fork"), &fork);

    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    let total: u32 = snap.sessions.iter().map(|s| s.tool_calls).sum();
    let prompts: u32 = snap.sessions.iter().map(|s| s.prompts).sum();
    assert_eq!(total, 1, "kopyalanan araç çağrısı bir kez sayılır");
    assert_eq!(prompts, 2);
}

#[test]
fn same_pid_new_session_id_continues_same_card() {
    let fx = Fixture::new();
    fx.record(
        55,
        json!({"pid":55,"sessionId":"old","cwd":CWD,"status":"busy","startedAt":1000}),
    );
    fx.append(
        &fx.transcript_path("old"),
        &[fx.tool("o1", 100, "m1", "t1", "Bash", json!({}), 1)],
    );
    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    eng.tick(fx.now, &|_, _| true);

    // compaction: aynı süreç yeni session id ile devam eder
    fx.record(55, json!({"pid":55,"sessionId":"new","cwd":CWD,"status":"idle","startedAt":1000,"updatedAt":2}));
    fx.append(
        &fx.transcript_path("new"),
        &[fx.tool("n1", 10, "m2", "t2", "Grep", json!({}), 1)],
    );
    let (snap, events) = eng.tick(fx.now, &|_, _| true);

    assert_eq!(
        snap.sessions.len(),
        1,
        "yeni kart açılmamalı: {:?}",
        snap.sessions.iter().map(|s| &s.id).collect::<Vec<_>>()
    );
    let s = &snap.sessions[0];
    assert_eq!(s.id, "old");
    assert_eq!(s.tool_calls, 2);
    assert_eq!(s.status, Status::Idle);
    assert_eq!(events[0].session, "old");
}

#[test]
fn running_subagent_keeps_idle_session_busy() {
    let fx = Fixture::new();
    fx.record(
        9,
        json!({"pid":9,"sessionId":"main","cwd":CWD,"status":"idle"}),
    );
    let main = fx.transcript_path("main");
    fx.append(&main, &[fx.prompt("m-1", 400, "research this")]);
    let sub = main.parent().unwrap().join("main/subagents/agent-a1.jsonl");
    fx.append(
        &sub,
        &[fx.tool(
            "s-1",
            20,
            "sm1",
            "st1",
            "WebFetch",
            json!({"url":"https://example.com"}),
            1,
        )],
    );
    fs::write(
        sub.with_extension("meta.json"),
        json!({"agentType":"Explore","description":"find docs","finished":false}).to_string(),
    )
    .unwrap();

    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    let s = find(&snap, "main");
    assert_eq!(s.status, Status::Busy);
    assert_eq!(s.subagents.len(), 1);
    assert_eq!(s.subagents[0].kind, "Explore");
    assert_eq!(s.subagents[0].last_tool.as_deref(), Some("WebFetch"));
    assert_eq!(s.web_calls, 1, "alt ajanın araçları ana karta sayılır");
    assert_eq!(snap.kpis.agents_running, 1);

    fs::write(
        sub.with_extension("meta.json"),
        json!({"agentType":"Explore","description":"find docs","finished":true}).to_string(),
    )
    .unwrap();
    // mtime çözünürlüğü kaba olabilir; meta'yı zorla yeniden okut.
    for t in eng.transcripts.values_mut() {
        if let Some(m) = t.sub.as_mut() {
            m.mtime = None;
        }
    }
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    assert_eq!(find(&snap, "main").status, Status::Idle);
}

#[test]
fn hides_non_interactive_and_sdk_sessions() {
    let fx = Fixture::new();
    fx.record(
        1,
        json!({"pid":1,"sessionId":"spare1","cwd":CWD,"status":"idle","spare":true}),
    );
    fx.append(&fx.transcript_path("spare1"), &[fx.prompt("q1", 10, "x")]);
    let sdk = json!({"type":"user","uuid":"q2","timestamp":fx.ts(10),"entrypoint":"sdk-ts","message":{"content":"y"}});
    fx.append(&fx.transcript_path("sdk1"), &[sdk]);

    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    assert!(
        snap.sessions.is_empty(),
        "{:?}",
        snap.sessions.iter().map(|s| &s.id).collect::<Vec<_>>()
    );
}

#[test]
fn cost_context_and_activity() {
    let fx = Fixture::new();
    fx.record(
        3,
        json!({"pid":3,"sessionId":"c","cwd":CWD,"status":"busy"}),
    );
    let p = fx.transcript_path("c");
    // Opus 5.5: 10 in, 1000 cache read, 100 cache write (5dk), out 1_000_000 → $20 + ufak
    fx.append(
        &p,
        &[
            fx.tool(
                "c1",
                30,
                "m1",
                "t1",
                "Bash",
                json!({"command":"ls"}),
                1_000_000,
            ),
            fx.tool("c2", 5 * 60, "m2", "t2", "Bash", json!({"command":"ls"}), 0),
            // fiyat tablosunda olmayan model
            json!({"type":"assistant","uuid":"c3","timestamp":fx.ts(20),
               "message":{"id":"mx","model":"mystery-model","content":[],
                          "usage":{"input_tokens":5,"output_tokens":5}}}),
        ],
    );
    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    let s = find(&snap, "c");

    let expected = 20.0 + 2.0 * (10.0 * 4.0 + 1000.0 * 0.20 + 100.0 * 4.0 * 1.25) / 1e6;
    assert!(
        (s.cost - expected).abs() < 1e-9,
        "cost {} != {expected}",
        s.cost
    );
    assert!(s.unpriced, "bilinmeyen modelin token'ları işaretlenir");
    assert!((snap.kpis.cost_today - expected).abs() < 1e-9);
    assert!(snap.kpis.burn_per_hour > 20.0);
    assert!(snap.kpis.cache_savings_today > 0.0);

    // son kullanım bilinmeyen modelden: pencere bilinmez (autocompact ayarı da yok)
    assert_eq!(s.model.as_deref(), Some("mystery-model"));

    assert_eq!(s.activity.len(), ACTIVITY_BUCKETS);
    assert_eq!(
        s.activity[ACTIVITY_BUCKETS - 1],
        1,
        "30 sn önceki çağrı son kovada"
    );
    assert_eq!(
        s.activity[ACTIVITY_BUCKETS - 6],
        1,
        "5 dk önceki çağrı 6. kovada"
    );
}

#[test]
fn context_window_is_min_of_model_and_autocompact() {
    let fx = Fixture::new();
    let cwd = fx.root.join("proj");
    let cwd_s = cwd.to_str().unwrap().to_string();
    fs::create_dir_all(cwd.join(".claude")).unwrap();
    fs::write(
        cwd.join(".claude/settings.json"),
        r#"{"autoCompactWindow":"500k"}"#,
    )
    .unwrap();
    fx.record(
        4,
        json!({"pid":4,"sessionId":"w1","cwd":cwd_s,"status":"idle"}),
    );
    let p = fx
        .root
        .join("projects")
        .join(paths::encode_cwd(&cwd_s))
        .join("w1.jsonl");
    fx.append(
        &p,
        &[
            json!({"type":"assistant","uuid":"w-1","timestamp":fx.ts(10),
        "message":{"id":"m","model":"claude-opus-5-5","content":[],
                   "usage":{"input_tokens":0,"output_tokens":1,"cache_read_input_tokens":250000}}}),
        ],
    );

    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    let s = find(&snap, "w1");
    assert_eq!(s.context_window, Some(500_000));
    assert!((s.context_pct.unwrap() - 50.0).abs() < 1e-9);
}

#[test]
fn reused_pid_with_different_start_time_is_not_live() {
    let fx = Fixture::new();
    fx.record(
        77,
        json!({"pid":77,"sessionId":"r","cwd":CWD,"status":"busy",
                         "procStart":"Sun Nov  8 10:24:45 2026"}),
    );
    fx.append(&fx.transcript_path("r"), &[fx.prompt("r1", 600, "x")]);
    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());

    let expected = proc::parse_proc_start("Sun Nov  8 10:24:45 2026");
    let (snap, _) = eng.tick(fx.now, &|_, start| start == expected);
    assert_eq!(
        find(&snap, "r").status,
        Status::Busy,
        "başlangıç zamanı engine'e geçer"
    );
    let (snap, _) = eng.tick(fx.now, &|_, start| start != expected);
    assert_eq!(find(&snap, "r").status, Status::Ended);
}

/// Diskteki sabit fixture (`fixtures/claude/`); biçim gerçek Claude Code dosyalarını taklit eder.
#[test]
fn static_fixture_end_to_end() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/claude");
    let now = DateTime::parse_from_rfc3339("2026-01-15T12:30:00Z")
        .unwrap()
        .with_timezone(&Local);
    let started = proc::parse_proc_start("Thu Jan 15 11:00:00 2026");
    assert_eq!(started, Some(1_768_474_800));

    let mut eng = Engine::new(root, now.date_naive());
    let (snap, _) = eng.tick(now, &|pid, start| pid == 4242 && start == started);
    assert_eq!(snap.sessions.len(), 1);
    let s = &snap.sessions[0];

    assert_eq!(s.id, "fx-main");
    assert_eq!(
        s.title, "Fixture chat",
        "kayıttaki ad ai-title'dan önce gelir"
    );
    assert_eq!(s.project, "fixture-app");
    assert_eq!(
        s.status,
        Status::Waiting,
        "çalışan alt ajan 'needs you' durumunu ezmez"
    );
    assert_eq!(s.waiting_for.as_deref(), Some("permission"));
    assert_eq!(s.prompts, 1, "isMeta satırı prompt sayılmaz");
    assert_eq!(
        s.last_prompt.as_deref(),
        Some("add login with token: ‹•••›")
    );

    // Read, Bash, Edit, Agent (ana) + WebSearch (alt ajan)
    assert_eq!(s.tool_calls, 5);
    assert_eq!(s.tool_errors, 1);
    assert_eq!(s.web_calls, 1);
    assert_eq!((s.files_read, s.files_edited), (1, 1));

    // msg_f1 iki satıra bölünmüş: max alınır (out 60), toplanmaz.
    assert_eq!(s.tokens.input, 10 + 5 + 5 + 200);
    assert_eq!(s.tokens.output, 60 + 40 + 100 + 30);
    assert_eq!(s.tokens.cache_read, 5000 + 7000 + 7100);
    assert_eq!(s.tokens.cache_write, 2000 + 300);
    assert_eq!(s.tokens.cache_write_1h, 2000);

    // Opus 5.5 ($4 in, $20 out, $0.20 okuma, yazma 5dk ×1.25 / 1sa ×2) + Haiku 5.5 ($0.10 / $0.50):
    //   f1: 10·4 + 60·20 + 5000·0.2 + 2000·8   = 18240
    //   f2:  5·4 + 40·20 + 7000·0.2            =  2220
    //   f3:  5·4 + 100·20 + 7100·0.2 + 300·5   =  4940
    //   s1: 200·0.1 + 30·0.5                   =    35
    let expected = (18_240.0 + 2_220.0 + 4_940.0 + 35.0) / 1e6;
    assert!(
        (s.cost - expected).abs() < 1e-12,
        "{} != {expected}",
        s.cost
    );
    assert!(!s.unpriced);

    assert_eq!(
        s.model.as_deref(),
        Some("claude-opus-5-5"),
        "alt ajanın modeli kartın modeli olmaz"
    );
    assert_eq!(s.context_tokens, 5 + 7100 + 300);
    assert_eq!(s.context_window, Some(1_000_000));

    assert_eq!(s.subagents.len(), 1);
    assert_eq!(s.subagents[0].kind, "Explore");
    assert_eq!(s.subagents[0].last_tool.as_deref(), Some("WebSearch"));
    assert_eq!(snap.kpis.agents_running, 1);

    let ring = eng.recent_events();
    let bash = ring
        .iter()
        .find(|e| e.tool.as_deref() == Some("Bash"))
        .unwrap();
    assert_eq!(
        bash.target.as_deref(),
        Some("curl -H 'Authorization: Bearer ‹•••›' localhost"),
        "komutlardaki gizli bilgi maskelenir"
    );
    assert!(
        ring.iter()
            .any(|e| e.kind == EventKind::ToolError && e.id.as_deref() == Some("toolu_f2"))
    );
    assert!(ring.iter().any(|e| e.kind == EventKind::Reply));
}

#[test]
fn same_file_between_two_chats_but_not_own_subagent() {
    let fx = Fixture::new();
    let file = "/tmp/demo-app/src/shared.rs";
    fx.record(
        10,
        json!({"pid":10,"sessionId":"a","cwd":CWD,"status":"busy"}),
    );
    fx.record(
        11,
        json!({"pid":11,"sessionId":"b","cwd":CWD,"status":"busy"}),
    );
    fx.record(
        12,
        json!({"pid":12,"sessionId":"c","cwd":CWD,"status":"busy"}),
    );
    fx.append(
        &fx.transcript_path("a"),
        &[fx.tool("a1", 60, "ma", "ta", "Edit", json!({"file_path": file}), 1)],
    );
    // b aynı dosyaya 10 dk önce yazdı → çakışma
    fx.append(
        &fx.transcript_path("b"),
        &[fx.tool(
            "b1",
            600,
            "mb",
            "tb",
            "Write",
            json!({"file_path": file}),
            1,
        )],
    );
    // c yalnızca okudu → çakışma değil
    fx.append(
        &fx.transcript_path("c"),
        &[fx.tool("c1", 30, "mc", "tc", "Read", json!({"file_path": file}), 1)],
    );
    // a'nın alt ajanı da yazdı → a'nın kendisi sayılır, yeni bir sohbet değil
    let sub = fx
        .transcript_path("a")
        .parent()
        .unwrap()
        .join("a/subagents/agent-z.jsonl");
    fx.append(
        &sub,
        &[fx.tool("z1", 20, "mz", "tz", "Edit", json!({"file_path": file}), 1)],
    );

    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    let (snap, _) = eng.tick(fx.now, &|_, _| true);

    assert_eq!(snap.clashes.len(), 1);
    let mut who = snap.clashes[0].sessions.clone();
    who.sort();
    assert_eq!(who, ["a", "b"]);
    assert_eq!(find(&snap, "a").same_file.len(), 1);
    assert_eq!(find(&snap, "b").same_file.len(), 1);
    assert!(find(&snap, "c").same_file.is_empty());

    let heat = &snap.files[0];
    assert_eq!((heat.reads, heat.writes), (1, 3));
    assert!(heat.clash);
    assert_eq!(heat.sessions.len(), 3);
}

#[test]
fn old_writes_do_not_clash() {
    let fx = Fixture::new();
    let file = "/tmp/demo-app/x.rs";
    fx.record(
        1,
        json!({"pid":1,"sessionId":"a","cwd":CWD,"status":"busy"}),
    );
    fx.record(
        2,
        json!({"pid":2,"sessionId":"b","cwd":CWD,"status":"busy"}),
    );
    fx.append(
        &fx.transcript_path("a"),
        &[fx.tool("a1", 30, "ma", "ta", "Edit", json!({"file_path": file}), 1)],
    );
    fx.append(
        &fx.transcript_path("b"),
        &[fx.tool(
            "b1",
            25 * 60,
            "mb",
            "tb",
            "Edit",
            json!({"file_path": file}),
            1,
        )],
    );
    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    assert!(
        snap.clashes.is_empty(),
        "25 dk önceki yazma pencere dışında"
    );
    assert!(!snap.files[0].clash);
}

#[test]
fn research_feed_tool_mix_and_workers() {
    let fx = Fixture::new();
    fx.record(
        5,
        json!({"pid":5,"sessionId":"r","cwd":CWD,"status":"busy"}),
    );
    let main = fx.transcript_path("r");
    fx.append(
        &main,
        &[
            fx.tool(
                "r1",
                50,
                "m1",
                "t1",
                "WebSearch",
                json!({"query":"tauri events api_key=abc123xyz"}),
                1,
            ),
            fx.tool(
                "r2",
                40,
                "m2",
                "t2",
                "WebFetch",
                json!({"url":"https://v2.tauri.app/"}),
                1,
            ),
            fx.tool(
                "r3",
                30,
                "m3",
                "t3",
                "mcp__claude-in-chrome__navigate",
                json!({"url":"https://x.dev"}),
                1,
            ),
            fx.tool("r4", 20, "m4", "t4", "Bash", json!({"command":"ls"}), 1),
        ],
    );
    let sub = main.parent().unwrap().join("r/subagents/agent-w.jsonl");
    fx.append(
        &sub,
        &[fx.tool("w1", 15, "mw", "tw", "Grep", json!({"pattern":"x"}), 1)],
    );
    fs::write(
        sub.with_extension("meta.json"),
        json!({"agentType":"Explore","description":"scan","finished":false}).to_string(),
    )
    .unwrap();

    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    let (snap, _) = eng.tick(fx.now, &|_, _| true);

    let kinds: Vec<_> = snap.web.iter().map(|w| w.kind).collect();
    assert_eq!(kinds, ["browser", "fetch", "search"], "yeniden eskiye");
    assert_eq!(snap.web[2].target, "tauri events api_key=‹•••›");

    assert_eq!(snap.kpis.tool_mix.get("web"), Some(&2));
    assert_eq!(snap.kpis.tool_mix.get("browser"), Some(&1));
    assert_eq!(snap.kpis.tool_mix.get("search"), Some(&1));

    let w = &find(&snap, "r").subagents[0];
    assert_eq!(
        w.current_tool.as_deref(),
        Some("Grep"),
        "sonucu gelmemiş araç"
    );
    assert_eq!(
        w.started,
        (fx.now - Duration::seconds(15)).timestamp_millis()
    );
}

#[test]
fn ledger_view_merges_past_with_today() {
    let fx = Fixture::new();
    fx.record(
        6,
        json!({"pid":6,"sessionId":"l","cwd":CWD,"status":"busy"}),
    );
    fx.append(
        &fx.transcript_path("l"),
        &[fx.tool("l1", 60, "m1", "t1", "Bash", json!({}), 1_000_000)],
    );
    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());

    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    assert!(!snap.ledger.ready, "geçmiş gelmeden hazır değil");
    let json = serde_json::to_string(&snap.ledger).unwrap();
    assert!(!json.contains("-0.0"), "negatif sıfır sızmamalı: {json}");
    let today = &snap.ledger.ranges[0];
    assert_eq!(today.key, "today");
    assert_eq!(today.rows[0].project, "demo-app");
    assert!((today.cost - snap.kpis.cost_today).abs() < 1e-9);

    let mut past = PastTotals {
        today: Some(fx.now.date_naive()),
        ranges: vec![HashMap::new(); RANGES.len()],
        daily: vec![("x".into(), 3.0)],
    };
    let key = ("demo-app".to_string(), "claude-opus-5-5".to_string());
    for r in past.ranges.iter_mut().skip(1) {
        r.insert(
            key.clone(),
            Entry {
                tokens: Tokens {
                    output: 10,
                    ..Default::default()
                },
                cost: 5.0,
            },
        );
    }
    eng.set_past(past);
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    assert!(snap.ledger.ready);
    let week = &snap.ledger.ranges[1];
    assert_eq!(week.key, "7d");
    assert!((week.cost - (5.0 + snap.kpis.cost_today)).abs() < 1e-9);
    assert_eq!(week.rows.len(), 1, "aynı proje+model birleşir");
    assert_eq!(snap.ledger.daily.len(), 2);
    assert!((snap.ledger.daily[1] - snap.kpis.cost_today).abs() < 1e-9);
}

#[test]
fn keeps_full_last_reply_joining_blocks_of_one_turn() {
    let fx = Fixture::new();
    fx.record(
        8,
        json!({"pid":8,"sessionId":"z","cwd":CWD,"status":"idle"}),
    );
    let reply = |uuid: &str, ago: i64, msg: &str, text: &str| {
        json!({"type":"assistant","uuid":uuid,"timestamp":fx.ts(ago),
        "message":{"id":msg,"model":"claude-opus-5-5","content":[{"type":"text","text":text}],"usage":{"input_tokens":1,"output_tokens":1}}})
    };
    fx.append(
        &fx.transcript_path("z"),
        &[
            reply("z1", 50, "m1", "old answer"),
            fx.tool("z2", 40, "m2", "t1", "Bash", json!({"command":"ls"}), 1),
            reply("z3", 30, "m3", "Part one."),
            reply(
                "z4",
                29,
                "m3",
                "Part two with token=abc123 kept verbatim for the clipboard.",
            ),
        ],
    );
    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive());
    eng.tick(fx.now, &|_, _| true);
    let updates = eng.take_reply_updates();
    assert_eq!(updates.len(), 1);
    assert_eq!(
        updates[0].1,
        "Part one.\n\nPart two with token=abc123 kept verbatim for the clipboard."
    );
    assert!(
        eng.take_reply_updates().is_empty(),
        "değişmeyen cevap tekrar verilmez"
    );
}

#[test]
fn codex_rollout_status_tokens_limits_and_other_agents() {
    use super::agents::AgentProc;
    let fx = Fixture::new();
    let codex = fx.root.join("codex");
    let dir = codex
        .join("sessions")
        .join(fx.now.format("%Y/%m/%d").to_string());
    let roll = dir.join("rollout-2026-x-abc.jsonl");
    let line = |ago: i64, ty: &str, payload: serde_json::Value| json!({"timestamp": fx.ts(ago), "type": ty, "payload": payload});
    fx.append(&roll, &[
        line(300, "session_meta", json!({"id":"abc","cwd":"/w/cx"})),
        line(299, "turn_context", json!({"model":"gpt-5.5","cwd":"/w/cx"})),
        line(298, "event_msg", json!({"type":"task_started","model_context_window":258400})),
        line(297, "response_item", json!({"type":"message","role":"user","content":[{"type":"input_text","text":"refactor the parser please"}]})),
        line(290, "response_item", json!({"type":"function_call","name":"exec_command","call_id":"c1","arguments":"{\"cmd\":\"cargo test\"}"})),
        line(289, "response_item", json!({"type":"function_call_output","call_id":"c1","output":"Exit code: 101"})),
        line(288, "token_usage_record", json!({"response_id":"r1","usage":{"input_tokens":5000,"cached_input_tokens":4000,"output_tokens":300}})),
        line(287, "event_msg", json!({"type":"token_count","info":{"model_context_window":258400,"last_token_usage":{"input_tokens":5000,"output_tokens":300}},
            "rate_limits":{"primary":{"used_percent":20.0,"window_minutes":300,"resets_at":1},"secondary":{"used_percent":55.0,"window_minutes":10080,"resets_at":2}}})),
        line(280, "response_item", json!({"type":"function_call","name":"request_user_input","call_id":"q1","arguments":"{}"})),
    ]);

    let proc_ = |tool: &'static str, pid: u32, cwd: &str, busy: bool| AgentProc {
        pid,
        start_secs: 100,
        tool,
        cwd: cwd.into(),
        busy,
        child: None,
    };
    let mut eng = Engine::new(fx.root.clone(), fx.now.date_naive()).with_codex(Some(codex.clone()));
    eng.set_agents(vec![
        proc_("codex", 70, "/w/cx", false),
        proc_("gemini", 80, "/w/gm", true),
    ]);
    let (snap, _) = eng.tick(fx.now, &|_, _| true);

    let c = find(&snap, "codex:rollout-2026-x-abc");
    assert_eq!(c.tool, "codex");
    assert_eq!(c.title, "refactor the parser please");
    assert_eq!(c.status, Status::Waiting, "tur açık ve soru soruyor");
    assert_eq!(c.waiting_for.as_deref(), Some("question"));
    assert_eq!(c.pid, Some(70));
    assert_eq!(c.model.as_deref(), Some("gpt-5.5"));
    assert_eq!((c.context_tokens, c.context_window), (5300, Some(258_400)));
    assert_eq!(
        (c.tokens.input, c.tokens.cache_read, c.tokens.output),
        (1000, 4000, 300)
    );
    assert_eq!(c.cost, 0.0);
    assert!(
        !c.unpriced,
        "Codex fiyatlandırılmaz ama 'eksik fiyat' de sayılmaz"
    );
    assert_eq!((c.tool_calls, c.tool_errors), (2, 1));
    assert_eq!(c.tools.get("bash"), Some(&1));

    let l = snap.limits.iter().find(|l| l.tool == "codex").unwrap();
    assert!((l.five_hour.unwrap().used_pct - 20.0).abs() < 1e-9);
    assert!((l.weekly.unwrap().used_pct - 55.0).abs() < 1e-9);

    let g = snap.sessions.iter().find(|s| s.tool == "gemini").unwrap();
    assert_eq!(
        (g.status, g.pid, g.project.as_str()),
        (Status::Busy, Some(80), "gm")
    );
    assert!(
        !snap
            .sessions
            .iter()
            .any(|s| s.tool == "codex" && s.id.starts_with("ai:")),
        "rollout'u olan codex süreci ayrı kart olmaz"
    );

    // Tur bitti → sıra sende
    fx.append(
        &roll,
        &[line(10, "event_msg", json!({"type":"task_complete"}))],
    );
    let (snap, _) = eng.tick(fx.now, &|_, _| true);
    assert_eq!(find(&snap, "codex:rollout-2026-x-abc").status, Status::Idle);

    // Süreç kapandı ve 90 sn'dir yazmıyor → bitti
    eng.set_agents(vec![]);
    let (snap, _) = eng.tick(fx.now + Duration::seconds(120), &|_, _| true);
    assert_eq!(
        find(&snap, "codex:rollout-2026-x-abc").status,
        Status::Ended
    );
}
