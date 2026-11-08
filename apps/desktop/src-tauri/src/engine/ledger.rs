//! Harcama defteri (MIMARI.md §3.5): gün → proje → model → toplamlar.
//!
//! - Geçmiş günler transcript'lerden **bir kez** okunur ve dondurulur (`frozen_through`). Claude Code
//!   transcript'leri sildikten sonra da veri kaybolmaz.
//! - Bugün engine'in canlı sayaçlarından gelir; gün değişince dün okunup dondurulur.
//! - Bu dosya uygulamanın kendi veri klasöründedir; `~/.claude` altına hiçbir şey yazılmaz.

use super::paths;
use super::pricing::PriceTable;
use super::transcript::{Tokens, tokens_of};
use chrono::{DateTime, Days, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub tokens: Tokens,
    pub cost: f64,
}

impl Entry {
    pub fn add(&mut self, o: &Entry) {
        self.tokens += o.tokens;
        self.cost += o.cost;
    }
}

/// "2026-11-08" → proje → model → toplam
type DayMap = BTreeMap<String, BTreeMap<String, BTreeMap<String, Entry>>>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    version: u32,
    /// Bu gün (dahil) ve öncesi dondurulmuş; bir daha transcript'ten okunmaz.
    frozen_through: Option<NaiveDate>,
    days: DayMap,
}

/// Arayüzdeki sekmeler: anahtar ve kaç günü (bugün dahil) kapsadığı.
pub const RANGES: [(&str, u64); 5] = [
    ("today", 1),
    ("7d", 7),
    ("30d", 30),
    ("12w", 84),
    ("12m", 365),
];

/// Geçmiş günlerin (bugün hariç) aralık başına toplamları. Engine bunlara bugünü ekler.
#[derive(Debug, Clone, Default)]
pub struct PastTotals {
    pub today: Option<NaiveDate>,
    /// RANGES ile aynı sırada: (proje, model) → toplam
    pub ranges: Vec<HashMap<(String, String), Entry>>,
    /// Son 30 günün günlük maliyeti (bugün hariç), eskiden yeniye.
    pub daily: Vec<(String, f64)>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct BackfillStats {
    pub files: usize,
    pub messages: usize,
}

fn hash(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn local_midnight(day: NaiveDate) -> Option<DateTime<Local>> {
    Local
        .from_local_datetime(&day.and_hms_opt(0, 0, 0)?)
        .earliest()
}

/// `projects/` altındaki bütün `.jsonl` dosyaları (alt ajanlar dahil), en fazla `depth` seviye.
fn collect_jsonl(dir: &Path, depth: u8, since: Option<SystemTime>, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let p = e.path();
        if ft.is_dir() && depth > 0 {
            collect_jsonl(&p, depth - 1, since, out);
        } else if ft.is_file() && p.extension().is_some_and(|x| x == "jsonl") {
            let fresh = since.is_none_or(|s| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|m| m >= s)
            });
            if fresh {
                out.push(p);
            }
        }
    }
}

