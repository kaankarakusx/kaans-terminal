//! Transcript (`*.jsonl`) okuma: dosya **tail** edilir, her satır olaylara ayrıştırılır.

use chrono::{DateTime, Local};
use serde_json::Value;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::ops::AddAssign;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Son okunan bayt konumundan devam eden okuyucu. Yarım kalan satır bir sonraki okumaya saklanır.
pub struct Tail {
    path: PathBuf,
    offset: u64,
    partial: Vec<u8>,
    mtime: Option<SystemTime>,
}

impl Tail {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            partial: Vec::new(),
            mtime: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Yeni tamamlanmış satırlar. Dosya değişmediyse diske dokunmadan boş döner.
    pub fn read_new(&mut self) -> io::Result<Vec<String>> {
        let meta = std::fs::metadata(&self.path)?;
        let mtime = meta.modified().ok();
        if meta.len() == self.offset && mtime == self.mtime {
            return Ok(Vec::new());
        }
        self.mtime = mtime;
        if meta.len() < self.offset {
            // Dosya kısaldı ya da yeniden yazıldı: baştan oku.
            self.offset = 0;
            self.partial.clear();
        }
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        self.offset += buf.len() as u64;

        self.partial.extend_from_slice(&buf);
        let Some(last_nl) = self.partial.iter().rposition(|&b| b == b'\n') else {
            return Ok(Vec::new());
        };
        let rest = self.partial.split_off(last_nl + 1);
        let complete = std::mem::replace(&mut self.partial, rest);
        Ok(complete
            .split(|&b| b == b'\n')
            .filter(|l| !l.is_empty())
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .collect())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// `cache_write` içinden 1 saatlik TTL ile yazılanlar (fiyatı farklı).
    pub cache_write_1h: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    /// Bağlam doluluğu: modele giden toplam girdi.
    pub fn context(&self) -> u64 {
        self.input + self.cache_read + self.cache_write
    }

    /// Alan alan `max(self, other)`; aynı mesajın sonraki satırları sayaç güncelleyebilir.
    pub fn max(self, o: Tokens) -> Tokens {
        Tokens {
            input: self.input.max(o.input),
            output: self.output.max(o.output),
            cache_read: self.cache_read.max(o.cache_read),
            cache_write: self.cache_write.max(o.cache_write),
            cache_write_1h: self.cache_write_1h.max(o.cache_write_1h),
        }
    }

    pub fn saturating_sub(self, o: Tokens) -> Tokens {
        Tokens {
            input: self.input.saturating_sub(o.input),
            output: self.output.saturating_sub(o.output),
            cache_read: self.cache_read.saturating_sub(o.cache_read),
            cache_write: self.cache_write.saturating_sub(o.cache_write),
            cache_write_1h: self.cache_write_1h.saturating_sub(o.cache_write_1h),
        }
    }
}

impl AddAssign for Tokens {
    fn add_assign(&mut self, o: Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
        self.cache_write_1h += o.cache_write_1h;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Prompt(String),
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        id: String,
        is_error: bool,
    },
    Reply(String),
    Usage {
        msg_id: String,
        model: String,
        tokens: Tokens,
    },
    Title(String),
    LastPrompt(String),
    // ── Codex (ve ileride diğer araçlar) ──
    /// Oturum kimliği ve klasörü (Codex `session_meta`)
    Session {
        id: String,
        cwd: String,
    },
    /// Model (Codex `turn_context`)
    Model(String),
    TurnStart,
    TurnEnd,
    /// Modelin bağlam penceresi
    ContextWindow(u64),
    /// Son istemin bağlam doluluğu (token)
    ContextTokens(u64),
    /// Hesap limitleri (Codex `token_count.rate_limits`)
    Limits(RateLimits),
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitWindow {
    pub used_pct: f64,
    pub window_minutes: u64,
    /// Unix saniyesi
    pub resets_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimits {
    pub primary: Option<LimitWindow>,
    pub secondary: Option<LimitWindow>,
}

#[derive(Debug, Default)]
pub struct Line {
    pub uuid: Option<String>,
    pub ts: Option<DateTime<Local>>,
    pub cwd: Option<String>,
    pub entrypoint: Option<String>,
    /// Assistant satırındaki effort seviyesi ("low" … "max")
    pub effort: Option<String>,
    pub items: Vec<Item>,
}

/// Kullanıcı mesajı gibi görünen ama prompt olmayan (komut çıktısı, sistem notu) metinler.
fn is_synthetic_prompt(text: &str) -> bool {
    const PREFIXES: [&str; 7] = [
        "<local-command-stdout",
        "<local-command-stderr",
        "<local-command-caveat",
        "<bash-stdout",
        "<bash-stderr",
        "<system-reminder",
        "<task-notification",
    ];
    let t = text.trim_start();
    t.is_empty() || PREFIXES.iter().any(|p| t.starts_with(p))
}

pub(crate) fn tokens_of(usage: &Value) -> Tokens {
    let n = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
    Tokens {
        input: n("input_tokens"),
        output: n("output_tokens"),
        cache_read: n("cache_read_input_tokens"),
        cache_write: n("cache_creation_input_tokens"),
        cache_write_1h: usage
            .pointer("/cache_creation/ephemeral_1h_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    }
}

/// Bir transcript satırını ayrıştırır. Tanınmayan ya da bozuk satırlar `None`.
pub fn parse_line(raw: &str) -> Option<Line> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let mut line = Line {
        uuid: s("uuid"),
        ts: v
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            .map(|t| t.with_timezone(&Local)),
        cwd: s("cwd"),
        entrypoint: s("entrypoint"),
        effort: s("effort"),
        items: Vec::new(),
    };

    match v.get("type").and_then(Value::as_str)? {
        "ai-title" => line.items.extend(s("aiTitle").map(Item::Title)),
        "last-prompt" => line.items.extend(s("lastPrompt").map(Item::LastPrompt)),
        "user" => {
            if v.get("isMeta").and_then(Value::as_bool) == Some(true) {
                return Some(line);
            }
            match v.pointer("/message/content")? {
                Value::String(text) if !is_synthetic_prompt(text) => {
                    line.items.push(Item::Prompt(text.clone()));
                }
                Value::Array(blocks) => {
                    let mut prompt = String::new();
                    for b in blocks {
                        match b.get("type").and_then(Value::as_str) {
                            Some("tool_result") => {
                                if let Some(id) = b.get("tool_use_id").and_then(Value::as_str) {
                                    line.items.push(Item::ToolResult {
                                        id: id.to_string(),
                                        is_error: b.get("is_error").and_then(Value::as_bool)
                                            == Some(true),
                                    });
                                }
                            }
                            Some("text") => {
                                if let Some(t) = b.get("text").and_then(Value::as_str) {
                                    if !prompt.is_empty() {
                                        prompt.push('\n');
                                    }
                                    prompt.push_str(t);
                                }
                            }
                            _ => {}
                        }
                    }
                    if !is_synthetic_prompt(&prompt) {
                        line.items.push(Item::Prompt(prompt));
                    }
                }
                _ => {}
            }
        }
        "assistant" => {
            let msg = v.get("message")?;
            for b in msg
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                match b.get("type").and_then(Value::as_str) {
                    Some("tool_use" | "server_tool_use") => line.items.push(Item::ToolUse {
                        id: b
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        name: b
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("?")
                            .to_string(),
                        input: b.get("input").cloned().unwrap_or(Value::Null),
                    }),
                    Some("text") => {
                        if let Some(t) = b.get("text").and_then(Value::as_str)
                            && !t.trim().is_empty()
                        {
                            line.items.push(Item::Reply(t.to_string()));
                        }
                    }
                    _ => {}
                }
            }
            let model = msg.get("model").and_then(Value::as_str).unwrap_or_default();
            if let (Some(usage), Some(id)) =
                (msg.get("usage"), msg.get("id").and_then(Value::as_str))
                && model != "<synthetic>"
            {
                line.items.push(Item::Usage {
                    msg_id: id.to_string(),
                    model: model.to_string(),
                    tokens: tokens_of(usage),
                });
            }
        }
        _ => {}
    }
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn tail_keeps_partial_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("t.jsonl");
        let mut f = File::create(&p).unwrap();
        f.write_all(b"{\"a\":1}\n{\"b\":").unwrap();
        f.flush().unwrap();
        let mut t = Tail::new(p.clone());
        assert_eq!(t.read_new().unwrap(), vec!["{\"a\":1}"]);
        assert!(t.read_new().unwrap().is_empty());
        f.write_all("2,\"ğ\":1}\n".as_bytes()).unwrap();
        f.flush().unwrap();
        assert_eq!(t.read_new().unwrap(), vec!["{\"b\":2,\"ğ\":1}"]);
    }

    #[test]
    fn tail_restarts_when_file_shrinks() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("t.jsonl");
        std::fs::write(&p, "aaaaaaaa\nbbbbbbbb\n").unwrap();
        let mut t = Tail::new(p.clone());
        assert_eq!(t.read_new().unwrap().len(), 2);
        std::fs::write(&p, "c\n").unwrap();
        assert_eq!(t.read_new().unwrap(), vec!["c"]);
    }

