//! Codex rollout'ları (`$CODEX_HOME/sessions/YYYY/MM/DD/rollout-*.jsonl`, OZELLIKLER.md §1.1, MIMARI.md §3.3).
//! Her satır `{timestamp, type, payload}`; engine'in ortak `Item` akışına çevrilir.
//! Codex token'ları sayılır ama fiyatlandırılmaz. Limitler burada yaklaşıktır (asıl kaynak Codex app-server).

use super::transcript::{Item, LimitWindow, Line, RateLimits, Tokens};
use chrono::{DateTime, Local};
use serde_json::Value;
use std::path::PathBuf;

/// `CODEX_HOME`, yoksa `~/.codex`.
pub fn codex_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("CODEX_HOME").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    std::env::home_dir().map(|h| h.join(".codex"))
}

/// Codex araç adı → ortak araç adı (kategori ve hedef ortak koddan gelsin).
pub fn tool_name(name: &str) -> &str {
    match name {
        "exec" | "exec_command" | "local_shell" | "local_shell_call" | "shell" | "write_stdin"
        | "js" => "Bash",
        "apply_patch" => "Edit",
        "web_search_call" | "web_search" => "WebSearch",
        "request_user_input" => "AskUserQuestion",
        "update_plan" | "create_goal" | "get_goal" => "TodoWrite",
        "spawn_agent" => "Agent",
        "tool_search_call" | "tool_search" => "ToolSearch",
        "read_thread" | "list_threads" | "read_thread_terminal" => "Read",
        other => other,
    }
}

