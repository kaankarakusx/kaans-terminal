//! Süreç bilgisi. Harici komut (`ps`) çalıştırılmaz; doğrudan kernel'e sorulur (MIMARI.md §3.1).

use chrono::NaiveDateTime;

/// Kayıttaki başlangıç zamanı ile kernel'in söylediği arasında kabul edilen fark.
const START_TOLERANCE_SECS: i64 = 2;

/// Kayıt dosyasındaki süreç yaşıyor mu? Pid yeniden kullanıldıysa (başka bir süreç aynı pid'i aldıysa)
/// başlangıç zamanları tutmaz ve süreç ölü sayılır. Başlangıç zamanı bilinmiyorsa yalnızca varlığa bakılır.
pub fn is_alive_since(pid: u32, expected_start_secs: Option<i64>) -> bool {
    if !is_alive(pid) {
        return false;
    }
    match (expected_start_secs, start_time_secs(pid)) {
        (Some(expected), Some(actual)) => (expected - actual).abs() <= START_TOLERANCE_SECS,
        _ => true,
    }
}

/// Claude Code'un `procStart` alanı: `"Sun Nov  8 10:24:45 2026"` (UTC) → unix saniyesi.
pub fn parse_proc_start(s: &str) -> Option<i64> {
    let normalized = s.split_whitespace().collect::<Vec<_>>().join(" ");
    NaiveDateTime::parse_from_str(&normalized, "%a %b %d %H:%M:%S %Y")
        .ok()
        .map(|t| t.and_utc().timestamp())
}

/// `kill(pid, 0)` sinyal göndermez, yalnızca varlığı sınar.
#[cfg(unix)]
pub fn is_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // SAFETY: kill(2) sinyal 0 ile yalnızca izin/varlık kontrolü yapar, belleğe dokunmaz.
    let r = unsafe { libc::kill(pid, 0) };
    r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Sürecin başlangıç zamanı (unix saniyesi).
#[cfg(target_os = "macos")]
pub fn start_time_secs(pid: u32) -> Option<i64> {
    let pid = libc::c_int::try_from(pid).ok()?;
    let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: proc_bsdinfo düz bir C yapısı; sıfırla doldurmak geçerli bir değer. proc_pidinfo en fazla
    // `size` bayt yazar ve yazdığı bayt sayısını döndürür.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&raw mut info).cast::<libc::c_void>(),
            size,
        )
    };
    (written == size).then(|| i64::try_from(info.pbi_start_tvsec).unwrap_or(0))
}

/// Linux'ta henüz okunmuyor (hedef platformlar macOS ve Windows); pid varlığı yeterli sayılır.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn start_time_secs(_pid: u32) -> Option<i64> {
    None
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: tutamaç OpenProcess'ten geldi ve yalnızca burada kapanır.
            unsafe { CloseHandle(self.0) };
        }
    }

    fn open(pid: u32) -> Option<Handle> {
        // SAFETY: OpenProcess başarısızsa null döner; null tutamaç saklanmaz.
        let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        (!h.is_null()).then_some(Handle(h))
    }

    pub fn is_alive(pid: u32) -> bool {
        let Some(h) = open(pid) else { return false };
        let mut code = 0u32;
        // SAFETY: geçerli tutamaç ve yazılabilir u32.
        let ok = unsafe { GetExitCodeProcess(h.0, &mut code) };
        ok != 0 && code == STILL_ACTIVE as u32
    }

    pub fn start_time_secs(pid: u32) -> Option<i64> {
        let h = open(pid)?;
        let zero = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        // SAFETY: geçerli tutamaç ve dört yazılabilir FILETIME.
        let ok = unsafe { GetProcessTimes(h.0, &mut created, &mut exited, &mut kernel, &mut user) };
        if ok == 0 {
            return None;
        }
        // FILETIME: 1601'den beri 100 ns aralıklar.
        let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        const EPOCH_DIFF: u64 = 116_444_736_000_000_000;
        i64::try_from(ticks.checked_sub(EPOCH_DIFF)? / 10_000_000).ok()
    }
}

#[cfg(windows)]
pub use win::{is_alive, start_time_secs};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_start() {
        // 2026-11-08 10:24:45 UTC
        assert_eq!(
            parse_proc_start("Sun Nov  8 10:24:45 2026"),
            Some(1_794_133_485)
        );
        assert_eq!(
            parse_proc_start("Sat Nov 7 10:42:43 2026"),
            Some(1_794_048_163)
        );
        assert_eq!(parse_proc_start("dün"), None);
    }

    #[test]
    fn own_process_is_alive() {
        let me = std::process::id();
        assert!(is_alive(me));
        assert!(!is_alive(u32::MAX));
        assert!(is_alive_since(me, None));
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn detects_pid_reuse_by_start_time() {
        let me = std::process::id();
        let start = start_time_secs(me).expect("başlangıç zamanı okunmalı");
        let now = chrono::Utc::now().timestamp();
        assert!(
            start <= now && now - start < 3600,
            "makul olmayan başlangıç: {start}"
        );
        assert!(is_alive_since(me, Some(start)));
        assert!(
            is_alive_since(me, Some(start + 1)),
            "küçük fark tolere edilir"
        );
        assert!(
            !is_alive_since(me, Some(start - 3600)),
            "pid başka sürece geçmiş sayılır"
        );
    }
}