impl Ledger {
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Ledger>(&b).ok())
            .filter(|l| l.version == VERSION)
            .unwrap_or_else(|| Ledger {
                version: VERSION,
                ..Default::default()
            })
    }

    /// Önce geçici dosyaya yazar, sonra yerine taşır (yarım dosya kalmasın).
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec(self).map_err(io::Error::other)?)?;
        fs::rename(tmp, path)
    }

    pub fn frozen_through(&self) -> Option<NaiveDate> {
        self.frozen_through
    }

    /// Dün henüz dondurulmamışsa true.
    pub fn needs_freeze(&self, today: NaiveDate) -> bool {
        let yesterday = today.pred_opt().unwrap_or(today);
        self.frozen_through.is_none_or(|f| f < yesterday)
    }

    /// Dondurulmamış geçmiş günleri (bugün hariç) transcript'lerden okur ve dondurur.
    pub fn backfill(
        &mut self,
        claude_dir: &Path,
        today: NaiveDate,
        prices: &PriceTable,
    ) -> BackfillStats {
        let Some(until) = today.pred_opt() else {
            return BackfillStats::default();
        };
        let since = self.frozen_through.and_then(|f| f.succ_opt());
        if since.is_some_and(|s| s > until) {
            return BackfillStats::default();
        }
        // Dondurulmuş günden sonra hiç değişmemiş dosyada yeni gün olamaz.
        let since_time = since.and_then(local_midnight).map(SystemTime::from);
        let mut files = Vec::new();
        collect_jsonl(&claude_dir.join("projects"), 3, since_time, &mut files);
        files.sort();

        let mut seen_uuids: HashSet<u64> = HashSet::new();
        // mesaj → (gün, proje, model, birleşik kullanım)
        let mut messages: HashMap<u64, (String, String, String, Tokens)> = HashMap::new();
        for file in &files {
            let Ok(f) = File::open(file) else { continue };
            let fallback_project = file
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            for line in BufReader::new(f).lines().map_while(Result::ok) {
                // Ucuz ön eleme: kullanım yalnızca assistant satırlarında.
                if !line.contains("\"usage\"") || !line.contains("\"assistant\"") {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if v.get("type").and_then(Value::as_str) != Some("assistant") {
                    continue;
                }
                let Some(day) = v
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                    .map(|t| t.with_timezone(&Local).date_naive())
                else {
                    continue;
                };
                if day > until || since.is_some_and(|s| day < s) {
                    continue;
                }
                if let Some(u) = v.get("uuid").and_then(Value::as_str)
                    && !seen_uuids.insert(hash(u))
                {
                    continue;
                }
                let Some(msg) = v.get("message") else {
                    continue;
                };
                let (Some(id), Some(usage)) =
                    (msg.get("id").and_then(Value::as_str), msg.get("usage"))
                else {
                    continue;
                };
                let model = msg
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                if model == "<synthetic>" {
                    continue;
                }
                let project = v
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(paths::project_name)
                    .unwrap_or_else(|| fallback_project.clone());
                let tokens = tokens_of(usage);
                messages
                    .entry(hash(id))
                    .and_modify(|m| m.3 = m.3.max(tokens))
                    .or_insert_with(|| (day.to_string(), project, model.to_string(), tokens));
            }
        }

        let stats = BackfillStats {
            files: files.len(),
            messages: messages.len(),
        };
        for (day, project, model, tokens) in messages.into_values() {
            let cost = prices
                .cost(&model, &tokens, tokens.context())
                .unwrap_or(0.0);
            self.days
                .entry(day)
                .or_default()
                .entry(project)
                .or_default()
                .entry(model)
                .or_default()
                .add(&Entry { tokens, cost });
        }
        self.frozen_through = Some(until);
        stats
    }

    /// Bugün hariç, her aralığın geçmiş kısmının toplamları.
    pub fn past_totals(&self, today: NaiveDate) -> PastTotals {
        let mut ranges = vec![HashMap::new(); RANGES.len()];
        let mut daily = Vec::new();
        for back in 1..365u64 {
            let Some(day) = today.checked_sub_days(Days::new(back)) else {
                break;
            };
            let key = day.to_string();
            let mut day_cost = 0.0;
            if let Some(projects) = self.days.get(&key) {
                for (project, models) in projects {
                    for (model, e) in models {
                        day_cost += e.cost;
                        for (i, (_, len)) in RANGES.iter().enumerate() {
                            if back < *len {
                                ranges[i]
                                    .entry((project.clone(), model.clone()))
                                    .or_insert_with(Entry::default)
                                    .add(e);
                            }
                        }
                    }
                }
            }
            if back < 30 {
                daily.push((key, day_cost));
            }
        }
        daily.reverse();
        PastTotals {
            today: Some(today),
            ranges,
            daily,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn line(uuid: &str, ts: &str, msg: &str, model: &str, out: u64, cwd: &str) -> String {
        json!({"type":"assistant","uuid":uuid,"timestamp":ts,"cwd":cwd,
               "message":{"id":msg,"model":model,"content":[],
                          "usage":{"input_tokens":1_000_000,"output_tokens":out}}})
        .to_string()
    }

    fn write(path: &Path, lines: &[String]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = File::create(path).unwrap();
        for l in lines {
            writeln!(f, "{l}").unwrap();
        }
    }

    fn noon(day: NaiveDate) -> String {
        local_midnight(day)
            .unwrap()
            .checked_add_signed(chrono::Duration::hours(12))
            .unwrap()
            .to_rfc3339()
    }

    #[test]
    fn backfills_past_days_once_and_freezes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let d1 = today.pred_opt().unwrap();
        let d3 = today.checked_sub_days(Days::new(3)).unwrap();
        let proj = root.join("projects/-w-app");
        write(
            &proj.join("a.jsonl"),
            &[
                line("u1", &noon(d3), "m1", "claude-opus-5-5", 0, "/w/app"),
                // aynı mesaj iki satır: max alınır
                line("u2", &noon(d1), "m2", "claude-opus-5-5", 500_000, "/w/app"),
                line(
                    "u3",
                    &noon(d1),
                    "m2",
                    "claude-opus-5-5",
                    1_000_000,
                    "/w/app",
                ),
                // bugün: deftere girmez (canlı sayaçlardan gelir)
                line("u4", &noon(today), "m3", "claude-opus-5-5", 0, "/w/app"),
                r#"{"type":"user","message":{"content":"usage assistant"}}"#.into(),
                "bozuk satır".into(),
            ],
        );
        // fork kopyası: aynı uuid → sayılmaz; alt ajan dosyası → sayılır
        write(
            &proj.join("b.jsonl"),
            &[line("u1", &noon(d3), "m1", "claude-opus-5-5", 0, "/w/app")],
        );
        write(
            &proj.join("b/subagents/agent-x.jsonl"),
            &[line("s1", &noon(d1), "ms", "claude-haiku-5-5", 0, "/w/app")],
        );

        let prices = PriceTable::bundled();
        let mut l = Ledger::load(&root.join("nope.json"));
        assert!(l.needs_freeze(today));
        let stats = l.backfill(root, today, &prices);
        assert_eq!(
            stats,
            BackfillStats {
                files: 3,
                messages: 3
            }
        );
        assert_eq!(l.frozen_through(), Some(d1));
        assert!(!l.needs_freeze(today));

        let opus_d1 = &l.days[&d1.to_string()]["app"]["claude-opus-5-5"];
        assert_eq!(opus_d1.tokens.output, 1_000_000);
        assert!((opus_d1.cost - (4.0 + 20.0)).abs() < 1e-9);
        assert!(!l.days.contains_key(&today.to_string()));

        // kaydet → yükle → aynı
        let path = root.join("app-data/ledger.json");
        l.save(&path).unwrap();
        let back = Ledger::load(&path);
        assert_eq!(back.frozen_through(), Some(d1));
        assert_eq!(back.days, l.days);

        // ikinci çalıştırma: yeni gün yok → hiçbir şey okunmaz
        let mut again = back;
        assert_eq!(
            again.backfill(root, today, &prices),
            BackfillStats::default()
        );

        let past = l.past_totals(today);
        let key = ("app".to_string(), "claude-opus-5-5".to_string());
        assert!(
            !past.ranges[0].contains_key(&key),
            "today aralığında geçmiş yok"
        );
        assert!(
            (past.ranges[1][&key].cost - (4.0 + 24.0)).abs() < 1e-9,
            "7d: d1 + d3"
        );
        assert_eq!(past.daily.len(), 29);
        // Haiku isteğinin bağlamı 1M > 100K eşik → uzun bağlam fiyatı (5 × $0.10)
        assert!(
            (past.daily.last().unwrap().1 - (24.0 + 0.5)).abs() < 1e-9,
            "dün: opus + haiku"
        );
    }

    #[test]
    fn corrupt_or_old_ledger_starts_fresh() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("ledger.json");
        fs::write(&p, b"{ bozuk").unwrap();
        assert_eq!(Ledger::load(&p).frozen_through(), None);
        fs::write(
            &p,
            br#"{"version":0,"frozenThrough":"2026-01-01","days":{}}"#,
        )
        .unwrap();
        assert_eq!(Ledger::load(&p).frozen_through(), None);
    }
}
