//! Source Control (OZELLIKLER.md §4): sohbetlerin ve terminallerin çalıştığı repolar.
//!
//! İşlemler kullanıcının kendi `git`'iyle yapılır (VS Code gibi): kendi ayarları, kimlik bilgisi yardımcısı,
//! hook'ları ve imzası geçerli olur. Etkileşimli soru sorulmaz (`GIT_TERMINAL_PROMPT=0`); takılan komut süre
//! dolunca sonlandırılır. Komutlar yalnızca kullanıcı paneli kullanırken çalışır (döngüde değil).
use crate::engine::redact::{redact, redact_code};
use regex::Regex;
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{LazyLock, OnceLock};
use std::time::{Duration, Instant};

/// Yerel komutlar (status, add, commit…)
const LOCAL: Duration = Duration::from_secs(30);
/// Ağ komutları (push, pull)
const NETWORK: Duration = Duration::from_secs(120);
/// Arayüze gönderilen en fazla değişiklik (dev bir `node_modules` eklenmiş olabilir)
const MAX_CHANGES: usize = 2000;
const MAX_LOG: usize = 400;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoRef {
    pub root: String,
    pub name: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// Repo köküne göre yol (işlemlerde kimlik olarak kullanılır)
    pub path: String,
    /// Yeniden adlandırmada eski yol
    pub orig: Option<String>,
    /// M A D R C T U ? (VS Code harfleri)
    pub code: char,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub root: String,
    /// Dal adı; ayrık HEAD'de kısa commit
    pub branch: String,
    pub detached: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Henüz commit yok
    pub initial: bool,
    pub staged: Vec<Change>,
    pub changes: Vec<Change>,
    pub conflicts: Vec<Change>,
    /// Liste MAX_CHANGES'te kesildi
    pub truncated: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub parents: Vec<String>,
    pub author: String,
    /// ms
    pub time: i64,
    /// HEAD -> main, origin/main, tag: v1 …
    pub refs: Vec<String>,
    pub subject: String,
}

// ---------------------------------------------------------------- git'i bulma ve çalıştırma

/// Kullanıcının git'i. Mac'te `/usr/bin/git` Xcode araçları kurulu değilse kurulum penceresi açan bir kabuktur:
/// önce gerçek kopyalar aranır, `/usr/bin/git` yalnızca araçlar kuruluysa kullanılır.
fn git_bin() -> Option<&'static Path> {
    static BIN: OnceLock<Option<PathBuf>> = OnceLock::new();
    BIN.get_or_init(|| {
        let exe = if cfg!(windows) { "git.exe" } else { "git" };
        let mut candidates: Vec<PathBuf> = Vec::new();
        #[cfg(target_os = "macos")]
        {
            candidates.push("/opt/homebrew/bin/git".into());
            candidates.push("/usr/local/bin/git".into());
            if Path::new("/Library/Developer/CommandLineTools/usr/bin/git").exists()
                || Path::new("/Applications/Xcode.app").exists()
            {
                candidates.push("/usr/bin/git".into());
            }
        }
        #[cfg(windows)]
        {
            candidates.push(r"C:\Program Files\Git\cmd\git.exe".into());
        }
        if let Some(path) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path) {
                let p = dir.join(exe);
                // Mac'te PATH'teki /usr/bin/git de aynı kabuk olabilir: yukarıdaki kontrol yeterli.
                if cfg!(target_os = "macos") && p == Path::new("/usr/bin/git") {
                    continue;
                }
                candidates.push(p);
            }
        }
        candidates.into_iter().find(|p| p.is_file())
    })
    .as_deref()
}

