//! Engine: `~/.claude` (ileride `~/.codex`, `~/.gemini`) ve süreç tablosunu **salt okunur** okuyan katman.
//! Ayrıntı: docs/MIMARI.md §3.
//!
//! Her `tick()`:
//! 1. kayıtlar (`sessions/<pid>.json`) taranır ve kartlara bağlanır,
//! 2. bugünün transcript'leri ve alt ajan dosyaları keşfedilir,
//! 3. transcript'lerin yeni satırları okunup sayaçlara işlenir,
//! 4. arayüz için bir anlık görüntü (`Snapshot`) ve yeni olaylar döner.

pub mod agents;
pub mod apps;
pub mod codex;
pub mod ledger;
pub mod paths;
pub mod pricing;
pub mod proc;
pub mod redact;
pub mod registry;
pub mod runaway;
pub mod settings;
pub mod status;
pub mod tools;
pub mod transcript;

use crate::host::client::HostState;
use crate::host::protocol::Leftover;
use agents::AgentProc;
use chrono::{DateTime, Local, NaiveDate, TimeZone};
use ledger::{Entry, PastTotals, RANGES};
use pricing::PriceTable;
use redact::clean;
use registry::{Registry, SessionRecord};
use runaway::Runaway;
use serde::Serialize;

/// Uygulamanın kendi başlattığı yardımcı `claude -p` süreçleri (AI commit mesajı): kayıt dosyası yazarlar ama
/// sohbet değildirler, listede görünmezler.
static OWN_HELPERS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<u32>>> =
    std::sync::LazyLock::new(Default::default);

pub fn mark_own_helper(pid: u32, on: bool) {
    if let Ok(mut set) = OWN_HELPERS.lock() {
        if on {
            set.insert(pid);
        } else {
            set.remove(&pid);
        }
    }
}

fn is_own_helper(pid: u32) -> bool {
    OWN_HELPERS.lock().is_ok_and(|s| s.contains(&pid))
}
use serde_json::Value;
use status::{Status, StatusInput, base_status, status_of, subagent_running};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use transcript::{Item, LimitWindow, RateLimits, Tail, Tokens, parse_line};

/// Arayüzün ilk yüklemede alacağı son olay sayısı (ticker 200 kare gösterir).
pub const RING_SIZE: usize = 200;
/// tools/min ve tokens/min için pencere.
const RATE_WINDOW_MS: i64 = 5 * 60_000;
/// Yeni transcript ve alt ajan dosyası arama sıklığı.
const DIR_SCAN_MS: i64 = 5_000;
/// Saatlik yanma hızı ($/h) için pencere.
const BURN_WINDOW_MS: i64 = 60 * 60_000;
/// Kart etkinlik çizgisi: son 30 dakika, dakikada bir kova.
pub const ACTIVITY_BUCKETS: usize = 30;
const ACTIVITY_WINDOW_MS: i64 = ACTIVITY_BUCKETS as i64 * 60_000;
/// İki farklı sohbet bu süre içinde aynı dosyaya yazarsa "same file".
pub const SAME_FILE_WINDOW_MS: i64 = 20 * 60_000;
const WEB_FEED_SIZE: usize = 60;
const FILES_HEAT_SIZE: usize = 40;
const LEDGER_ROWS: usize = 15;
const PROMPT_MAX: usize = 160;
const TARGET_MAX: usize = 160;

// ───────────────────────────── arayüze giden tipler ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    Prompt,
    Tool,
    ToolError,
    Reply,
}

/// Tekil olay (session.log, ticker). Bütün metinler maskelenmiştir.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineEvent {
    pub ts: i64,
    pub session: String,
    pub kind: EventKind,
    pub tool: Option<String>,
    pub category: Option<&'static str>,
    pub target: Option<String>,
    /// Araç çağrısı kimliği; `toolError` olayı ilgili `tool` olayını bununla bulur.
    pub id: Option<String>,
    pub subagent: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentView {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub running: bool,
    pub last_tool: Option<String>,
    /// Sonucu beklenen araç çağrısı (şu an ne yapıyor).
    pub current_tool: Option<String>,
    /// İlk kaydının zamanı (süre için).
    pub started: i64,
}

/// Uygulamanın kendi terminali (oturum sunucusundaki bir PTY).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalView {
    pub id: String,
    pub cwd: String,
    pub project: String,
    /// Ön plandaki komut (boştaysa kabuğun adı)
    pub title: String,
    /// Kabuktan başka bir şey çalışıyor
    pub busy: bool,
    /// İçinde Claude çalışıyorsa sohbet kartının kimliği (o zaman "sohbet" sayılır)
    pub chat: Option<String>,
    pub started: i64,
    /// Açık bir pencerede gösteriliyor mu
    pub attached: bool,
}

/// Bir aracın 5 saatlik ve haftalık kullanım limitleri.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolLimits {
    pub tool: String,
    pub five_hour: Option<LimitWindow>,
    pub weekly: Option<LimitWindow>,
    /// Bilginin zamanı (ms)
    pub updated: i64,
}

impl ToolLimits {
    /// Pencere süresine göre 5 saatlik / haftalık ayrımı.
    pub fn from_windows(tool: &str, l: &RateLimits, updated: i64) -> Self {
        let mut out = Self {
            tool: tool.into(),
            five_hour: None,
            weekly: None,
            updated,
        };
        for w in [l.primary, l.secondary].into_iter().flatten() {
            if w.window_minutes >= 24 * 60 {
                out.weekly = Some(w);
            } else {
                out.five_hour = Some(w);
            }
        }
        out
    }
}

/// research.feed satırı: web araması ya da getirilen sayfa.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebItem {
    pub ts: i64,
    pub session: String,
    /// "search" | "fetch" | "browser"
    pub kind: &'static str,
    pub target: String,
    pub subagent: bool,
}

