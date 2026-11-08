//! Güç durumu: pilde mi, düşük güç modu açık mı (OZELLIKLER.md §2.3 enerji tasarrufu).
//! Sayfa bunu `power://state` olayıyla alır ve orbit.map'in kare hızını/çözünürlüğünü sınırlar.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerState {
    pub on_battery: bool,
    /// macOS Low Power Mode / Windows pil tasarrufu
    pub low_power: bool,
}

#[cfg(target_os = "macos")]
mod imp {
    use super::PowerState;

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        /// Prize takılıyken `kIOPSTimeRemainingUnlimited` (-2.0) döner.
        fn IOPSGetTimeRemainingEstimate() -> f64;
    }
    const UNLIMITED: f64 = -2.0;

    pub fn read() -> PowerState {
        // SAFETY: argümansız, yan etkisiz IOKit sorgusu.
        let remaining = unsafe { IOPSGetTimeRemainingEstimate() };
        let low_power = objc2_foundation::NSProcessInfo::processInfo().isLowPowerModeEnabled();
        PowerState {
            on_battery: remaining != UNLIMITED,
            low_power,
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::PowerState;
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    pub fn read() -> PowerState {
        // SAFETY: SYSTEM_POWER_STATUS düz bir C yapısı; GetSystemPowerStatus onu doldurur.
        let mut s: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
        if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
            return PowerState::default();
        }
        PowerState {
            // 0 = pilde, 1 = prizde, 255 = bilinmiyor
            on_battery: s.ACLineStatus == 0,
            // 1 = pil tasarrufu açık
            low_power: s.SystemStatusFlag == 1,
        }
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod imp {
    pub fn read() -> super::PowerState {
        super::PowerState::default()
    }
}

pub use imp::read;

#[cfg(test)]
mod tests {
    #[test]
    fn reads_without_panicking() {
        // Değer makineye bağlı; yalnızca çağrının güvenle döndüğünü sınar.
        let _ = super::read();
    }
}
