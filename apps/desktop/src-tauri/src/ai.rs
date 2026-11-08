//! Yapay zekâ yardımcıları: kullanıcının kendi Claude Code'u `claude -p` ile çalıştırılır (onun girişi/aboneliği
//! geçerli; API anahtarı ya da token'a dokunmayız). Şimdilik yalnızca Source Control'deki AI commit mesajı.
//!
//! - `--bare` KULLANILMAZ: o kip yalnızca `ANTHROPIC_API_KEY` ile çalışır, aboneliği görmez.
//! - `--no-session-persistence`: transcript yazılmaz. Kayıt dosyası (`sessions/<pid>.json`) yine yazılır; engine
//!   bu pid'leri `mark_own_helper` ile sohbet listesinden gizler.
//! - `--tools ""`: araç yok; yalnızca stdin'deki metni okuyup cevap verir.
//! - Ortamdan Claude Code'un iç değişkenleri temizlenir (uygulama bir Claude oturumundan başlatıldıysa alt oturum
//!   sanmasın).

use crate::engine::mark_own_helper;
use crate::engine::redact::redact;
use crate::host::server::is_claude_internal;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(90);
const MODEL: &str = "haiku";

const COMMIT_PROMPT: &str = "Write a git commit message for the changes on stdin. \
Use Conventional Commits (feat:, fix:, refactor:, docs:, chore:, …) unless the recent subjects clearly use another \
style. First line: imperative, under 72 characters. If the change has several parts, add a blank line and up to 4 \
short bullet lines starting with \"- \". Describe what changed and why, not file-by-file noise. Output only the \
message: no code fences, no quotes, no preamble.";

/// Kullanıcının `claude` komutu: bilinen kurulum yerleri, sonra PATH (Finder'dan açılan uygulamanın PATH'i kısadır).
fn claude_bin() -> Option<PathBuf> {
    let home = dirs_home();
    let mut c: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        if let Some(h) = &home {
            c.push(h.join(".local/bin/claude.exe"));
        }
        if let Some(a) = std::env::var_os("APPDATA") {
            c.push(PathBuf::from(a).join("npm/claude.cmd"));
        }
    } else {
        if let Some(h) = &home {
            c.push(h.join(".local/bin/claude"));
            c.push(h.join(".claude/local/claude"));
        }
        c.push("/opt/homebrew/bin/claude".into());
        c.push("/usr/local/bin/claude".into());
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if cfg!(windows) {
                c.push(dir.join("claude.exe"));
                c.push(dir.join("claude.cmd"));
            } else {
                c.push(dir.join("claude"));
            }
        }
    }
    c.into_iter().find(|p| p.is_file())
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Seçili repodaki değişikliklerden commit mesajı.
pub fn commit_message(root: &str) -> Result<String, String> {
    let (dir, context) = crate::git::changes_for_ai(root)?;
    let raw = run_claude(&dir, COMMIT_PROMPT, &context)?;
    let msg = tidy_message(&raw);
    if msg.is_empty() {
        return Err("Claude returned an empty message".into());
    }
    Ok(msg)
}

fn run_claude(dir: &Path, prompt: &str, stdin: &str) -> Result<String, String> {
    let bin = claude_bin()
        .ok_or("Claude Code not found: install it, then run claude and /login once.")?;
    let mut cmd = Command::new(bin);
    // İstem değişken sayılı `--tools`'tan önce gelmeli (yoksa araç adı sanılır).
    cmd.arg("-p")
        .arg(prompt)
        .args(["--no-session-persistence", "--model", MODEL, "--tools", ""])
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, _) in std::env::vars_os() {
        if k.to_str().is_some_and(is_claude_internal) {
            cmd.env_remove(k);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("can't start claude: {e}"))?;
    let pid = child.id();
    mark_own_helper(pid, true);
    let result = (|| {
        let mut input = child.stdin.take().ok_or("stdin")?;
        let text = stdin.to_owned();
        let in_t = std::thread::spawn(move || {
            let _ = input.write_all(text.as_bytes());
        });
        let mut so = child.stdout.take().ok_or("stdout")?;
        let mut se = child.stderr.take().ok_or("stderr")?;
        let out_t = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = so.read_to_end(&mut b);
            b
        });
        let err_t = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = se.read_to_end(&mut b);
            b
        });
        let deadline = Instant::now() + TIMEOUT;
        let status = loop {
            if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
                break st;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Claude took too long (90 s)".to_string());
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let _ = in_t.join();
        let out = String::from_utf8_lossy(&out_t.join().unwrap_or_default()).into_owned();
        let err = String::from_utf8_lossy(&err_t.join().unwrap_or_default()).into_owned();
        if status.success() && !out.trim().is_empty() {
            Ok(out)
        } else {
            Err(friendly_error(&format!("{err}\n{out}")))
        }
    })();
    mark_own_helper(pid, false);
    result
}

fn friendly_error(text: &str) -> String {
    let t = text.to_ascii_lowercase();
    if t.contains("/login")
        || t.contains("not logged in")
        || t.contains("invalid api key")
        || t.contains("authentication")
    {
        return "Claude Code isn't signed in: open a terminal, run claude and /login.".into();
    }
    if t.contains("usage limit") || t.contains("rate limit") || t.contains("limit reached") {
        return "Claude usage limit reached; try again after it resets.".into();
    }
    let line = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    let out: String = redact(&line).chars().take(300).collect();
    if out.is_empty() {
        "claude failed".into()
    } else {
        out
    }
}

/// Kod çiti, tırnak ve "Commit message:" gibi önekleri temizler.
fn tidy_message(raw: &str) -> String {
    let mut lines: Vec<&str> = raw.trim().lines().collect();
    if lines
        .first()
        .is_some_and(|l| l.trim_start().starts_with("```"))
    {
        lines.remove(0);
    }
    if lines.last().is_some_and(|l| l.trim() == "```") {
        lines.pop();
    }
    let mut text = lines.join("\n").trim().to_string();
    for prefix in ["Commit message:", "commit message:"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.trim().to_string();
        }
    }
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        text = text[1..text.len() - 1].trim().to_string();
    }
    text.chars().take(2000).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tidies_model_output() {
        assert_eq!(tidy_message("```\nfeat: x\n\n- a\n```\n"), "feat: x\n\n- a");
        assert_eq!(tidy_message("Commit message: fix: y"), "fix: y");
        assert_eq!(tidy_message("\"chore: z\""), "chore: z");
        assert_eq!(tidy_message("  feat: plain  \n"), "feat: plain");
    }

    #[test]
    fn explains_sign_in_errors() {
        assert!(friendly_error("Invalid API key · Please run /login").contains("/login"));
        assert!(friendly_error("").contains("claude failed"));
    }
}
