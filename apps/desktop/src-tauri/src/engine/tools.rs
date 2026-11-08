//! Araç adı → kategori ve araç çağrısının "hedefi" (OZELLIKLER.md §1.3).

use serde_json::Value;

/// Kategoriler: web, browser, read, search, edit, bash, agent, mcp, plan, other.
pub fn category(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("mcp__") {
        if lower.contains("chrome") || lower.contains("browser") || lower.contains("playwright") {
            return "browser";
        }
        return "mcp";
    }
    match name {
        "WebFetch" | "WebSearch" | "web_fetch" | "search_web" => "web",
        "Read" | "NotebookRead" | "LS" | "view_file" | "list_dir" => "read",
        "Grep" | "Glob" | "ToolSearch" | "grep_search" | "codebase_search" => "search",
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => "edit",
        "Bash" | "BashOutput" | "KillShell" | "KillBash" | "Monitor" | "run_command"
        | "read_terminal" => "bash",
        "Agent" | "Task" | "SendMessage" | "invoke_subagent" => "agent",
        "TodoWrite" | "EnterPlanMode" | "ExitPlanMode" | "TaskCreate" | "TaskUpdate"
        | "TaskList" | "AskUserQuestion" => "plan",
        _ => "other",
    }
}

/// Bu araç bir dosyaya yazar mı? (same file ve files.heat için)
pub fn is_write(name: &str) -> bool {
    matches!(name, "Edit" | "MultiEdit" | "Write" | "NotebookEdit")
}

/// Araç çağrısının kısa hedefi: komut, dosya yolu, desen, URL…
/// Döndürülen metin henüz maskelenmemiştir; arayüze göndermeden önce `redact()`.
pub fn target(name: &str, input: &Value) -> Option<String> {
    let s = |k: &str| input.get(k).and_then(Value::as_str).map(str::to_string);
    match name {
        "Bash" => s("command"),
        "Read" | "Edit" | "MultiEdit" | "Write" => s("file_path"),
        "NotebookEdit" | "NotebookRead" => s("notebook_path"),
        "Grep" | "Glob" => s("pattern"),
        "WebFetch" => s("url"),
        "WebSearch" => s("query"),
        "Agent" | "Task" => s("description"),
        "LS" => s("path"),
        _ => [
            "file_path",
            "path",
            "url",
            "query",
            "command",
            "description",
            "pattern",
        ]
        .iter()
        .find_map(|k| s(k)),
    }
}

/// Dosya yolu taşıyan araçların yolu (files.heat için).
pub fn file_path(name: &str, input: &Value) -> Option<String> {
    let key = match name {
        "Read" | "Edit" | "MultiEdit" | "Write" => "file_path",
        "NotebookEdit" | "NotebookRead" => "notebook_path",
        _ => return None,
    };
    input.get(key).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn categories() {
        assert_eq!(category("WebSearch"), "web");
        assert_eq!(category("Bash"), "bash");
        assert_eq!(category("run_command"), "bash");
        assert_eq!(category("Edit"), "edit");
        assert_eq!(category("Grep"), "search");
        assert_eq!(category("Agent"), "agent");
        assert_eq!(category("mcp__github__create_issue"), "mcp");
        assert_eq!(category("mcp__claude-in-chrome__navigate"), "browser");
        assert_eq!(category("TodoWrite"), "plan");
        assert_eq!(category("SomethingNew"), "other");
    }

    #[test]
    fn targets() {
        assert_eq!(
            target("Bash", &json!({"command": "ls -la"})).as_deref(),
            Some("ls -la")
        );
        assert_eq!(
            target("Read", &json!({"file_path": "/a/b.rs"})).as_deref(),
            Some("/a/b.rs")
        );
        assert_eq!(
            target("mcp__x__y", &json!({"url": "https://x"})).as_deref(),
            Some("https://x")
        );
        assert_eq!(target("Bash", &json!({})), None);
        assert_eq!(file_path("Grep", &json!({"file_path": "/x"})), None);
    }
}
