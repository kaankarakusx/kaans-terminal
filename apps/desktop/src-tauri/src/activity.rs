//! Aktif çalışma dakikası (OZELLIKLER.md §6.3). Bir dakika iki koşul birlikte sağlanırsa aktiftir:
//! 1. kullanıcı AFK değil: son 5 dakikada sistem genelinde klavye ya da fare kullanılmış,
//! 2. çalışma sinyali var: son 5 dakikada bir sohbet çalışmış ya da bir terminalde komut koşmuş.
//!
//! Yalnızca günlük aktif saniye saklanır (`<app_data>/activity.json`); XP, level ve rütbe bundan
//! `packages/shared/src/xp.ts` ile hesaplanır. Kredi sunucunun heartbeat kuralıyla aynıdır: atış başına en
//! fazla 90 sn, boşluk 3 dakikayı aşarsa 60 sn.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// AFK eşiği ve çalışma sinyali penceresi
pub const WINDOW_SECS: f64 = 300.0;
/// Saklanan gün sayısı
const KEEP_DAYS: usize = 400;
const FILE: &str = "activity.json";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DayActivity {
    pub day: String,
    pub active_secs: u64,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct Stored {
    days: BTreeMap<String, u64>,
}

/// Sistem genelinde son klavye/fare hareketinden bu yana geçen saniye. Okunamazsa None.
pub fn system_idle_secs() -> Option<f64> {
    imp::idle()
}

#[cfg(target_os = "macos")]
mod imp {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }
    /// kCGEventSourceStateHIDSystemState
    const HID_SYSTEM_STATE: i32 = 1;
    /// kCGAnyInputEventType
    const ANY_INPUT: u32 = !0;

    pub fn idle() -> Option<f64> {
        // SAFETY: yan etkisiz sorgu; izin gerektirmez (olay içeriği okunmaz, yalnızca süre).
        let s = unsafe { CGEventSourceSecondsSinceLastEventType(HID_SYSTEM_STATE, ANY_INPUT) };
        s.is_finite().then_some(s.max(0.0))
    }
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    pub fn idle() -> Option<f64> {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // SAFETY: cbSize doğru verilmiş yerel bir yapı.
        if unsafe { GetLastInputInfo(&mut info) } == 0 {
            return None;
        }
        // SAFETY: argümansız sorgu.
        let now = unsafe { GetTickCount() };
        Some(f64::from(now.wrapping_sub(info.dwTime)) / 1000.0)
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod imp {
    pub fn idle() -> Option<f64> {
        None
    }
}

/// Sunucunun heartbeat kuralı (packages/shared `heartbeatCredit` ile aynı)
pub fn credit(gap_secs: u64) -> u64 {
    if gap_secs > 180 { 60 } else { gap_secs.min(90) }
}

pub struct Tracker {
    path: PathBuf,
    data: Stored,
    /// Son örnek (ms)
    last_sample: Option<i64>,
}

impl Tracker {
    pub fn load(app_data: &Path) -> Self {
        let path = app_data.join(FILE);
        let data = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            path,
            data,
            last_sample: None,
        }
    }

    /// Bir örnek (dakikada bir). `active` ise önceki örnekten bu yana geçen süre kadar (kurala göre) kredi verir.
    /// Değiştiyse true.
    pub fn sample(&mut self, now_ms: i64, day: &str, active: bool) -> bool {
        let gap = self
            .last_sample
            .map(|t| ((now_ms - t).max(0) / 1000) as u64)
            .unwrap_or(60);
        self.last_sample = Some(now_ms);
        if !active {
            return false;
        }
        *self.data.days.entry(day.to_string()).or_insert(0) += credit(gap);
        while self.data.days.len() > KEEP_DAYS {
            let first = self.data.days.keys().next().cloned();
            if let Some(k) = first {
                self.data.days.remove(&k);
            }
        }
        self.save();
        true
    }

    fn save(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("json.tmp");
        if let Ok(b) = serde_json::to_vec(&self.data)
            && std::fs::write(&tmp, b).is_ok()
        {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }

    pub fn days(&self) -> Vec<DayActivity> {
        self.data
            .days
            .iter()
            .map(|(day, secs)| DayActivity {
                day: day.clone(),
                active_secs: *secs,
            })
            .collect()
    }
}

/// Çalışma sinyali: son 5 dakikada bir sohbet etkinliği ya da çalışan bir terminal komutu.
pub fn working(snapshot: &crate::engine::Snapshot, now_ms: i64) -> bool {
    let window = (WINDOW_SECS * 1000.0) as i64;
    snapshot.sessions.iter().any(|s| {
        s.status == crate::engine::status::Status::Busy || now_ms - s.last_activity <= window
    }) || snapshot.terminals.iter().any(|t| t.busy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn reads_system_idle_time() {
        let s = system_idle_secs().expect("boşta kalma süresi okunamadı");
        assert!(s >= 0.0);
    }

    #[test]
    fn credits_like_the_server_and_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let mut t = Tracker::load(tmp.path());
        assert!(t.sample(1_000_000, "2026-11-08", true)); // ilk örnek: 60 sn
        assert!(!t.sample(1_060_000, "2026-11-08", false)); // AFK: kredi yok
        assert!(t.sample(1_180_000, "2026-11-08", true)); // 120 sn boşluk → 90
        assert!(t.sample(1_600_000, "2026-11-08", true)); // 420 sn boşluk (uyku) → 60
        assert!(t.sample(1_660_000, "2026-11-09", true)); // yeni gün
        assert_eq!(
            t.days(),
            [
                DayActivity {
                    day: "2026-11-08".into(),
                    active_secs: 210
                },
                DayActivity {
                    day: "2026-11-09".into(),
                    active_secs: 60
                },
            ]
        );
        // Diskten geri gelir
        assert_eq!(Tracker::load(tmp.path()).days().len(), 2);
        assert_eq!((credit(30), credit(120), credit(181)), (30, 90, 60));
    }
}
