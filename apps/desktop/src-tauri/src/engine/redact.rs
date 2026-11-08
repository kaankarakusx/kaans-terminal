//! Gizli bilgi maskeleme (OZELLIKLER.md §1.4).
//! Arayüze giden **her** metin buradan geçer; ekran kaydına anahtar sızmasın.

use regex::{Captures, Regex};
use std::borrow::Cow;
use std::sync::LazyLock;

pub const MASK: &str = "‹•••›";

/// Bilinen anahtar biçimleri: eşleşmenin tamamı maskelenir.
static TOKENS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\bsk-ant-[A-Za-z0-9_\-]{16,}",                 // Anthropic
        r"|\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_\-]{20,}", // OpenAI
        r"|\bgh[pousr]_[A-Za-z0-9]{30,}",                // GitHub
        r"|\bgithub_pat_[A-Za-z0-9_]{22,}",
        r"|\blin_(?:api|oauth)_[A-Za-z0-9]{32,}", // Linear
        r"|\bxox[abposr]-[A-Za-z0-9\-]{10,}",     // Slack
        r"|\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",        // AWS erişim anahtarı
        r"|\beyJ[A-Za-z0-9_\-]{6,}\.eyJ[A-Za-z0-9_\-]{6,}\.[A-Za-z0-9_\-]{6,}", // JWT
        r"|\b[0-9a-fA-F]{33,}\b",                 // 32 karakterden uzun hex
    ))
    .expect("redact token regex")
});

/// `Bearer xxx`: önek korunur, değer maskelenir.
static BEARER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(bearer)\s+[A-Za-z0-9._~+/=\-]{8,}").expect("bearer regex")
});

/// `password=…`, `token: …`, `"api_key": "…"`: anahtar korunur, değer maskelenir.
static KEY_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r#"(?i)\b((?:[A-Za-z0-9]+[_\-])*(?:password|passwd|pwd|secret|token|api[_\-]?key|access[_\-]?key|auth)"#,
        r#"["']?\s*[:=]\s*["']?)([^\s"'&,;]{3,})"#,
    ))
    .expect("key-value regex")
});

pub fn redact(text: &str) -> Cow<'_, str> {
    if text.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut out: Cow<str> = Cow::Borrowed(text);
    if let Cow::Owned(s) = TOKENS.replace_all(&out, MASK) {
        out = Cow::Owned(s);
    }
    if let Cow::Owned(s) = BEARER.replace_all(&out, |c: &Captures| format!("{} {MASK}", &c[1])) {
        out = Cow::Owned(s);
    }
    if let Cow::Owned(s) = KEY_VALUE.replace_all(&out, |c: &Captures| {
        if &c[2] == MASK {
            c[0].to_string()
        } else {
            format!("{}{MASK}", &c[1])
        }
    }) {
        out = Cow::Owned(s);
    }
    out
}

/// Kod görüntüleyici için: yalnızca bilinen anahtar biçimleri ve `Bearer …` maskelenir. `token = getToken()`
/// gibi sıradan kodu bozmasın diye anahtar=değer sezgisi uygulanmaz.
pub fn redact_code(text: &str) -> Cow<'_, str> {
    let mut out: Cow<str> = Cow::Borrowed(text);
    if let Cow::Owned(s) = TOKENS.replace_all(&out, MASK) {
        out = Cow::Owned(s);
    }
    if let Cow::Owned(s) = BEARER.replace_all(&out, |c: &Captures| format!("{} {MASK}", &c[1])) {
        out = Cow::Owned(s);
    }
    out
}

/// Maskeler, tek satıra indirir ve `max` karakterde `…` ile keser.
pub fn clean(text: &str, max: usize) -> String {
    let red = redact(text);
    let one_line = red.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max {
        one_line
    } else {
        let mut s: String = one_line.chars().take(max.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_known_key_formats() {
        let cases = [
            "sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWxYz012345",
            "sk-proj-AbCdEfGhIjKlMnOpQrStUvWxYz012345",
            "ghp_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789",
            "github_pat_11ABCDEFG0123456789_abcdefghijklmnop",
            "lin_api_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789",
            "xoxb-1234567890-abcdefghij",
            "AKIAIOSFODNN7EXAMPLE",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U",
            "0123456789abcdef0123456789abcdef0", // 33 hex
        ];
        for key in cases {
            let input = format!("x {key} y");
            assert_eq!(redact(&input), format!("x {MASK} y"), "sızdı: {key}");
        }
    }

    #[test]
    fn masks_key_values_but_keeps_key_names() {
        assert_eq!(redact("password=hunter22"), format!("password={MASK}"));
        assert_eq!(redact("token: abc123def"), format!("token: {MASK}"));
        assert_eq!(
            redact(r#""api_key": "s3cr3tvalue""#),
            format!(r#""api_key": "{MASK}""#)
        );
        assert_eq!(
            redact("GITHUB_TOKEN=abcdef123"),
            format!("GITHUB_TOKEN={MASK}")
        );
        assert_eq!(
            redact("curl -H 'Authorization: Bearer abc.def.ghijkl'"),
            format!("curl -H 'Authorization: Bearer {MASK}'")
        );
    }

    #[test]
    fn leaves_normal_text_alone() {
        let plain = [
            "cargo build --release",
            "/Users/kaan/Desktop/kaans-terminal/src/main.rs",
            "session c0db5897-86a1-478d-b86c-4b5a2041bb3f",
            "git checkout 3f2a9c1",
            "the token count is high",
            "0123456789abcdef0123456789abcdef", // tam 32 hex: maskelenmez
            "npx disk-usage-analyzer-for-macos-system",
        ];
        for t in plain {
            assert!(matches!(redact(t), Cow::Borrowed(_)), "gereksiz maske: {t}");
        }
    }

    #[test]
    fn clean_truncates_on_chars() {
        assert_eq!(clean("a\n  b\tc", 10), "a b c");
        assert_eq!(clean("ğğğğğğ", 4), "ğğğ…");
    }
}