/// Sık hataların anlaşılır karşılığı (ham metin yerine ne yapılacağını söyler).
fn friendly(text: &str) -> Option<String> {
    static DENIED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"Permission to (\S+?)(?:\.git)? denied to ([^.\s]+)").expect("denied regex")
    });
    if let Some(c) = DENIED.captures(text) {
        return Some(format!(
            "{} can't push to {}: no write access. Ask the owner to add you as a collaborator, or push to your fork.",
            &c[2], &c[1]
        ));
    }
    let t = text.to_ascii_lowercase();
    if t.contains("could not read username")
        || t.contains("authentication failed")
        || t.contains("terminal prompts disabled")
    {
        return Some("Sign-in needed: git has no saved credentials for this remote. Push once from a terminal to sign in.".into());
    }
    if t.contains("updates were rejected")
        || t.contains("non-fast-forward")
        || t.contains("fetch first")
    {
        return Some("The remote has commits you don't have. Use sync (pull, then push).".into());
    }
    if t.contains("could not resolve host") || t.contains("network is unreachable") {
        return Some("Can't reach the remote: check your internet connection.".into());
    }
    if t.contains("please tell me who you are") {
        return Some("Git doesn't know your name/email yet: run git config --global user.name and user.email once.".into());
    }
    None
}

/// Hata metni arayüze gider: maskelenir, URL'deki kullanıcı:parola gizlenir.
fn tidy_error(text: &str) -> String {
    if let Some(f) = friendly(text) {
        return redact(&f).into_owned();
    }
    static USERINFO: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b([a-z][a-z0-9+.-]*://)[^/@\s]+@").expect("userinfo regex")
    });
    let masked = USERINFO.replace_all(text, "$1•••@");
    let line = masked
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("hint:"))
        .collect::<Vec<_>>()
        .join(" · ");
    let out = redact(&line).into_owned();
    if out.is_empty() {
        "git failed".into()
    } else {
        out.chars().take(400).collect()
    }
}

/// `git -C <root> <args>`; süre dolarsa süreç sonlandırılır.
fn run(root: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let bin = git_bin().ok_or("git not found")?;
    let mut cmd = Command::new(bin);
    cmd.arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_MERGE_AUTOEDIT", "no")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    // Çıktılar ayrı iş parçacıklarında okunur: boru dolup git beklemesin.
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
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("git {} timed out", args.first().unwrap_or(&"")));
        }
        std::thread::sleep(Duration::from_millis(15));
    };
    let stdout = out_t.join().unwrap_or_default();
    let stderr = err_t.join().unwrap_or_default();
    if status.success() {
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    } else {
        let mut text = String::from_utf8_lossy(&stderr).into_owned();
        if text.trim().is_empty() {
            text = String::from_utf8_lossy(&stdout).into_owned();
        }
        Err(tidy_error(&text))
    }
}

// ---------------------------------------------------------------- repo bulma

/// Klasörün repo kökü (`.git` klasörü ya da worktree'deki `.git` dosyası), yukarı doğru aranır.
pub fn repo_root(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(".git").exists() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

/// Klasörlerin repoları (tekrarsız, sıra korunur). Ev klasörünün kendisi repo sayılmaz (dotfile reposu).
pub fn repos_of(cwds: &[String]) -> Vec<RepoRef> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut out: Vec<RepoRef> = Vec::new();
    for cwd in cwds {
        let Some(root) = repo_root(Path::new(cwd)) else {
            continue;
        };
        if home.as_deref() == Some(root.as_path()) {
            continue;
        }
        let root_s = root.to_string_lossy().into_owned();
        if out.iter().any(|r| r.root == root_s) {
            continue;
        }
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root_s.clone());
        out.push(RepoRef { root: root_s, name });
    }
    out
}

/// Arayüzden gelen kök gerçekten bir repo mu (rastgele klasörde git çalıştırılmasın).
fn checked_root(root: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(root);
    if p.is_absolute() && p.join(".git").exists() {
        Ok(p)
    } else {
        Err("not a repository".into())
    }
}

