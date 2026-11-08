//! Oturum durumu hesabı (`statusOf`, MIMARI.md §3.2).

use serde::Serialize;

/// Sıralama sırası da budur: waiting → idle → busy → ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// "needs you": onay ya da soru bekliyor
    Waiting,
    /// "your turn": cevap verdi, sıra kullanıcıda
    Idle,
    /// "working"
    Busy,
    Ended,
}

/// Kayıt yokken son transcript yazımı bu kadar yeniyse oturum hâlâ çalışıyor sayılır.
pub const RECENT_ACTIVITY_MS: i64 = 90_000;
/// Ana oturum çalışırken alt ajanın sessiz kalabileceği süre.
pub const SUBAGENT_QUIET_BUSY_MS: i64 = 15 * 60_000;
/// Ana oturum çalışmıyorken alt ajanın sessiz kalabileceği süre.
pub const SUBAGENT_QUIET_IDLE_MS: i64 = 3 * 60_000;

#[derive(Debug, Clone, Copy, Default)]
pub struct StatusInput<'a> {
    /// Canlı kayıt dosyası var **ve** süreci yaşıyorsa: kayıttaki `status` (boş olabilir).
    /// Kayıt yoksa ya da süreç ölmüşse `None`.
    pub live_record: Option<&'a str>,
    /// Son transcript kaydından bu yana geçen süre.
    pub last_activity_age_ms: Option<i64>,
    pub subagent_running: bool,
    /// Bu oturumun canlı kaydı daha önce görüldü ama artık yok (süreç kapandı, End Session): hemen biter.
    /// Kaydı hiç görülmemiş oturumlar (eski sürüm, SDK) için son etkinlik kuralı geçerlidir.
    pub record_gone: bool,
}

/// Alt ajan çalışıyor mu sayılır?
/// Çalışıyor = bitmedi VE (bekleyen araç çağrısı var VEYA son kaydı yeterince yeni).
pub fn subagent_running(
    finished: bool,
    pending_tools: usize,
    age_ms: i64,
    main_busy: bool,
) -> bool {
    if finished {
        return false;
    }
    let quiet = if main_busy {
        SUBAGENT_QUIET_BUSY_MS
    } else {
        SUBAGENT_QUIET_IDLE_MS
    };
    pending_tools > 0 || age_ms < quiet
}

/// Alt ajan kuralı uygulanmadan önceki durum.
pub fn base_status(input: &StatusInput) -> Status {
    match input.live_record {
        Some("idle" | "shell") => Status::Idle,
        Some("waiting") => Status::Waiting,
        Some(_) => Status::Busy,
        None if input.record_gone => Status::Ended,
        None => match input.last_activity_age_ms {
            Some(age) if age < RECENT_ACTIVITY_MS => Status::Busy,
            _ => Status::Ended,
        },
    }
}

pub fn status_of(input: &StatusInput) -> Status {
    match base_status(input) {
        Status::Idle if input.subagent_running => Status::Busy,
        s => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vanished_record_ends_the_session_at_once() {
        // End Session: kayıt silindi, transcript birkaç saniye önce yazılmıştı → çalışıyor değil, bitti
        let gone = StatusInput {
            live_record: None,
            last_activity_age_ms: Some(3_000),
            subagent_running: false,
            record_gone: true,
        };
        assert_eq!(status_of(&gone), Status::Ended);
        // Kaydı hiç olmamış oturum: yakın zamanda yazıyorsa hâlâ çalışıyor sayılır
        let never = StatusInput {
            record_gone: false,
            ..gone
        };
        assert_eq!(status_of(&never), Status::Busy);
    }

    fn st(live: Option<&str>, age: Option<i64>, sub: bool) -> Status {
        status_of(&StatusInput {
            live_record: live,
            last_activity_age_ms: age,
            subagent_running: sub,
            record_gone: false,
        })
    }

    #[test]
    fn live_record_statuses() {
        assert_eq!(st(Some("idle"), None, false), Status::Idle);
        assert_eq!(st(Some("shell"), None, false), Status::Idle);
        assert_eq!(st(Some("waiting"), None, false), Status::Waiting);
        assert_eq!(st(Some("busy"), None, false), Status::Busy);
        assert_eq!(st(Some(""), None, false), Status::Busy);
        assert_eq!(st(Some("something-new"), None, false), Status::Busy);
    }

    #[test]
    fn no_record_uses_recent_activity() {
        assert_eq!(st(None, Some(10_000), false), Status::Busy);
        assert_eq!(st(None, Some(89_999), false), Status::Busy);
        assert_eq!(st(None, Some(90_000), false), Status::Ended);
        assert_eq!(st(None, None, false), Status::Ended);
    }

    #[test]
    fn running_subagent_makes_idle_busy_but_not_waiting() {
        assert_eq!(st(Some("idle"), None, true), Status::Busy);
        assert_eq!(st(Some("waiting"), None, true), Status::Waiting);
        assert_eq!(st(None, None, true), Status::Ended);
    }

    #[test]
    fn subagent_running_rules() {
        assert!(!subagent_running(true, 3, 0, true));
        assert!(subagent_running(false, 1, 60 * 60_000, false));
        assert!(subagent_running(false, 0, 10 * 60_000, true));
        assert!(!subagent_running(false, 0, 10 * 60_000, false));
        assert!(subagent_running(false, 0, 2 * 60_000, false));
    }

    #[test]
    fn sort_order() {
        let mut v = vec![Status::Ended, Status::Busy, Status::Waiting, Status::Idle];
        v.sort();
        assert_eq!(
            v,
            [Status::Waiting, Status::Idle, Status::Busy, Status::Ended]
        );
    }
}