/// files.heat satırı.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHeat {
    /// Ev klasörü `~` ile kısaltılmış yol.
    pub path: String,
    pub reads: u32,
    pub writes: u32,
    pub last: i64,
    pub sessions: Vec<String>,
    /// Son 20 dakikada iki farklı sohbet bu dosyaya yazdı.
    pub clash: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clash {
    pub path: String,
    pub sessions: Vec<String>,
    pub last: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerRow {
    pub project: String,
    pub model: String,
    pub tokens: u64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerRange {
    /// "today" | "7d" | "30d" | "12w" | "12m"
    pub key: &'static str,
    pub cost: f64,
    pub tokens: u64,
    /// Maliyete göre ilk satırlar.
    pub rows: Vec<LedgerRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerView {
    /// Geçmiş günler henüz okunmadıysa false (yalnızca "today" doğrudur).
    pub ready: bool,
    pub ranges: Vec<LedgerRange>,
    /// Son 30 günün günlük maliyeti, eskiden yeniye; son eleman bugün.
    pub daily: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    /// "claude" | "codex" | "gemini" | "antigravity" | …
    pub tool: String,
    pub pid: Option<u32>,
    pub title: String,
    pub project: String,
    pub cwd: String,
    pub status: Status,
    pub waiting_for: Option<String>,
    pub model: Option<String>,
    pub context_tokens: u64,
    /// Model penceresi ile `/autocompact` penceresinin küçüğü; model bilinmiyorsa `None`.
    pub context_window: Option<u64>,
    pub context_pct: Option<f64>,
    /// Bugünkü token'lar.
    pub tokens: Tokens,
    /// Bugünkü api-eq maliyeti ($).
    pub cost: f64,
    /// Fiyat tablosunda olmayan bir modelin token'ları varsa (maliyet eksik).
    pub unpriced: bool,
    /// Son 30 dakikanın dakika başına araç çağrısı sayısı; son eleman şimdiki dakika.
    pub activity: Vec<u16>,
    /// Başka bir sohbetle son 20 dakikada aynı dosyaya yazılan yollar.
    pub same_file: Vec<String>,
    /// Bu sohbet uygulamanın bir terminalinde çalışıyorsa o terminal.
    pub terminal: Option<String>,
    pub tools: BTreeMap<&'static str, u32>,
    pub tool_calls: u32,
    pub tool_errors: u32,
    pub web_calls: u32,
    pub prompts: u32,
    pub files_edited: usize,
    pub files_read: usize,
    pub last_prompt: Option<String>,
    pub last_tool: Option<String>,
    pub last_activity: i64,
    /// `--hue0` … `--hue9`
    pub hue: u8,
    /// Effort seviyesi (Claude: son assistant satırı)
    pub effort: Option<String>,
    /// Son prompt'un zamanı: çalışırken turun ne kadardır sürdüğü buradan sayılır
    pub turn_started: i64,
    pub subagents: Vec<SubagentView>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Kpis {
    pub sessions_live: usize,
    pub agents_running: usize,
    pub tools_per_min: f64,
    pub tokens_per_min: f64,
    pub web_calls: u32,
    pub tool_calls: u32,
    pub tool_errors: u32,
    /// Hiç sonuç yoksa `None`.
    pub tool_ok_pct: Option<f64>,
    pub cache_hit_pct: Option<f64>,
    pub tokens_today: Tokens,
    /// Bugünkü api-eq maliyeti ($).
    pub cost_today: f64,
    /// Son 60 dakikada harcanan ($/h).
    pub burn_per_hour: f64,
    /// Önbellek okumalarının normal girdi fiyatına göre kazandırdığı ($).
    pub cache_savings_today: f64,
    /// Bugünkü araç çağrıları kategoriye göre (tool.mix).
    pub tool_mix: BTreeMap<&'static str, u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub generated_at: i64,
    pub day: String,
    pub sessions: Vec<SessionView>,
    pub kpis: Kpis,
    /// research.feed: yeniden eskiye
    pub web: Vec<WebItem>,
    /// files.heat: en sıcaktan
    pub files: Vec<FileHeat>,
    pub clashes: Vec<Clash>,
    pub ledger: LedgerView,
    pub terminals: Vec<TerminalView>,
    /// Araçların hesap limitleri (Claude: statusLine, Codex: rollout)
    pub limits: Vec<ToolLimits>,
    /// Kapanan terminallerden kalan süreçler (amber araba)
    pub leftovers: Vec<Leftover>,
    /// Kaçak süreçler (DANGER)
    pub runaways: Vec<Runaway>,
    /// Çalışan uygulama komutları (dev sunucuları): orbit.map'te araba
    pub apps: Vec<AppView>,
}

/// Çalışan uygulama komutu (`npm run dev`, `vite`, `cargo run`…)
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppView {
    pub pid: u32,
    pub label: String,
    pub project: String,
    pub cwd: String,
    pub cpu_pct: f64,
    pub started: i64,
    /// Uygulamanın bir terminalinde çalışıyorsa o terminal (tıklayınca açılır)
    pub terminal: Option<String>,
}

// ───────────────────────────── iç durum ─────────────────────────────

#[derive(Default)]
struct DayStats {
    prompts: u32,
    tools: BTreeMap<&'static str, u32>,
    tool_calls: u32,
    tool_results: u32,
    tool_errors: u32,
    web_calls: u32,
    files_edited: HashSet<String>,
    files_read: HashSet<String>,
    tokens: Tokens,
    cost: f64,
    savings: f64,
    unpriced: bool,
    /// model → bugünkü toplam (ledger'ın "today" satırları)
    by_model: BTreeMap<String, Entry>,
}

/// Arayüzdeki bir sohbet kartı. Compaction ile session id değişse de aynı kart devam eder.
struct Card {
    hue: u8,
    cwd: String,
    name: Option<String>,
    title: Option<String>,
    entrypoint: Option<String>,
    /// Kayıt "kullanıcı sohbeti değil" diyorsa (sdk, spare, host'lu) gizlenir.
    hidden: bool,
    model: Option<String>,
    context_tokens: u64,
    stats: DayStats,
    last_prompt: Option<String>,
    last_tool: Option<String>,
    last_activity_ms: i64,
    /// Son 30 dakikadaki araç çağrılarının zamanları (etkinlik çizgisi).
    activity: VecDeque<i64>,
    /// Son cevabın tamamı (⇧⌘C "copy last answer"); bir turdaki ardışık metin blokları birleşir.
    last_reply: String,
    reply_open: bool,
    effort: Option<String>,
    turn_started_ms: i64,
    /// "claude" | "codex"
    tool: &'static str,
    /// Codex: tur açık (task_started … task_complete)
    turn_open: bool,
    /// Codex: modelin bağlam penceresi (rollout'tan)
    window: Option<u64>,
    /// Codex: son hesap limitleri (zaman, değerler)
    limits: Option<(i64, RateLimits)>,
}

struct SubMeta {
    meta_path: PathBuf,
    mtime: Option<SystemTime>,
    finished: bool,
    kind: String,
    description: String,
    last_tool: Option<String>,
}

struct Transcript {
    tail: Tail,
    card: String,
    /// Alt ajan transcript'iyse meta bilgisi.
    sub: Option<SubMeta>,
    /// Sonucu henüz gelmemiş araç çağrıları: id → ad.
    pending: HashMap<String, String>,
    /// Aynı mesaj birden çok satıra bölünür; kullanım mesaj başına bir kez sayılır.
    usage_seen: HashMap<String, Tokens>,
    first_ts_ms: i64,
    last_ts_ms: i64,
    /// Codex rollout'u (farklı ayrıştırıcı)
    codex: bool,
}

#[derive(Default)]
struct FileStat {
    reads: u32,
    writes: u32,
    last: i64,
    cards: Vec<String>,
}

pub struct Engine {
    root: PathBuf,
    registry: Registry,
    day: NaiveDate,
    indexed: bool,
    records: Vec<SessionRecord>,
    transcripts: HashMap<PathBuf, Transcript>,
    /// transcript session id → kart id
    owner: HashMap<String, String>,
    cards: HashMap<String, Card>,
    next_hue: u8,
    /// pid → (başlangıç zamanı, kart id): aynı süreç yeni session id alırsa aynı kart.
    pid_card: HashMap<u32, (Option<i64>, String)>,
    /// session id → transcript yolu (cwd'den bulunamazsa)
    sid_paths: HashMap<String, PathBuf>,
    dir_mtimes: HashMap<PathBuf, SystemTime>,
    last_dir_scan: i64,
    /// Fork'larda kopyalanan geçmiş iki kez sayılmasın.
    seen_uuids: HashSet<String>,
    recent_tools: VecDeque<i64>,
    recent_tokens: VecDeque<(i64, u64)>,
    recent_cost: VecDeque<(i64, f64)>,
    ring: VecDeque<EngineEvent>,
    prices: PriceTable,
    /// cwd → `/autocompact` penceresi
    compact_windows: HashMap<String, Option<u64>>,
    web: VecDeque<WebItem>,
    /// bugün dokunulan dosyalar (mutlak yol)
    files: HashMap<String, FileStat>,
    /// son 20 dakikanın yazmaları: (zaman, yol, kart)
    writes: VecDeque<(i64, String, String)>,
    past: Option<PastTotals>,
    home: Option<String>,
    host: HostState,
    runaways: Vec<Runaway>,
    apps: Vec<apps::AppProc>,
    /// Son cevabı değişen kartlar (uygulama kopyalama için alır)
    replies_dirty: HashSet<String>,
    /// Canlı kaydı bir kez görülmüş kartlar: kayıt kaybolunca oturum hemen biter (`StatusInput::record_gone`).
    /// `snapshot` salt okunur olduğu için RefCell.
    seen_live: std::cell::RefCell<HashSet<String>>,
    /// `$CODEX_HOME` (yoksa Codex okunmaz; testler gerçek klasöre bağlanmasın)
    codex_root: Option<PathBuf>,
    /// Süreç tablosundan bulunan yapay zekâ araçları
    agents: Vec<AgentProc>,
    /// Claude limitleri (statusLine dosyasından)
    claude_limits: Option<ToolLimits>,
}

impl Engine {
    pub fn new(claude_dir: PathBuf, today: NaiveDate) -> Self {
        Self {
            registry: Registry::new(&claude_dir),
            root: claude_dir,
            day: today,
            indexed: false,
            records: Vec::new(),
            transcripts: HashMap::new(),
            owner: HashMap::new(),
            cards: HashMap::new(),
            next_hue: 0,
            pid_card: HashMap::new(),
            sid_paths: HashMap::new(),
            dir_mtimes: HashMap::new(),
            last_dir_scan: 0,
            seen_uuids: HashSet::new(),
            recent_tools: VecDeque::new(),
            recent_tokens: VecDeque::new(),
            recent_cost: VecDeque::new(),
            ring: VecDeque::new(),
            prices: PriceTable::bundled(),
            compact_windows: HashMap::new(),
            web: VecDeque::new(),
            files: HashMap::new(),
            writes: VecDeque::new(),
            past: None,
            home: std::env::home_dir().map(|h| h.to_string_lossy().into_owned()),
            host: HostState::default(),
            runaways: Vec::new(),
            apps: Vec::new(),
            replies_dirty: HashSet::new(),
            seen_live: std::cell::RefCell::new(HashSet::new()),
            codex_root: None,
            agents: Vec::new(),
            claude_limits: None,
        }
    }

    /// Codex rollout'larını da oku.
    pub fn with_codex(mut self, root: Option<PathBuf>) -> Self {
        self.codex_root = root;
        self
    }

    pub fn set_agents(&mut self, list: Vec<AgentProc>) {
        self.agents = list;
    }

    pub fn set_claude_limits(&mut self, l: Option<ToolLimits>) {
        self.claude_limits = l;
    }

    /// Bugün bir sohbetin yazdığı dosyalar (Source Control'deki ✦ işareti)
    pub fn edited_paths(&self) -> Vec<String> {
        self.files
            .iter()
            .filter(|(_, f)| f.writes > 0)
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// Son cevabı değişen kartlar ve cevapları (yalnızca değişenler; her turda kopyalanmasın).
    pub fn take_reply_updates(&mut self) -> Vec<(String, String)> {
        let dirty = std::mem::take(&mut self.replies_dirty);
        dirty
            .into_iter()
            .filter_map(|id| self.cards.get(&id).map(|c| (id, c.last_reply.clone())))
            .collect()
    }

    /// Oturum sunucusunun son durumu (terminaller, kalıntılar).
    pub fn set_host(&mut self, host: HostState) {
        self.host = host;
    }

    pub fn set_runaways(&mut self, list: Vec<Runaway>) {
        self.runaways = list;
    }

    pub fn set_apps(&mut self, list: Vec<apps::AppProc>) {
        self.apps = list;
    }

    /// Canlı sohbetlerin klasörleri (kaçak süreç tespitinde hariç tutulur).
    pub fn chat_cwds(&self) -> Vec<String> {
        self.records
            .iter()
            .map(|r| r.cwd.clone())
            .filter(|c| !c.is_empty())
            .collect()
    }

    /// Ledger iş parçacığından gelen geçmiş toplamlar (bugün hariç).
    pub fn set_past(&mut self, past: PastTotals) {
        self.past = Some(past);
    }

    fn display_path(&self, p: &str) -> String {
        let shown = match &self.home {
            Some(h) if p.starts_with(h.as_str()) => format!("~{}", &p[h.len()..]),
            _ => p.to_string(),
        };
        redact::redact(&shown).into_owned()
    }

    /// Gömülü fiyat tablosu yerine başka bir tablo (kullanıcının güncel tablosu, testler).
    pub fn with_prices(mut self, prices: PriceTable) -> Self {
        self.prices = prices;
        self
    }

    /// Son `RING_SIZE` olay (arayüzün ilk yüklemesi için).
    pub fn recent_events(&self) -> Vec<EngineEvent> {
        self.ring.iter().cloned().collect()
    }

    /// Bir tur. İlk tur bugünün bütün kayıtlarını indeksler ve olay döndürmez (geçmiş `recent_events`'te).
    pub fn tick(
        &mut self,
        now: DateTime<Local>,
        alive: &dyn Fn(u32, Option<i64>) -> bool,
    ) -> (Snapshot, Vec<EngineEvent>) {
        if now.date_naive() != self.day {
            // Gün değişti: sayaçlar sıfırdan, bugünün kayıtlarıyla yeniden kurulur.
            let prices = self.prices.clone();
            let past = self.past.take();
            let codex = self.codex_root.take();
            *self = Engine::new(self.root.clone(), now.date_naive())
                .with_prices(prices)
                .with_codex(codex);
            self.past = past;
        }
        let now_ms = now.timestamp_millis();

        self.records = self.registry.scan();
        self.link_records();
        if !self.indexed || now_ms - self.last_dir_scan >= DIR_SCAN_MS {
            self.scan_project_dirs();
            self.scan_subagents();
            self.scan_codex();
            self.refresh_compact_windows();
            self.last_dir_scan = now_ms;
        }
        let mut events = Vec::new();
        self.read_transcripts(now_ms, &mut events);
        self.prune_rates(now_ms);

        if !self.indexed {
            self.indexed = true;
            events.clear();
        }
        (self.snapshot(now_ms, alive), events)
    }

    fn new_card(&mut self, id: &str) {
        if self.cards.contains_key(id) {
            return;
        }
        let hue = self.next_hue;
        self.next_hue = (self.next_hue + 1) % 10;
        self.cards.insert(
            id.to_string(),
            Card {
                hue,
                cwd: String::new(),
                name: None,
                title: None,
                entrypoint: None,
                hidden: false,
                model: None,
                context_tokens: 0,
                stats: DayStats::default(),
                last_prompt: None,
                last_tool: None,
                last_activity_ms: 0,
                activity: VecDeque::new(),
                last_reply: String::new(),
                reply_open: false,
                effort: None,
                turn_started_ms: 0,
                tool: "claude",
                turn_open: false,
                window: None,
                limits: None,
            },
        );
    }

    fn track(&mut self, path: PathBuf, card: &str, sub: Option<SubMeta>) {
        self.transcripts
            .entry(path.clone())
            .or_insert_with(|| Transcript {
                tail: Tail::new(path),
                card: card.to_string(),
                sub,
                pending: HashMap::new(),
                usage_seen: HashMap::new(),
                first_ts_ms: 0,
                last_ts_ms: 0,
                codex: false,
            });
    }

    fn transcript_path(&self, sid: &str, cwd: &str) -> Option<PathBuf> {
        let p = self
            .root
            .join("projects")
            .join(paths::encode_cwd(cwd))
            .join(format!("{sid}.jsonl"));
        if p.is_file() {
            return Some(p);
        }
        self.sid_paths.get(sid).cloned()
    }

    /// Kayıtları kartlara bağlar. Aynı pid (aynı başlangıç zamanıyla) yeni session id alırsa
    /// bu, compaction gibi bir devam sayılır ve kart değişmez.
    fn link_records(&mut self) {
        let records = std::mem::take(&mut self.records);
        for rec in &records {
            let card_id = if let Some(c) = self.owner.get(&rec.session_id) {
                c.clone()
            } else {
                match self.pid_card.get(&rec.pid) {
                    Some((started, c)) if *started == rec.started_at => c.clone(),
                    _ => rec.session_id.clone(),
                }
            };
            self.owner.insert(rec.session_id.clone(), card_id.clone());
            self.pid_card
                .insert(rec.pid, (rec.started_at, card_id.clone()));
            self.new_card(&card_id);
            if let Some(card) = self.cards.get_mut(&card_id) {
                if card.cwd.is_empty() {
                    card.cwd.clone_from(&rec.cwd);
                }
                if rec.name.as_deref().is_some_and(|n| !n.is_empty()) {
                    card.name.clone_from(&rec.name);
                }
                card.hidden = !rec.is_user_chat() || is_own_helper(rec.pid);
            }
            if let Some(path) = self.transcript_path(&rec.session_id, &rec.cwd) {
                self.track(path, &card_id, None);
            }
        }
        self.records = records;
    }

    /// `projects/*/` altında bugün değişmiş transcript'leri bulur.
    /// Klasörün mtime'ı yalnızca dosya eklenince değişir; o yüzden yalnızca değişen klasörler listelenir.
    fn scan_project_dirs(&mut self) {
        let Ok(dirs) = fs::read_dir(self.root.join("projects")) else {
            return;
        };
        let mut found = Vec::new();
        for dir in dirs.flatten() {
            let path = dir.path();
            let Ok(mtime) = dir.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if !path.is_dir() || self.dir_mtimes.get(&path) == Some(&mtime) {
                continue;
            }
            self.dir_mtimes.insert(path.clone(), mtime);
            let Ok(files) = fs::read_dir(&path) else {
                continue;
            };
            for f in files.flatten() {
                let fp = f.path();
                if fp.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Some(sid) = fp.file_stem().and_then(|s| s.to_str()).map(str::to_string) else {
                    continue;
                };
                self.sid_paths.insert(sid.clone(), fp.clone());
                let modified_today = f
                    .metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| DateTime::<Local>::from(t).date_naive() == self.day);
                if modified_today && !self.transcripts.contains_key(&fp) {
                    found.push((sid, fp));
                }
            }
        }
        for (sid, fp) in found {
            let card = self.owner.get(&sid).cloned().unwrap_or_else(|| sid.clone());
            self.owner.insert(sid, card.clone());
            self.new_card(&card);
            self.track(fp, &card, None);
        }
    }

    /// Codex: bugünün ve dünün klasöründe bugün değişmiş rollout'lar (gece yarısını geçen oturum dünün
    /// klasörüne yazmaya devam eder).
    fn scan_codex(&mut self) {
        let Some(root) = self.codex_root.clone() else {
            return;
        };
        let days = [Some(self.day), self.day.pred_opt()];
        let mut found = Vec::new();
        for d in days.into_iter().flatten() {
            let dir = root.join("sessions").join(d.format("%Y/%m/%d").to_string());
            let Ok(files) = fs::read_dir(&dir) else {
                continue;
            };
            for f in files.flatten() {
                let p = f.path();
                let name = f.file_name();
                let Some(name) = name.to_str() else { continue };
                if !name.starts_with("rollout-")
                    || !name.ends_with(".jsonl")
                    || self.transcripts.contains_key(&p)
                {
                    continue;
                }
                let today = f
                    .metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| DateTime::<Local>::from(t).date_naive() == self.day);
                if today {
                    found.push((format!("codex:{}", name.trim_end_matches(".jsonl")), p));
                }
            }
        }
        for (card, p) in found {
            self.new_card(&card);
            if let Some(c) = self.cards.get_mut(&card) {
                c.tool = "codex";
            }
            self.track(p.clone(), &card, None);
            if let Some(t) = self.transcripts.get_mut(&p) {
                t.codex = true;
            }
        }
    }

    /// `<proje>/<sessionId>/subagents/agent-*.jsonl` dosyalarını bulur.
    fn scan_subagents(&mut self) {
        let mains: Vec<(PathBuf, String)> = self
            .transcripts
            .iter()
            .filter(|(_, t)| t.sub.is_none() && !t.codex)
            .map(|(p, t)| (p.clone(), t.card.clone()))
            .collect();
        for (main, card) in mains {
            let (Some(parent), Some(sid)) = (main.parent(), main.file_stem()) else {
                continue;
            };
            let Ok(files) = fs::read_dir(parent.join(sid).join("subagents")) else {
                continue;
            };
            for f in files.flatten() {
                let fp = f.path();
                let name = f.file_name();
                let Some(name) = name.to_str() else { continue };
                if !name.starts_with("agent-")
                    || !name.ends_with(".jsonl")
                    || self.transcripts.contains_key(&fp)
                {
                    continue;
                }
                let meta_path = fp.with_extension("meta.json");
                self.track(
                    fp,
                    &card,
                    Some(SubMeta {
                        meta_path,
                        mtime: None,
                        finished: false,
                        kind: "agent".into(),
                        description: String::new(),
                        last_tool: None,
                    }),
                );
            }
        }
    }

    /// Her kartın klasörü için `/autocompact` penceresi (ayar dosyaları değişebilir; tarama sıklığında yenilenir).
    fn refresh_compact_windows(&mut self) {
        let env = std::env::var("CLAUDE_CODE_AUTO_COMPACT_WINDOW").ok();
        let cwds: HashSet<&str> = self.cards.values().map(|c| c.cwd.as_str()).collect();
        self.compact_windows = cwds
            .into_iter()
            .map(|cwd| {
                let w = settings::auto_compact_window(env.as_deref(), &self.root, cwd);
                (cwd.to_string(), w)
            })
            .collect();
    }

    fn read_transcripts(&mut self, now_ms: i64, events: &mut Vec<EngineEvent>) {
        let day = self.day;
        let Engine {
            transcripts,
            cards,
            seen_uuids,
            recent_tools,
            recent_tokens,
            recent_cost,
            ring,
            prices,
            web,
            files,
            writes,
            replies_dirty,
            ..
        } = self;

        // Sıralı okuma: aynı uuid'ler varsa önce eski (ebeveyn) dosya sayılır.
        let mut order: Vec<&PathBuf> = transcripts.keys().collect();
        order.sort();
        let order: Vec<PathBuf> = order.into_iter().cloned().collect();

        for path in order {
            let Some(t) = transcripts.get_mut(&path) else {
                continue;
            };
            if let Some(sub) = t.sub.as_mut() {
                refresh_meta(sub);
            }
            let Ok(lines) = t.tail.read_new() else {
                continue;
            };
            let Some(card) = cards.get_mut(&t.card) else {
                continue;
            };
            let is_main = t.sub.is_none();
            let is_codex = t.codex;

            for raw in lines {
                let parsed = if is_codex {
                    codex::parse_line(&raw)
                } else {
                    parse_line(&raw)
                };
                let Some(line) = parsed else {
                    continue;
                };
                if let Some(uuid) = &line.uuid
                    && !seen_uuids.insert(uuid.clone())
                {
                    continue;
                }
                let ts_ms = line.ts.map(|t| t.timestamp_millis());
                let today = line.ts.is_some_and(|t| t.date_naive() == day);
                let ev_ts = ts_ms.unwrap_or(now_ms);
                let in_window = now_ms - ev_ts < RATE_WINDOW_MS;
                if let Some(ms) = ts_ms {
                    if t.first_ts_ms == 0 {
                        t.first_ts_ms = ms;
                    }
                    t.last_ts_ms = t.last_ts_ms.max(ms);
                    card.last_activity_ms = card.last_activity_ms.max(ms);
                }
                if card.cwd.is_empty()
                    && let Some(cwd) = &line.cwd
                {
                    card.cwd.clone_from(cwd);
                }
                if is_main && line.effort.is_some() {
                    card.effort.clone_from(&line.effort);
                }
                if is_main && card.entrypoint.is_none() {
                    card.entrypoint.clone_from(&line.entrypoint);
                }

                let mut emit = |e: EngineEvent| {
                    if today {
                        ring.push_back(e.clone());
                        if ring.len() > RING_SIZE {
                            ring.pop_front();
                        }
                        events.push(e);
                    }
                };
                let event = |kind, tool: Option<String>, category, target, id| EngineEvent {
                    ts: ev_ts,
                    session: t.card.clone(),
                    kind,
                    tool,
                    category,
                    target,
                    id,
                    subagent: !is_main,
                };

                for item in line.items {
                    match item {
                        Item::Title(x) if is_main => card.title = Some(clean(&x, 80)),
                        Item::LastPrompt(x) if is_main => {
                            card.last_prompt = Some(clean(&x, PROMPT_MAX));
                        }
                        Item::Prompt(x) if is_main => {
                            card.reply_open = false;
                            card.turn_started_ms = ev_ts;
                            // Codex'te başlık yok: ilk prompt başlık olur.
                            if is_codex && card.title.is_none() {
                                card.title = Some(clean(&x, 60));
                            }
                            let text = clean(&x, PROMPT_MAX);
                            card.last_prompt = Some(text.clone());
                            if today {
                                card.stats.prompts += 1;
                            }
                            emit(event(EventKind::Prompt, None, None, Some(text), None));
                        }
                        Item::ToolUse { id, name, input } => {
                            if is_main {
                                card.reply_open = false;
                            }
                            let cat = tools::category(&name);
                            t.pending.insert(id.clone(), name.clone());
                            match t.sub.as_mut() {
                                Some(sub) => sub.last_tool = Some(name.clone()),
                                None => card.last_tool = Some(name.clone()),
                            }
                            if today {
                                let s = &mut card.stats;
                                *s.tools.entry(cat).or_default() += 1;
                                s.tool_calls += 1;
                                if cat == "web" {
                                    s.web_calls += 1;
                                }
                                if let Some(fp) = tools::file_path(&name, &input) {
                                    let write = tools::is_write(&name);
                                    let f = files.entry(fp.clone()).or_default();
                                    if write {
                                        f.writes += 1;
                                    } else {
                                        f.reads += 1;
                                    }
                                    f.last = f.last.max(ev_ts);
                                    if !f.cards.contains(&t.card) {
                                        f.cards.push(t.card.clone());
                                    }
                                    if write && now_ms - ev_ts < SAME_FILE_WINDOW_MS {
                                        writes.push_back((ev_ts, fp.clone(), t.card.clone()));
                                    }
                                    if write {
                                        s.files_edited.insert(fp);
                                    } else {
                                        s.files_read.insert(fp);
                                    }
                                }
                                if matches!(cat, "web" | "browser") {
                                    let kind = match name.as_str() {
                                        "WebSearch" | "search_web" => "search",
                                        _ if cat == "browser" => "browser",
                                        _ => "fetch",
                                    };
                                    if let Some(target) = tools::target(&name, &input) {
                                        web.push_front(WebItem {
                                            ts: ev_ts,
                                            session: t.card.clone(),
                                            kind,
                                            target: clean(&target, TARGET_MAX),
                                            subagent: !is_main,
                                        });
                                        web.truncate(WEB_FEED_SIZE);
                                    }
                                }
                                if in_window {
                                    recent_tools.push_back(ev_ts);
                                }
                                if now_ms - ev_ts < ACTIVITY_WINDOW_MS {
                                    card.activity.push_back(ev_ts);
                                }
                            }
                            let target =
                                tools::target(&name, &input).map(|x| clean(&x, TARGET_MAX));
                            emit(event(
                                EventKind::Tool,
                                Some(name),
                                Some(cat),
                                target,
                                Some(id),
                            ));
                        }
                        Item::ToolResult { id, is_error } => {
                            let name = t.pending.remove(&id);
                            if today {
                                card.stats.tool_results += 1;
                                if is_error {
                                    card.stats.tool_errors += 1;
                                }
                            }
                            if is_error {
                                let cat = name.as_deref().map(tools::category);
                                emit(event(EventKind::ToolError, name, cat, None, Some(id)));
                            }
                        }
                        Item::Reply(x) if is_main => {
                            const MAX_REPLY: usize = 256 * 1024;
                            if card.reply_open {
                                card.last_reply.push_str("\n\n");
                                card.last_reply.push_str(&x);
                            } else {
                                card.last_reply.clone_from(&x);
                            }
                            if card.last_reply.len() > MAX_REPLY {
                                let mut cut = card.last_reply.len() - MAX_REPLY;
                                while !card.last_reply.is_char_boundary(cut) {
                                    cut += 1;
                                }
                                card.last_reply.drain(..cut);
                            }
                            card.reply_open = true;
                            replies_dirty.insert(t.card.clone());
                            emit(event(
                                EventKind::Reply,
                                None,
                                None,
                                Some(clean(&x, TARGET_MAX)),
                                None,
                            ));
                        }
                        Item::Usage {
                            msg_id,
                            model,
                            tokens,
                        } => {
                            let prev = t.usage_seen.get(&msg_id).copied().unwrap_or_default();
                            let merged = prev.max(tokens);
                            let delta = merged.saturating_sub(prev);
                            t.usage_seen.insert(msg_id, merged);
                            if today && is_codex {
                                // Codex token'ları sayılır ama fiyatlandırılmaz.
                                let s = &mut card.stats;
                                s.tokens += delta;
                                if in_window && delta.total() > 0 {
                                    recent_tokens.push_back((ev_ts, delta.total()));
                                }
                                let m = card.model.clone().unwrap_or_else(|| "codex".into());
                                s.by_model.entry(m).or_default().add(&Entry {
                                    tokens: delta,
                                    cost: 0.0,
                                });
                            } else if today {
                                let s = &mut card.stats;
                                s.tokens += delta;
                                if in_window && delta.total() > 0 {
                                    recent_tokens.push_back((ev_ts, delta.total()));
                                }
                                // Uzun bağlam eşiği istemin boyutuna bakar; iki taraf aynı bağlamla fiyatlanır.
                                let ctx = merged.context();
                                match (
                                    prices.cost(&model, &merged, ctx),
                                    prices.cost(&model, &prev, ctx),
                                ) {
                                    (Some(now_cost), Some(before)) => {
                                        let usd = (now_cost - before).max(0.0);
                                        s.cost += usd;
                                        s.by_model.entry(model.clone()).or_default().add(&Entry {
                                            tokens: delta,
                                            cost: usd,
                                        });
                                        s.savings +=
                                            prices.cache_savings(&model, &delta).unwrap_or(0.0);
                                        if usd > 0.0 && now_ms - ev_ts < BURN_WINDOW_MS {
                                            recent_cost.push_back((ev_ts, usd));
                                        }
                                    }
                                    _ => {
                                        s.unpriced |= delta.total() > 0;
                                        s.by_model.entry(model.clone()).or_default().add(&Entry {
                                            tokens: delta,
                                            cost: 0.0,
                                        });
                                    }
                                }
                            }
                            if is_main && !is_codex {
                                card.model = Some(model);
                                card.context_tokens = merged.context();
                            }
                        }
                        Item::Session { cwd, .. } if !cwd.is_empty() => card.cwd = cwd,
                        Item::Model(m) => card.model = Some(m),
                        Item::TurnStart => card.turn_open = true,
                        Item::TurnEnd => {
                            card.turn_open = false;
                            t.pending.clear();
                        }
                        Item::ContextWindow(w) => card.window = Some(w),
                        Item::ContextTokens(n) => card.context_tokens = n,
                        Item::Limits(l) => card.limits = Some((ev_ts, l)),
                        _ => {}
                    }
                }
            }
        }
    }

    fn prune_rates(&mut self, now_ms: i64) {
        while self
            .recent_tools
            .front()
            .is_some_and(|&t| now_ms - t >= RATE_WINDOW_MS)
        {
            self.recent_tools.pop_front();
        }
        while self
            .recent_tokens
            .front()
            .is_some_and(|&(t, _)| now_ms - t >= RATE_WINDOW_MS)
        {
            self.recent_tokens.pop_front();
        }
        while self
            .recent_cost
            .front()
            .is_some_and(|&(t, _)| now_ms - t >= BURN_WINDOW_MS)
        {
            self.recent_cost.pop_front();
        }
        while self
            .writes
            .front()
            .is_some_and(|(t, _, _)| now_ms - t >= SAME_FILE_WINDOW_MS)
        {
            self.writes.pop_front();
        }
        for card in self.cards.values_mut() {
            while card
                .activity
                .front()
                .is_some_and(|&t| now_ms - t >= ACTIVITY_WINDOW_MS)
            {
                card.activity.pop_front();
            }
        }
    }

    fn snapshot(&self, now_ms: i64, alive: &dyn Fn(u32, Option<i64>) -> bool) -> Snapshot {
        let start_of_day = Local
            .from_local_datetime(&self.day.and_hms_opt(0, 0, 0).unwrap_or_default())
            .earliest()
            .map_or(0, |d| d.timestamp_millis());

        // Kart başına canlı kayıt (süreci yaşayan, en yeni).
        let mut live: HashMap<&str, &SessionRecord> = HashMap::new();
        for rec in &self.records {
            let Some(card) = self.owner.get(&rec.session_id) else {
                continue;
            };
            let start = rec.proc_start.as_deref().and_then(proc::parse_proc_start);
            if !alive(rec.pid, start) {
                continue;
            }
            live.entry(card.as_str())
                .and_modify(|cur| {
                    if rec.updated_at > cur.updated_at {
                        *cur = rec;
                    }
                })
                .or_insert(rec);
        }
        {
            let mut seen = self.seen_live.borrow_mut();
            for card in live.keys() {
                seen.insert((*card).to_string());
            }
        }

        // Same file: son 20 dakikada aynı yola yazan farklı kartlar (alt ajanlar kendi kartına bağlı).
        let mut writers: HashMap<&str, Vec<&str>> = HashMap::new();
        for (_, path, card) in &self.writes {
            let cards = writers.entry(path.as_str()).or_default();
            if !cards.contains(&card.as_str()) {
                cards.push(card.as_str());
            }
        }
        writers.retain(|_, cards| cards.len() >= 2);

        // Sohbet ↔ terminal: kaydın pid'i terminalin kabuğunun torunu mu (ya da ön plandaki süreç mi)?
        let mut chat_terminal: HashMap<&str, String> = HashMap::new();
        for (card, rec) in &live {
            if let Some(t) = self.host.sessions.iter().find(|t| {
                t.fg_pid == Some(rec.pid)
                    || t.pid.is_some_and(|shell| is_descendant(rec.pid, shell))
            }) {
                chat_terminal.insert(card, t.id.clone());
            }
        }

        let mut kpis = Kpis::default();
        let mut tool_results = 0u32;
        let mut sessions = Vec::new();

        for (id, card) in &self.cards {
            let sdk = card
                .entrypoint
                .as_deref()
                .is_some_and(|e| e.starts_with("sdk"));
            if card.hidden || sdk {
                continue;
            }
            let rec = live.get(id.as_str()).copied();
            // Codex: aynı klasörde çalışan en yeni codex süreci; tur açıksa çalışıyor, soru soruyorsa seni bekliyor.
            let codex_proc = (card.tool == "codex")
                .then(|| {
                    self.agents
                        .iter()
                        .filter(|a| a.tool == "codex" && !card.cwd.is_empty() && a.cwd == card.cwd)
                        .max_by_key(|a| a.start_secs)
                })
                .flatten();
            let asking = card.tool == "codex"
                && card.turn_open
                && self
                    .transcripts
                    .values()
                    .any(|t| &t.card == id && t.pending.values().any(|n| n == "AskUserQuestion"));
            let codex_state = codex_proc.map(|_| match (card.turn_open, asking) {
                (true, true) => "waiting",
                (true, false) => "busy",
                _ => "idle",
            });
            let mut input = StatusInput {
                live_record: rec
                    .map(|r| r.status.as_deref().unwrap_or(""))
                    .or(codex_state),
                last_activity_age_ms: (card.last_activity_ms > 0)
                    .then(|| now_ms - card.last_activity_ms),
                subagent_running: false,
                record_gone: rec.is_none()
                    && codex_state.is_none()
                    && self.seen_live.borrow().contains(id.as_str()),
            };
            let main_busy = base_status(&input) == Status::Busy;

            let mut subagents: Vec<SubagentView> = self
                .transcripts
                .iter()
                .filter(|(_, t)| &t.card == id)
                .filter_map(|(p, t)| {
                    let sub = t.sub.as_ref()?;
                    let age = now_ms - t.last_ts_ms;
                    Some(SubagentView {
                        id: p.file_stem()?.to_string_lossy().into_owned(),
                        kind: sub.kind.clone(),
                        description: clean(&sub.description, 80),
                        running: subagent_running(sub.finished, t.pending.len(), age, main_busy),
                        last_tool: sub.last_tool.clone(),
                        current_tool: t.pending.values().next().cloned(),
                        started: t.first_ts_ms,
                    })
                })
                .collect();
            subagents.retain(|s| s.running);
            subagents.sort_by(|a, b| a.id.cmp(&b.id));
            input.subagent_running = !subagents.is_empty();
            let status = status_of(&input);

            if status == Status::Ended && card.last_activity_ms < start_of_day {
                continue;
            }

            let s = &card.stats;
            if status != Status::Ended {
                kpis.sessions_live += 1;
            }
            kpis.agents_running += subagents.len();
            kpis.web_calls += s.web_calls;
            kpis.tool_calls += s.tool_calls;
            kpis.tool_errors += s.tool_errors;
            kpis.tokens_today += s.tokens;
            kpis.cost_today += s.cost;
            kpis.cache_savings_today += s.savings;
            tool_results += s.tool_results;
            for (cat, n) in &s.tools {
                *kpis.tool_mix.entry(cat).or_default() += n;
            }
            let mut same_file: Vec<String> = writers
                .iter()
                .filter(|(_, cards)| cards.contains(&id.as_str()))
                .map(|(path, _)| self.display_path(path))
                .collect();
            same_file.sort();

            let model_window = card
                .model
                .as_deref()
                .and_then(|m| self.prices.context_window(m));
            let compact = self.compact_windows.get(&card.cwd).copied().flatten();
            let context_window = match (card.window, model_window, compact) {
                (Some(w), _, _) => Some(w),
                (None, Some(m), Some(c)) => Some(m.min(c)),
                (None, m, c) => m.or(c),
            };
            let context_pct = context_window
                .filter(|&w| w > 0)
                .map(|w| 100.0 * card.context_tokens as f64 / w as f64);
            let mut activity = vec![0u16; ACTIVITY_BUCKETS];
            for &t in &card.activity {
                let age_min = usize::try_from((now_ms - t).max(0) / 60_000).unwrap_or(usize::MAX);
                if let Some(slot) = ACTIVITY_BUCKETS.checked_sub(age_min + 1) {
                    activity[slot] = activity[slot].saturating_add(1);
                }
            }

            let project = paths::project_name(&card.cwd);
            let title = card
                .name
                .as_deref()
                .or(card.title.as_deref())
                .map_or_else(|| project.clone(), |t| clean(t, 80));
            sessions.push(SessionView {
                id: id.clone(),
                tool: card.tool.to_string(),
                pid: rec.map(|r| r.pid).or(codex_proc.map(|a| a.pid)),
                title,
                project,
                cwd: card.cwd.clone(),
                status,
                waiting_for: (status == Status::Waiting)
                    .then(|| {
                        rec.and_then(SessionRecord::waiting_for_text)
                            .or_else(|| asking.then(|| "question".to_string()))
                    })
                    .flatten()
                    .map(|w| clean(&w, 80)),
                model: card.model.clone(),
                context_tokens: card.context_tokens,
                context_window,
                context_pct,
                tokens: s.tokens,
                cost: s.cost,
                unpriced: s.unpriced,
                activity,
                tools: s.tools.clone(),
                tool_calls: s.tool_calls,
                tool_errors: s.tool_errors,
                web_calls: s.web_calls,
                prompts: s.prompts,
                files_edited: s.files_edited.len(),
                files_read: s.files_read.len(),
                last_prompt: card.last_prompt.clone(),
                last_tool: card.last_tool.clone(),
                last_activity: card.last_activity_ms,
                hue: card.hue,
                effort: card.effort.clone(),
                turn_started: card.turn_started_ms,
                subagents,
                same_file,
                terminal: chat_terminal.get(id.as_str()).cloned(),
            });
        }

        // Rollout'u olmayan araçlar (Gemini, Aider, Antigravity…; bugün rollout'u görülmeyen Codex): süreçten kart.
        let codex_cards: Vec<&Card> = self.cards.values().filter(|c| c.tool == "codex").collect();
        for a in &self.agents {
            if a.tool == "codex"
                && codex_cards
                    .iter()
                    .any(|c| !c.cwd.is_empty() && c.cwd == a.cwd)
            {
                continue;
            }
            let project = paths::project_name(&a.cwd);
            let status = if a.busy { Status::Busy } else { Status::Idle };
            kpis.sessions_live += 1;
            sessions.push(SessionView {
                id: format!("ai:{}:{}", a.pid, a.start_secs),
                tool: a.tool.to_string(),
                pid: Some(a.pid),
                title: format!("{} · {project}", a.tool),
                project,
                cwd: a.cwd.clone(),
                status,
                waiting_for: None,
                model: None,
                context_tokens: 0,
                context_window: None,
                context_pct: None,
                tokens: Tokens::default(),
                cost: 0.0,
                unpriced: false,
                activity: vec![0; ACTIVITY_BUCKETS],
                same_file: Vec::new(),
                terminal: self
                    .host
                    .sessions
                    .iter()
                    .find(|t| {
                        t.fg_pid == Some(a.pid) || t.pid.is_some_and(|s| is_descendant(a.pid, s))
                    })
                    .map(|t| t.id.clone()),
                tools: BTreeMap::new(),
                tool_calls: 0,
                tool_errors: 0,
                web_calls: 0,
                prompts: 0,
                files_edited: 0,
                files_read: 0,
                last_prompt: a.child.as_ref().map(|c| format!("running {c}")),
                last_tool: a.child.clone(),
                last_activity: now_ms,
                hue: (a.pid % 10) as u8,
                effort: None,
                turn_started: 0,
                subagents: Vec::new(),
            });
        }

        sessions.sort_by(|a, b| {
            a.status
                .cmp(&b.status)
                .then(b.last_activity.cmp(&a.last_activity))
        });

        // Limitler: Claude statusLine'dan, Codex en son etkin Codex oturumunun rollout'undan.
        let mut limits: Vec<ToolLimits> = self.claude_limits.iter().cloned().collect();
        if let Some((ts, l)) = self
            .cards
            .values()
            .filter_map(|c| c.limits.as_ref())
            .max_by_key(|(ts, _)| *ts)
        {
            limits.push(ToolLimits::from_windows("codex", l, *ts));
        }

        let minutes = (RATE_WINDOW_MS / 60_000) as f64;
        kpis.tools_per_min = self.recent_tools.len() as f64 / minutes;
        kpis.tokens_per_min =
            self.recent_tokens.iter().map(|(_, n)| *n).sum::<u64>() as f64 / minutes;
        kpis.tool_ok_pct = (tool_results > 0).then(|| {
            100.0 * f64::from(tool_results - kpis.tool_errors.min(tool_results))
                / f64::from(tool_results)
        });
        let t = kpis.tokens_today;
        let cache_base = t.input + t.cache_read + t.cache_write;
        kpis.cache_hit_pct =
            (cache_base > 0).then(|| 100.0 * t.cache_read as f64 / cache_base as f64);
        // Son 60 dakikada harcanan, doğrudan $/saat.
        kpis.burn_per_hour = self.recent_cost.iter().map(|(_, usd)| usd).sum::<f64>() + 0.0;

        let mut hot: Vec<(&String, &FileStat)> = self.files.iter().collect();
        hot.sort_by(|a, b| {
            (b.1.reads + b.1.writes)
                .cmp(&(a.1.reads + a.1.writes))
                .then(b.1.last.cmp(&a.1.last))
        });
        let files = hot
            .into_iter()
            .take(FILES_HEAT_SIZE)
            .map(|(path, f)| FileHeat {
                path: self.display_path(path),
                reads: f.reads,
                writes: f.writes,
                last: f.last,
                sessions: f.cards.clone(),
                clash: writers.contains_key(path.as_str()),
            })
            .collect();
        let mut clashes: Vec<Clash> = writers
            .iter()
            .map(|(path, cards)| Clash {
                path: self.display_path(path),
                sessions: cards.iter().map(|c| c.to_string()).collect(),
                last: self
                    .writes
                    .iter()
                    .filter(|(_, p, _)| p == path)
                    .map(|(t, _, _)| *t)
                    .max()
                    .unwrap_or(0),
            })
            .collect();
        clashes.sort_by_key(|c| std::cmp::Reverse(c.last));

        let terminals = self
            .host
            .sessions
            .iter()
            .map(|t| {
                let shell = t
                    .command
                    .first()
                    .map(|c| paths::project_name(c))
                    .unwrap_or_default();
                TerminalView {
                    id: t.id.clone(),
                    cwd: t.cwd.clone(),
                    project: paths::project_name(&t.cwd),
                    title: if t.busy {
                        t.fg_name.as_deref().map(display_command).unwrap_or(shell)
                    } else {
                        shell
                    },
                    busy: t.busy,
                    chat: chat_terminal
                        .iter()
                        .find(|(_, tid)| **tid == t.id)
                        .map(|(c, _)| c.to_string()),
                    started: t.started,
                    attached: t.attached > 0,
                }
            })
            .collect();

        Snapshot {
            generated_at: now_ms,
            day: self.day.to_string(),
            sessions,
            kpis,
            web: self.web.iter().cloned().collect(),
            files,
            clashes,
            ledger: self.ledger_view(),
            terminals,
            limits,
            // Kalan süreç çalışan bir uygulamaysa (ya da onun alt süreciyse) yalnızca araba olarak görünür
            leftovers: self
                .host
                .leftovers
                .iter()
                .filter(|l| {
                    !self.apps.iter().any(|a| {
                        a.pid == l.pid || is_descendant(l.pid, a.pid) || is_descendant(a.pid, l.pid)
                    })
                })
                .cloned()
                .collect(),
            runaways: self.runaways.clone(),
            apps: self
                .apps
                .iter()
                .map(|a| AppView {
                    pid: a.pid,
                    label: redact::clean(&a.label, 60),
                    project: paths::project_name(&a.cwd),
                    cwd: a.cwd.clone(),
                    cpu_pct: a.cpu_pct,
                    started: a.start_secs * 1000,
                    terminal: self
                        .host
                        .sessions
                        .iter()
                        .find(|t| {
                            t.fg_pid == Some(a.pid)
                                || t.pid.is_some_and(|sh| is_descendant(a.pid, sh))
                        })
                        .map(|t| t.id.clone()),
                })
                .collect(),
        }
    }

    /// spend.ledger: geçmiş (dondurulmuş) toplamlar + bugünün canlı sayaçları.
    fn ledger_view(&self) -> LedgerView {
        // Bugün: gizli (sdk) oturumlar dahil bütün kullanım; geçmiş günler de böyle okunur.
        let mut today: HashMap<(String, String), Entry> = HashMap::new();
        for card in self.cards.values() {
            let project = paths::project_name(&card.cwd);
            for (model, e) in &card.stats.by_model {
                today
                    .entry((project.clone(), model.clone()))
                    .or_default()
                    .add(e);
            }
        }
        // Boş f64 toplamı -0.0 döner; + 0.0 onu 0.0'a çevirir (arayüzde "$-0.00" görünmesin).
        let today_cost: f64 = today.values().map(|e| e.cost).sum::<f64>() + 0.0;
        let ranges = RANGES
            .iter()
            .enumerate()
            .map(|(i, (key, _))| {
                let mut merged = self
                    .past
                    .as_ref()
                    .and_then(|p| p.ranges.get(i).cloned())
                    .unwrap_or_default();
                for (k, e) in &today {
                    merged.entry(k.clone()).or_default().add(e);
                }
                let cost = merged.values().map(|e| e.cost).sum::<f64>() + 0.0;
                let tokens = merged.values().map(|e| e.tokens.total()).sum();
                let mut rows: Vec<LedgerRow> = merged
                    .into_iter()
                    .filter(|(_, e)| e.tokens.total() > 0)
                    .map(|((project, model), e)| LedgerRow {
                        project,
                        model,
                        tokens: e.tokens.total(),
                        cost: e.cost,
                    })
                    .collect();
                rows.sort_by(|a, b| b.cost.total_cmp(&a.cost).then(b.tokens.cmp(&a.tokens)));
                rows.truncate(LEDGER_ROWS);
                LedgerRange {
                    key,
                    cost,
                    tokens,
                    rows,
                }
            })
            .collect();
        let mut daily: Vec<f64> = self
            .past
            .as_ref()
            .map(|p| p.daily.iter().map(|(_, c)| *c).collect())
            .unwrap_or_default();
        daily.push(today_cost);
        LedgerView {
            ready: self
                .past
                .as_ref()
                .is_some_and(|p| p.today == Some(self.day)),
            ranges,
            daily,
        }
    }
}

/// `pid`, `ancestor`'ın torunu mu (en fazla 12 kuşak yukarı bakılır).
fn is_descendant(pid: u32, ancestor: u32) -> bool {
    let mut cur = pid;
    for _ in 0..12 {
        let Some(p) = crate::procs::info(cur) else {
            return false;
        };
        if p.ppid == ancestor {
            return true;
        }
        if p.ppid <= 1 || p.ppid == cur {
            return false;
        }
        cur = p.ppid;
    }
    false
}

/// Alt ajanın `.meta.json` dosyasını (değiştiyse) yeniden okur.
fn refresh_meta(sub: &mut SubMeta) {
    let Ok(mtime) = fs::metadata(&sub.meta_path).and_then(|m| m.modified()) else {
        return;
    };
    if sub.mtime == Some(mtime) {
        return;
    }
    let Some(v) = fs::read(&sub.meta_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    else {
        return;
    };
    sub.mtime = Some(mtime);
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    sub.finished = v.get("finished").and_then(Value::as_bool) == Some(true);
    sub.kind = s("agentType").unwrap_or_else(|| "agent".into());
    sub.description = s("description").unwrap_or_default();
}

#[cfg(test)]
mod tests;

/// Ön plandaki sürecin görünen adı: Claude Code süreç adını sürüm numarası yapar (`2.1.319`) → "claude".
fn display_command(name: &str) -> String {
    let is_version = !name.is_empty()
        && name.split('.').count() >= 2
        && name
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    if is_version {
        "claude".into()
    } else {
        name.to_string()
    }
}