/// Yollar köke göre olmalı; `..` ya da mutlak yol kabul edilmez.
fn checked_paths(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("no files".into());
    }
    for p in paths {
        let path = Path::new(p);
        if p.is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!("bad path: {p}"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- status

/// `git status --porcelain=v2 --branch -z` çıktısı.
pub fn parse_status(root: &str, raw: &str) -> RepoStatus {
    let mut st = RepoStatus {
        root: root.into(),
        ..Default::default()
    };
    let mut items = raw.split('\0').filter(|s| !s.is_empty());
    let mut count = 0usize;
    let mut push = |list: &mut Vec<Change>, c: Change, truncated: &mut bool| {
        if count >= MAX_CHANGES {
            *truncated = true;
        } else {
            count += 1;
            list.push(c);
        }
    };
    while let Some(item) = items.next() {
        if let Some(h) = item.strip_prefix("# ") {
            let (k, v) = h.split_once(' ').unwrap_or((h, ""));
            match k {
                "branch.oid" => st.initial = v == "(initial)",
                "branch.head" => {
                    st.detached = v == "(detached)";
                    st.branch = v.to_string();
                }
                "branch.upstream" => st.upstream = Some(v.to_string()),
                "branch.ab" => {
                    for part in v.split(' ') {
                        if let Some(n) = part.strip_prefix('+') {
                            st.ahead = n.parse().unwrap_or(0);
                        } else if let Some(n) = part.strip_prefix('-') {
                            st.behind = n.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        let kind = item.as_bytes()[0];
        match kind {
            b'1' | b'2' => {
                let n = if kind == b'1' { 9 } else { 10 };
                let f: Vec<&str> = item.splitn(n, ' ').collect();
                if f.len() < n {
                    continue;
                }
                let xy: Vec<char> = f[1].chars().collect();
                let path = f[n - 1].to_string();
                let orig = if kind == b'2' {
                    items.next().map(str::to_string)
                } else {
                    None
                };
                let (x, y) = (
                    xy.first().copied().unwrap_or('.'),
                    xy.get(1).copied().unwrap_or('.'),
                );
                if x != '.' {
                    push(
                        &mut st.staged,
                        Change {
                            path: path.clone(),
                            orig: orig.clone(),
                            code: x,
                        },
                        &mut st.truncated,
                    );
                }
                if y != '.' {
                    push(
                        &mut st.changes,
                        Change {
                            path,
                            orig,
                            code: y,
                        },
                        &mut st.truncated,
                    );
                }
            }
            b'u' => {
                let f: Vec<&str> = item.splitn(11, ' ').collect();
                if let Some(p) = f.get(10) {
                    push(
                        &mut st.conflicts,
                        Change {
                            path: p.to_string(),
                            orig: None,
                            code: 'U',
                        },
                        &mut st.truncated,
                    );
                }
            }
            b'?' => {
                if let Some(p) = item.get(2..) {
                    push(
                        &mut st.changes,
                        Change {
                            path: p.to_string(),
                            orig: None,
                            code: '?',
                        },
                        &mut st.truncated,
                    );
                }
            }
            _ => {}
        }
    }
    if st.detached {
        st.branch = "(detached)".into();
    }
    st
}

pub fn status(root: &str) -> Result<RepoStatus, String> {
    let r = checked_root(root)?;
    let raw = run(
        &r,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=all",
        ],
        LOCAL,
    )?;
    let mut st = parse_status(root, &raw);
    if st.detached
        && let Ok(h) = run(&r, &["rev-parse", "--short", "HEAD"], LOCAL)
    {
        st.branch = h.trim().to_string();
    }
    Ok(st)
}

// ---------------------------------------------------------------- değiştiren işlemler

pub fn stage(root: &str, paths: &[String]) -> Result<(), String> {
    let r = checked_root(root)?;
    checked_paths(paths)?;
    let mut args = vec!["add", "-A", "--"];
    args.extend(paths.iter().map(String::as_str));
    run(&r, &args, LOCAL).map(drop)
}

pub fn unstage(root: &str, paths: &[String]) -> Result<(), String> {
    let r = checked_root(root)?;
    checked_paths(paths)?;
    // Henüz commit yoksa HEAD yok: dizinden çıkarmak yeterli.
    let has_head = run(&r, &["rev-parse", "--verify", "-q", "HEAD"], LOCAL).is_ok();
    let mut args = if has_head {
        vec!["restore", "--staged", "--"]
    } else {
        vec!["rm", "--cached", "-r", "-q", "--"]
    };
    args.extend(paths.iter().map(String::as_str));
    run(&r, &args, LOCAL).map(drop)
}

/// Çalışma ağacındaki değişikliği geri alır (yalnızca izlenen dosyalar; izlenmeyen dosya silinmez).
pub fn discard(root: &str, paths: &[String]) -> Result<(), String> {
    let r = checked_root(root)?;
    checked_paths(paths)?;
    let mut args = vec!["restore", "--worktree", "--"];
    args.extend(paths.iter().map(String::as_str));
    run(&r, &args, LOCAL).map(drop)
}

pub fn commit(root: &str, message: &str) -> Result<(), String> {
    let r = checked_root(root)?;
    if message.trim().is_empty() {
        return Err("commit message is empty".into());
    }
    run(&r, &["commit", "-m", message], LOCAL).map(drop)
}

/// AI commit mesajı için bağlam: staged değişiklik varsa yalnızca o (VS Code gibi), yoksa bütün değişiklikler +
/// izlenmeyen dosyaların adları; son commit konuları (repo'nun üslubu). Diff `redact_code()`'tan geçer ve kısaltılır.
/// Döner: (repo kökü, metin).
pub fn changes_for_ai(root: &str) -> Result<(PathBuf, String), String> {
    const MAX_DIFF: usize = 60_000;
    let r = checked_root(root)?;
    let has_head = run(&r, &["rev-parse", "--verify", "-q", "HEAD"], LOCAL).is_ok();
    let staged = !run(&r, &["diff", "--cached", "--name-only"], LOCAL)?
        .trim()
        .is_empty();
    let base: Vec<&str> = if staged {
        vec!["diff", "--cached"]
    } else if has_head {
        vec!["diff", "HEAD"]
    } else {
        vec!["diff"]
    };
    let stat = run(&r, &[base.as_slice(), &["--stat"]].concat(), LOCAL)?;
    let mut diff = run(&r, &base, LOCAL)?;
    let untracked = if staged {
        String::new()
    } else {
        run(&r, &["ls-files", "--others", "--exclude-standard"], LOCAL)?
    };
    if stat.trim().is_empty() && untracked.trim().is_empty() {
        return Err("nothing to describe: no changes".into());
    }
    if diff.len() > MAX_DIFF {
        let mut cut = MAX_DIFF;
        while !diff.is_char_boundary(cut) {
            cut -= 1;
        }
        diff.truncate(cut);
        diff.push_str("\n… (diff truncated)\n");
    }
    let recent = if has_head {
        run(&r, &["log", "-10", "--format=%s"], LOCAL).unwrap_or_default()
    } else {
        String::new()
    };
    let mut out = String::new();
    if !recent.trim().is_empty() {
        out.push_str("Recent commit subjects in this repository (match their style):\n");
        out.push_str(recent.trim_end());
        out.push_str("\n\n");
    }
    out.push_str(if staged {
        "Staged changes:\n"
    } else {
        "Changes:\n"
    });
    out.push_str(stat.trim_end());
    out.push('\n');
    if !untracked.trim().is_empty() {
        out.push_str("\nNew untracked files:\n");
        for f in untracked.lines().take(50) {
            out.push_str(f);
            out.push('\n');
        }
    }
    out.push_str("\nDiff:\n");
    out.push_str(&redact_code(&diff));
    Ok((r, out))
}

pub fn push(root: &str) -> Result<(), String> {
    let r = checked_root(root)?;
    let st = status(root)?;
    if st.detached {
        return Err("HEAD is detached".into());
    }
    if st.upstream.is_some() {
        return run(&r, &["push"], NETWORK).map(drop);
    }
    // Upstream yok: tek uzak varsa (ya da origin) ona aynı adla yayınla.
    let remotes = run(&r, &["remote"], LOCAL)?;
    let remotes: Vec<&str> = remotes
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let remote = if remotes.contains(&"origin") {
        "origin"
    } else if remotes.len() == 1 {
        remotes[0]
    } else if remotes.is_empty() {
        return Err("no remote configured".into());
    } else {
        return Err("several remotes and no upstream: push once from a terminal".into());
    };
    run(&r, &["push", "-u", remote, "HEAD"], NETWORK).map(drop)
}

/// VS Code'daki Sync: önce pull (kullanıcının pull ayarıyla), sonra push.
pub fn sync(root: &str) -> Result<(), String> {
    let r = checked_root(root)?;
    let st = status(root)?;
    if st.upstream.is_none() {
        return push(root);
    }
    run(&r, &["pull"], NETWORK)?;
    run(&r, &["push"], NETWORK).map(drop)
}

// ---------------------------------------------------------------- diff, dosya, blame

/// Görüntülenen dosya sınırı
const MAX_FILE: usize = 1024 * 1024;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileText {
    pub text: String,
    /// Yok (yeni ya da silinmiş dosyanın karşı tarafı)
    pub missing: bool,
    pub binary: bool,
    pub too_large: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiffView {
    pub old: FileText,
    pub new: FileText,
    /// `git diff` çıktısı (unified, -U3); izlenmeyen dosyada boş: hepsi eklenmiş sayılır
    pub patch: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub hash: String,
    pub author: String,
    /// ms
    pub time: i64,
    pub summary: String,
    /// Henüz commit edilmemiş satır
    pub uncommitted: bool,
}

/// Bayt → metin: ikili ya da çok büyükse içerik gönderilmez. Bilinen anahtar biçimleri maskelenir.
pub fn to_text(bytes: Option<Vec<u8>>) -> FileText {
    let Some(b) = bytes else {
        return FileText {
            missing: true,
            ..Default::default()
        };
    };
    if b.len() > MAX_FILE {
        return FileText {
            too_large: true,
            ..Default::default()
        };
    }
    if b.iter().take(8000).any(|&c| c == 0) {
        return FileText {
            binary: true,
            ..Default::default()
        };
    }
    let text = String::from_utf8_lossy(&b);
    FileText {
        text: redact_code(&text).into_owned(),
        ..Default::default()
    }
}

/// `git show <rev>:<path>` (rev boşsa dizin); yoksa None.
fn show(root: &Path, rev: &str, path: &str) -> Option<Vec<u8>> {
    run(root, &["show", &format!("{rev}:{path}")], LOCAL)
        .ok()
        .map(String::into_bytes)
}

fn read_worktree(root: &Path, path: &str) -> Option<Vec<u8>> {
    let p = root.join(path);
    let meta = std::fs::metadata(&p).ok()?;
    if !meta.is_file() {
        return None;
    }
    if meta.len() as usize > MAX_FILE {
        return Some(vec![0u8; MAX_FILE + 1]);
    }
    std::fs::read(p).ok()
}

/// Staged: HEAD ↔ dizin. Değil: dizin ↔ çalışma ağacı (izlenmeyen dosyada eski taraf yok).
pub fn diff(root: &str, path: &str, orig: Option<&str>, staged: bool) -> Result<DiffView, String> {
    let r = checked_root(root)?;
    checked_paths(&[path.to_string()])?;
    if let Some(o) = orig {
        checked_paths(&[o.to_string()])?;
    }
    let old_path = orig.unwrap_or(path);
    let (old, new) = if staged {
        (show(&r, "HEAD", old_path), show(&r, "", path))
    } else {
        (show(&r, "", old_path), read_worktree(&r, path))
    };
    let mut args = vec!["diff", "--no-color", "--no-ext-diff", "-U3", "-M"];
    if staged {
        args.push("--cached");
    }
    args.push("--");
    args.push(path);
    if let Some(o) = orig {
        args.push(o);
    }
    let patch = run(&r, &args, LOCAL).unwrap_or_default();
    Ok(DiffView {
        old: to_text(old),
        new: to_text(new),
        patch: redact_code(&patch).into_owned(),
    })
}

/// Çalışma ağacındaki dosya (görüntüleyici).
pub fn file(root: &str, path: &str) -> Result<FileText, String> {
    let r = checked_root(root)?;
    checked_paths(&[path.to_string()])?;
    Ok(to_text(read_worktree(&r, path)))
}

/// `git blame --porcelain` çıktısının ilk kaydı.
pub fn parse_blame(raw: &str) -> Option<BlameLine> {
    let mut lines = raw.lines();
    let hash = lines.next()?.split(' ').next()?.to_string();
    let mut b = BlameLine {
        uncommitted: hash.chars().all(|c| c == '0'),
        hash,
        author: String::new(),
        time: 0,
        summary: String::new(),
    };
    for l in lines {
        if let Some(v) = l.strip_prefix("author ") {
            b.author = v.to_string();
        } else if let Some(v) = l.strip_prefix("author-time ") {
            b.time = v.trim().parse::<i64>().unwrap_or(0) * 1000;
        } else if let Some(v) = l.strip_prefix("summary ") {
            b.summary = redact(v).into_owned();
        } else if l.starts_with('\t') {
            break;
        }
    }
    Some(b)
}

/// Tek satırın blame'i (1'den başlar), çalışma ağacındaki hâline göre.
pub fn blame(root: &str, path: &str, line: u32) -> Result<Option<BlameLine>, String> {
    let r = checked_root(root)?;
    checked_paths(&[path.to_string()])?;
    let range = format!("{line},{line}");
    match run(
        &r,
        &["blame", "--porcelain", "-L", &range, "--", path],
        LOCAL,
    ) {
        Ok(raw) => Ok(parse_blame(&raw)),
        // İzlenmeyen dosya ya da satır yok
        Err(_) => Ok(None),
    }
}

// ---------------------------------------------------------------- geçmiş

const FS: char = '\u{1f}';
const RS: char = '\u{1e}';

pub fn parse_log(raw: &str) -> Vec<Commit> {
    raw.split(RS)
        .filter_map(|rec| {
            let rec = rec.trim_start_matches('\n');
            let f: Vec<&str> = rec.splitn(6, FS).collect();
            if f.len() < 6 || f[0].is_empty() {
                return None;
            }
            Some(Commit {
                hash: f[0].to_string(),
                parents: f[1]
                    .split(' ')
                    .filter(|p| !p.is_empty())
                    .map(str::to_string)
                    .collect(),
                author: f[2].to_string(),
                time: f[3].trim().parse::<i64>().unwrap_or(0) * 1000,
                refs: f[4]
                    .split(", ")
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
                subject: redact(f[5].trim_end()).into_owned(),
            })
        })
        .collect()
}

/// HEAD ve (varsa) upstream'in geçmişi; dal grafiği arayüzde çizilir.
pub fn log(root: &str, limit: usize) -> Result<Vec<Commit>, String> {
    let r = checked_root(root)?;
    if run(&r, &["rev-parse", "--verify", "-q", "HEAD"], LOCAL).is_err() {
        return Ok(vec![]);
    }
    let n = format!("-n{}", limit.clamp(1, MAX_LOG));
    let format = "--format=%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s%x1e";
    let mut args = vec!["log", "--topo-order", n.as_str(), format, "HEAD"];
    let has_upstream = run(&r, &["rev-parse", "--verify", "-q", "@{upstream}"], LOCAL).is_ok();
    if has_upstream {
        args.push("@{upstream}");
    }
    args.push("--");
    Ok(parse_log(&run(&r, &args, LOCAL)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_v2() {
        let raw = [
            "# branch.oid 1234567890abcdef",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -1",
            "1 M. N... 100644 100644 100644 aaa bbb src/a b.rs",
            "1 .M N... 100644 100644 100644 aaa bbb lib.rs",
            "1 MM N... 100644 100644 100644 aaa bbb both.rs",
            "2 R. N... 100644 100644 100644 aaa bbb R100 new name.rs",
            "old name.rs",
            "u UU N... 100644 100644 100644 100644 a b c conflict.rs",
            "? notes/todo.md",
            "! ignored.log",
            "",
        ]
        .join("\0");
        let st = parse_status("/r", &raw);
        assert_eq!(st.branch, "main");
        assert_eq!(st.upstream.as_deref(), Some("origin/main"));
        assert_eq!((st.ahead, st.behind), (2, 1));
        assert!(!st.initial && !st.detached);
        let staged: Vec<_> = st
            .staged
            .iter()
            .map(|c| (c.path.as_str(), c.code))
            .collect();
        assert_eq!(
            staged,
            [("src/a b.rs", 'M'), ("both.rs", 'M'), ("new name.rs", 'R')]
        );
        assert_eq!(st.staged[2].orig.as_deref(), Some("old name.rs"));
        let changes: Vec<_> = st
            .changes
            .iter()
            .map(|c| (c.path.as_str(), c.code))
            .collect();
        assert_eq!(
            changes,
            [("lib.rs", 'M'), ("both.rs", 'M'), ("notes/todo.md", '?')]
        );
        assert_eq!(st.conflicts[0].path, "conflict.rs");
    }

    #[test]
    fn parses_initial_and_detached() {
        let st = parse_status("/r", "# branch.oid (initial)\0# branch.head main\0? a\0");
        assert!(st.initial);
        assert_eq!(st.upstream, None);
        let st = parse_status("/r", "# branch.oid abc\0# branch.head (detached)\0");
        assert!(st.detached);
    }

    #[test]
    fn parses_log_records() {
        let raw = "aaa\u{1f}bbb ccc\u{1f}Kaan\u{1f}1700000000\u{1f}HEAD -> main, origin/main\u{1f}merge: x\u{1e}\n\
                   bbb\u{1f}\u{1f}Kaan\u{1f}1690000000\u{1f}\u{1f}init\u{1e}\n";
        let log = parse_log(raw);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].parents, ["bbb", "ccc"]);
        assert_eq!(log[0].refs, ["HEAD -> main", "origin/main"]);
        assert_eq!(log[0].time, 1_700_000_000_000);
        assert!(log[1].parents.is_empty() && log[1].refs.is_empty());
    }

    #[test]
    fn rejects_paths_outside_the_repo() {
        assert!(checked_paths(&["a/b.rs".into()]).is_ok());
        assert!(checked_paths(&["../x".into()]).is_err());
        assert!(checked_paths(&["/etc/passwd".into()]).is_err());
        assert!(checked_paths(&[]).is_err());
        assert!(checked_root("relative").is_err());
    }

    #[test]
    fn explains_common_errors() {
        let e = tidy_error(
            "remote: Permission to octo-org/demo-terminal.git denied to kaankarakusx.\nfatal: unable to access 'https://github.com/x/'",
        );
        assert!(
            e.starts_with(
                "kaankarakusx can't push to octo-org/demo-terminal: no write access"
            ),
            "{e}"
        );
        assert!(
            tidy_error("! [rejected] main -> main (fetch first)\nerror: failed to push some refs")
                .contains("Use sync")
        );
    }

    #[test]
    fn errors_hide_credentials() {
        let e = tidy_error(
            "fatal: unable to access 'https://kaan:ghp_abcdefghijklmnopqrstuvwxyz0123456789@github.com/x.git/'\nhint: try",
        );
        assert!(!e.contains("ghp_") && !e.contains("kaan:"), "{e}");
        assert!(e.contains("https://•••@github.com"));
    }

    /// Gerçek git ile uçtan uca (geçici klasörde): stage → commit → değiştir → unstage → discard → log.
    #[test]
    fn real_git_round_trip() {
        if git_bin().is_none() {
            eprintln!("git yok, atlandı");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(root.join("src")).unwrap();
        run(&root, &["init", "-q", "-b", "main"], LOCAL).unwrap();
        run(&root, &["config", "user.email", "t@example.com"], LOCAL).unwrap();
        run(&root, &["config", "user.name", "Test"], LOCAL).unwrap();
        run(&root, &["config", "commit.gpgsign", "false"], LOCAL).unwrap();
        let r = root.to_string_lossy().into_owned();

        std::fs::write(root.join("src/a.txt"), "one\n").unwrap();
        let st = status(&r).unwrap();
        assert!(st.initial);
        assert_eq!(
            st.changes[0],
            Change {
                path: "src/a.txt".into(),
                orig: None,
                code: '?'
            }
        );

        stage(&r, &["src/a.txt".into()]).unwrap();
        unstage(&r, &["src/a.txt".into()]).unwrap(); // HEAD yokken de çalışmalı
        assert!(status(&r).unwrap().staged.is_empty());
        stage(&r, &["src/a.txt".into()]).unwrap();
        assert_eq!(status(&r).unwrap().staged[0].code, 'A');
        assert!(commit(&r, "  ").is_err());
        commit(&r, "first").unwrap();

        std::fs::write(root.join("src/a.txt"), "two\n").unwrap();
        let st = status(&r).unwrap();
        assert_eq!(st.changes[0].code, 'M');
        assert!(!st.initial);
        discard(&r, &["src/a.txt".into()]).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("src/a.txt")).unwrap(),
            "one\n"
        );
        assert!(status(&r).unwrap().changes.is_empty());

        let log = log(&r, 10).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].subject, "first");
        assert!(log[0].refs.iter().any(|x| x.contains("main")));
        // Uzak yok: push anlaşılır bir hatayla döner
        assert_eq!(push(&r).unwrap_err(), "no remote configured");

        // diff: çalışma ağacı ↔ dizin, staged: dizin ↔ HEAD, izlenmeyen dosya, blame
        std::fs::write(root.join("src/a.txt"), "one\ntwo\n").unwrap();
        let d = diff(&r, "src/a.txt", None, false).unwrap();
        assert_eq!(
            (d.old.text.as_str(), d.new.text.as_str()),
            ("one\n", "one\ntwo\n")
        );
        assert!(
            d.patch.contains("@@ -1 +1,2 @@") && d.patch.contains("+two"),
            "{}",
            d.patch
        );
        stage(&r, &["src/a.txt".into()]).unwrap();
        let d = diff(&r, "src/a.txt", None, true).unwrap();
        assert_eq!(d.new.text, "one\ntwo\n");
        assert!(diff(&r, "src/a.txt", None, false).unwrap().patch.is_empty());
        std::fs::write(
            root.join("new.txt"),
            "hello sk-ant-api03-abcdefghijklmnopqrstuvwxyz\n",
        )
        .unwrap();
        let d = diff(&r, "new.txt", None, false).unwrap();
        assert!(d.old.missing && d.patch.is_empty());
        assert!(
            !d.new.text.contains("sk-ant-api03"),
            "bilinen anahtar maskelenmeli"
        );
        std::fs::write(root.join("bin.dat"), [0u8, 1, 2]).unwrap();
        assert!(file(&r, "bin.dat").unwrap().binary);
        let b = blame(&r, "src/a.txt", 1).unwrap().unwrap();
        assert_eq!(
            (b.author.as_str(), b.summary.as_str(), b.uncommitted),
            ("Test", "first", false)
        );
        assert!(blame(&r, "src/a.txt", 2).unwrap().unwrap().uncommitted);
        assert!(diff(&r, "../etc/passwd", None, false).is_err());

        let repos = repos_of(&[root.join("src").to_string_lossy().into_owned(), r.clone()]);
        assert_eq!(
            repos,
            [RepoRef {
                root: r.clone(),
                name: "repo".into()
            }]
        );
    }
}