    #[test]
    fn parses_prompts_and_skips_synthetic_ones() {
        let l = parse_line(r#"{"type":"user","uuid":"u1","timestamp":"2026-11-08T10:00:00Z","message":{"content":"fix the bug"}}"#).unwrap();
        assert_eq!(l.items, vec![Item::Prompt("fix the bug".into())]);
        assert!(l.ts.is_some());
        let l = parse_line(r#"{"type":"user","message":{"content":"<local-command-stdout>ok</local-command-stdout>"}}"#).unwrap();
        assert!(l.items.is_empty());
        let l =
            parse_line(r#"{"type":"user","isMeta":true,"message":{"content":"meta"}}"#).unwrap();
        assert!(l.items.is_empty());
    }

    #[test]
    fn parses_tool_results() {
        let l = parse_line(r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","is_error":true,"content":"boom"}]}}"#).unwrap();
        assert_eq!(
            l.items,
            vec![Item::ToolResult {
                id: "t1".into(),
                is_error: true
            }]
        );
    }

    #[test]
    fn parses_assistant_blocks_and_usage() {
        let l = parse_line(r#"{"type":"assistant","message":{"id":"m1","model":"claude-opus-5-5","content":[{"type":"text","text":"Done."},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}],"usage":{"input_tokens":2,"output_tokens":10,"cache_read_input_tokens":100,"cache_creation_input_tokens":50,"cache_creation":{"ephemeral_1h_input_tokens":30}}}}"#).unwrap();
        assert_eq!(l.items.len(), 3);
        assert_eq!(l.items[0], Item::Reply("Done.".into()));
        assert!(matches!(&l.items[1], Item::ToolUse { name, .. } if name == "Bash"));
        let Item::Usage { tokens, model, .. } = &l.items[2] else {
            panic!()
        };
        assert_eq!(model, "claude-opus-5-5");
        assert_eq!(tokens.total(), 162);
        assert_eq!(tokens.context(), 152);
        assert_eq!(tokens.cache_write_1h, 30);
        let l =
            parse_line(r#"{"type":"assistant","effort":"max","message":{"id":"m","content":[]}}"#)
                .unwrap();
        assert_eq!(l.effort.as_deref(), Some("max"));
    }

    #[test]
    fn ignores_garbage() {
        assert!(parse_line("not json").is_none());
        assert!(parse_line(r#"{"no":"type"}"#).is_none());
    }
}