/// Araç girdisini ortak biçime getirir: komut → `command`, yama → ilk dosyanın `file_path`'i.
fn normalize_input(name: &str, raw: &Value) -> Value {
    // function_call: arguments bir JSON metni; custom_tool_call: input düz metin (örn. yama).
    let parsed = match raw {
        Value::String(s) => {
            serde_json::from_str::<Value>(s).unwrap_or_else(|_| Value::String(s.clone()))
        }
        v => v.clone(),
    };
    match tool_name(name) {
        "Bash" => {
            let cmd = parsed
                .get("cmd")
                .or_else(|| parsed.get("command"))
                .map(|c| match c {
                    Value::Array(a) => a
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" "),
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .or_else(|| {
                    parsed
                        .get("chars")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .or_else(|| parsed.as_str().map(str::to_string))
                .unwrap_or_default();
            serde_json::json!({ "command": cmd })
        }
        "Edit" => {
            let text = parsed.as_str().map(str::to_string).or_else(|| {
                parsed
                    .get("input")
                    .or_else(|| parsed.get("patch"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
            let path = text.as_deref().and_then(|t| {
                t.lines().find_map(|l| {
                    ["*** Update File: ", "*** Add File: ", "*** Delete File: "]
                        .iter()
                        .find_map(|p| l.strip_prefix(p))
                        .map(|p| p.trim().to_string())
                })
            });
            serde_json::json!({ "file_path": path })
        }
        "WebSearch" => {
            serde_json::json!({ "query": parsed.get("query").and_then(Value::as_str).unwrap_or_default() })
        }
        _ => parsed,
    }
}

fn limit(v: &Value) -> Option<LimitWindow> {
    Some(LimitWindow {
        used_pct: v.get("used_percent")?.as_f64()?,
        window_minutes: v.get("window_minutes").and_then(Value::as_u64).unwrap_or(0),
        resets_at: v.get("resets_at").and_then(Value::as_i64).unwrap_or(0),
    })
}

fn texts(content: Option<&Value>) -> String {
    content
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub fn parse_line(raw: &str) -> Option<Line> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let p = v.get("payload")?;
    let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).map(str::to_string);
    let mut line = Line {
        uuid: None,
        ts: v
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            .map(|t| t.with_timezone(&Local)),
        cwd: None,
        entrypoint: None,
        effort: None,
        items: Vec::new(),
    };
    let ptype = p.get("type").and_then(Value::as_str).unwrap_or_default();
    match (v.get("type").and_then(Value::as_str)?, ptype) {
        ("session_meta", _) => {
            let id = s(p, "id")
                .or_else(|| s(p, "session_id"))
                .unwrap_or_default();
            let cwd = s(p, "cwd").unwrap_or_default();
            line.cwd = Some(cwd.clone());
            line.items.push(Item::Session { id, cwd });
        }
        ("turn_context", _) => {
            line.cwd = s(p, "cwd");
            if let Some(m) = s(p, "model") {
                line.items.push(Item::Model(m));
            }
        }
        ("event_msg", "task_started") => {
            line.items.push(Item::TurnStart);
            if let Some(w) = p.get("model_context_window").and_then(Value::as_u64) {
                line.items.push(Item::ContextWindow(w));
            }
        }
        ("event_msg", "task_complete" | "turn_aborted") => line.items.push(Item::TurnEnd),
        ("event_msg", "token_count") => {
            if let Some(info) = p.get("info") {
                if let Some(w) = info.get("model_context_window").and_then(Value::as_u64) {
                    line.items.push(Item::ContextWindow(w));
                }
                if let Some(last) = info.get("last_token_usage") {
                    let n = |k: &str| last.get(k).and_then(Value::as_u64).unwrap_or(0);
                    line.items
                        .push(Item::ContextTokens(n("input_tokens") + n("output_tokens")));
                }
            }
            if let Some(rl) = p.get("rate_limits") {
                let l = RateLimits {
                    primary: rl.get("primary").and_then(limit),
                    secondary: rl.get("secondary").and_then(limit),
                };
                if l.primary.is_some() || l.secondary.is_some() {
                    line.items.push(Item::Limits(l));
                }
            }
        }
        ("token_usage_record", _) => {
            let u = p.get("usage")?;
            let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
            // OpenAI'de input_tokens önbellekten okunanları da içerir.
            let cached = n("cached_input_tokens");
            line.items.push(Item::Usage {
                msg_id: s(p, "response_id").unwrap_or_else(|| raw.len().to_string()),
                model: String::new(),
                tokens: Tokens {
                    input: n("input_tokens").saturating_sub(cached),
                    output: n("output_tokens"),
                    cache_read: cached,
                    cache_write: n("cache_write_input_tokens"),
                    cache_write_1h: 0,
                },
            });
        }
        ("response_item", "message") => {
            let text = texts(p.get("content"));
            match p.get("role").and_then(Value::as_str) {
                // Codex ortam bilgisini ve AGENTS.md'yi de "user" mesajı olarak ekler; bunlar < ile başlar.
                Some("user") if !text.trim_start().starts_with('<') && !text.trim().is_empty() => {
                    line.items.push(Item::Prompt(text));
                }
                Some("assistant") if !text.trim().is_empty() => line.items.push(Item::Reply(text)),
                _ => {}
            }
        }
        ("response_item", "function_call" | "custom_tool_call") => {
            let name = s(p, "name").unwrap_or_else(|| "?".into());
            let raw_input = p
                .get("arguments")
                .or_else(|| p.get("input"))
                .cloned()
                .unwrap_or(Value::Null);
            line.items.push(Item::ToolUse {
                id: s(p, "call_id").unwrap_or_default(),
                input: normalize_input(&name, &raw_input),
                name: tool_name(&name).to_string(),
            });
        }
        ("response_item", "function_call_output" | "custom_tool_call_output") => {
            // Çıkış kodu metnin içinde: "Exit code: 1" ya da {"metadata":{"exit_code":1}}
            let out = match p.get("output") {
                Some(Value::String(s)) => s.clone(),
                Some(o) => texts(Some(o)),
                None => String::new(),
            };
            let is_error = out.contains("Exit code: ") && !out.contains("Exit code: 0")
                || out.contains("\"exit_code\":") && !out.contains("\"exit_code\":0");
            line.items.push(Item::ToolResult {
                id: s(p, "call_id").unwrap_or_default(),
                is_error,
            });
        }
        ("response_item", "web_search_call") => {
            let q = p
                .pointer("/action/query")
                .and_then(Value::as_str)
                .unwrap_or_default();
            line.items.push(Item::ToolUse {
                id: String::new(),
                name: "WebSearch".into(),
                input: serde_json::json!({ "query": q }),
            });
        }
        _ => {}
    }
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn items(v: Value) -> Vec<Item> {
        parse_line(&v.to_string()).unwrap().items
    }

    #[test]
    fn session_turns_and_model() {
        assert_eq!(
            items(
                json!({"timestamp":"2026-11-08T10:00:00Z","type":"session_meta","payload":{"id":"abc","cwd":"/w/x"}})
            ),
            vec![Item::Session {
                id: "abc".into(),
                cwd: "/w/x".into()
            }]
        );
        assert_eq!(
            items(json!({"type":"turn_context","payload":{"model":"gpt-5.5","cwd":"/w"}})),
            vec![Item::Model("gpt-5.5".into())]
        );
        assert_eq!(
            items(
                json!({"type":"event_msg","payload":{"type":"task_started","model_context_window":258400}})
            ),
            vec![Item::TurnStart, Item::ContextWindow(258_400)]
        );
        assert_eq!(
            items(json!({"type":"event_msg","payload":{"type":"task_complete"}})),
            vec![Item::TurnEnd]
        );
        assert_eq!(
            items(json!({"type":"event_msg","payload":{"type":"turn_aborted"}})),
            vec![Item::TurnEnd]
        );
    }

    #[test]
    fn prompts_skip_injected_context() {
        let msg = |role: &str, text: &str| json!({"type":"response_item","payload":{"type":"message","role":role,"content":[{"type":"input_text","text":text}]}});
        assert_eq!(
            items(msg("user", "fix the tests")),
            vec![Item::Prompt("fix the tests".into())]
        );
        assert!(items(msg("user", "<environment_context>…</environment_context>")).is_empty());
        assert!(items(msg("developer", "rules")).is_empty());
        assert_eq!(
            items(msg("assistant", "Done.")),
            vec![Item::Reply("Done.".into())]
        );
    }

    #[test]
    fn tools_map_to_common_names() {
        let it = items(
            json!({"type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":"c1","arguments":"{\"cmd\":[\"cargo\",\"test\"]}"}}),
        );
        assert_eq!(
            it,
            vec![Item::ToolUse {
                id: "c1".into(),
                name: "Bash".into(),
                input: json!({"command":"cargo test"})
            }]
        );
        let it = items(
            json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","call_id":"c2","input":"*** Begin Patch\n*** Update File: src/a.rs\n@@\n-x\n+y\n*** End Patch"}}),
        );
        assert_eq!(
            it,
            vec![Item::ToolUse {
                id: "c2".into(),
                name: "Edit".into(),
                input: json!({"file_path":"src/a.rs"})
            }]
        );
        let it = items(
            json!({"type":"response_item","payload":{"type":"web_search_call","action":{"type":"search","query":"tauri"}}}),
        );
        assert!(matches!(&it[0], Item::ToolUse { name, .. } if name == "WebSearch"));
        assert_eq!(
            items(
                json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"c1","output":"Exit code: 1\nboom"}})
            ),
            vec![Item::ToolResult {
                id: "c1".into(),
                is_error: true
            }]
        );
        assert_eq!(
            items(
                json!({"type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"c2","output":[{"type":"input_text","text":"Exit code: 0"}]}})
            ),
            vec![Item::ToolResult {
                id: "c2".into(),
                is_error: false
            }]
        );
    }

    #[test]
    fn usage_context_and_limits() {
        let it = items(
            json!({"type":"token_usage_record","payload":{"response_id":"r1","usage":{"input_tokens":1000,"cached_input_tokens":800,"output_tokens":50,"cache_write_input_tokens":0}}}),
        );
        let Item::Usage { msg_id, tokens, .. } = &it[0] else {
            panic!()
        };
        assert_eq!(msg_id, "r1");
        assert_eq!(
            (tokens.input, tokens.cache_read, tokens.output),
            (200, 800, 50)
        );
        let it = items(json!({"type":"event_msg","payload":{"type":"token_count",
            "info":{"model_context_window":258400,"last_token_usage":{"input_tokens":9000,"output_tokens":100}},
            "rate_limits":{"primary":{"used_percent":12.5,"window_minutes":300,"resets_at":1794150000},"secondary":{"used_percent":40.0,"window_minutes":10080,"resets_at":1794500000}}}}));
        assert_eq!(it[0], Item::ContextWindow(258_400));
        assert_eq!(it[1], Item::ContextTokens(9_100));
        let Item::Limits(l) = it[2] else { panic!() };
        assert_eq!(l.primary.unwrap().window_minutes, 300);
        assert!((l.secondary.unwrap().used_pct - 40.0).abs() < 1e-9);
    }
}
