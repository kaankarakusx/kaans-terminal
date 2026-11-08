//! Süreç tablosu: bütün süreçler, ebeveyn, ad, yol, cwd, CPU zamanı, terminal (MIMARI.md §3.1, §3.7).
//! Doğrudan kernel'den okunur (Mac `libproc`, Windows ToolHelp); `ps` gibi komut çalıştırılmaz.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub ppid: u32,
    pub pgid: u32,
    pub uid: u32,
    /// Kontrol terminali var mı
    pub has_tty: bool,
    pub name: String,
    pub start_secs: i64,
    /// Toplam CPU zamanı (kullanıcı + sistem), nanosaniye
    pub cpu_ns: u64,
}

/// pid → ebeveyn eşlemesinden bir sürecin bütün torunları (kendisi hariç).
pub fn descendants(root: u32, table: &[ProcInfo]) -> Vec<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for p in table {
        if p.pid != p.ppid {
            children.entry(p.ppid).or_default().push(p.pid);
        }
    }
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(pid) = stack.pop() {
        if let Some(kids) = children.get(&pid) {
            for &k in kids {
                if !out.contains(&k) && k != root {
                    out.push(k);
                    stack.push(k);
                }
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
mod imp {
    use super::ProcInfo;
    use std::ffi::CStr;
    use std::sync::OnceLock;

    /// Apple Silicon'da görev CPU zamanı mach birimindedir; nanosaniyeye çevirme oranı.
    /// (libc bu yapıyı `mach2` crate'ine yönlendirip eskimiş saydı; tek çağrı için ek bağımlılık eklenmedi.)
    #[allow(deprecated)]
    fn timebase() -> (u64, u64) {
        static TB: OnceLock<(u64, u64)> = OnceLock::new();
        *TB.get_or_init(|| {
            let mut info = libc::mach_timebase_info { numer: 0, denom: 0 };
            // SAFETY: yazılabilir yapı; çağrı yalnızca onu doldurur.
            let ok = unsafe { libc::mach_timebase_info(&mut info) } == 0;
            if ok && info.denom != 0 {
                (u64::from(info.numer), u64::from(info.denom))
            } else {
                (1, 1)
            }
        })
    }

    pub fn all_pids() -> Vec<u32> {
        // SAFETY: önce gereken boyut sorulur, sonra o kadar yer ayrılıp doldurulur.
        let n = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        if n <= 0 {
            return Vec::new();
        }
        let mut buf = vec![0 as libc::pid_t; n as usize + 64];
        let bytes =
            i32::try_from(buf.len() * std::mem::size_of::<libc::pid_t>()).unwrap_or(i32::MAX);
        let got = unsafe { libc::proc_listallpids(buf.as_mut_ptr().cast(), bytes) };
        buf.truncate(got.max(0) as usize);
        buf.into_iter()
            .filter(|&p| p > 0)
            .map(|p| p as u32)
            .collect()
    }

    pub fn info(pid: u32) -> Option<ProcInfo> {
        let ipid = libc::c_int::try_from(pid).ok()?;
        let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_taskallinfo>()).ok()?;
        // SAFETY: düz C yapısı; proc_pidinfo en fazla `size` bayt yazar.
        let mut ti: libc::proc_taskallinfo = unsafe { std::mem::zeroed() };
        let got = unsafe {
            libc::proc_pidinfo(
                ipid,
                libc::PROC_PIDTASKALLINFO,
                0,
                (&raw mut ti).cast(),
                size,
            )
        };
        if got != size {
            return None;
        }
        let b = &ti.pbsd;
        // SAFETY: pbi_name / pbi_comm NUL ile biten sabit dizilerdir.
        let name = unsafe { CStr::from_ptr(b.pbi_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let name = if name.is_empty() {
            unsafe { CStr::from_ptr(b.pbi_comm.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        } else {
            name
        };
        let (numer, denom) = timebase();
        let ticks = ti.ptinfo.pti_total_user + ti.ptinfo.pti_total_system;
        Some(ProcInfo {
            pid,
            ppid: b.pbi_ppid,
            pgid: b.pbi_pgid,
            uid: b.pbi_uid,
            // NODEV (-1) terminal yok demektir
            has_tty: b.e_tdev != u32::MAX && b.e_tdev != 0,
            name,
            start_secs: i64::try_from(b.pbi_start_tvsec).unwrap_or(0),
            cpu_ns: ticks.saturating_mul(numer) / denom,
        })
    }

    pub fn path(pid: u32) -> Option<String> {
        let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let ipid = libc::c_int::try_from(pid).ok()?;
        // SAFETY: arabellek PROC_PIDPATHINFO_MAXSIZE uzunluğunda.
        let n = unsafe { libc::proc_pidpath(ipid, buf.as_mut_ptr().cast(), buf.len() as u32) };
        (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
    }

    pub fn cwd(pid: u32) -> Option<String> {
        let ipid = libc::c_int::try_from(pid).ok()?;
        let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_vnodepathinfo>()).ok()?;
        // SAFETY: düz C yapısı; en fazla `size` bayt yazılır.
        let mut vi: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
        let got = unsafe {
            libc::proc_pidinfo(
                ipid,
                libc::PROC_PIDVNODEPATHINFO,
                0,
                (&raw mut vi).cast(),
                size,
            )
        };
        if got != size {
            return None;
        }
        // SAFETY: vip_path NUL ile biten sabit dizi.
        let p = unsafe { CStr::from_ptr(vi.pvi_cdir.vip_path.as_ptr().cast()) };
        Some(p.to_string_lossy().into_owned()).filter(|s| !s.is_empty())
    }

    pub fn current_uid() -> u32 {
        // SAFETY: yan etkisiz.
        unsafe { libc::getuid() }
    }

    /// Sürecin argümanları (argv). `KERN_PROCARGS2`: [argc][exec yolu\0…][argv\0…][env…]. Başka kullanıcının
    /// sürecinde izin yoktur (`None`).
    pub fn args(pid: u32) -> Option<Vec<String>> {
        let mut mib = [
            libc::CTL_KERN,
            libc::KERN_PROCARGS2,
            libc::c_int::try_from(pid).ok()?,
        ];
        let mut size: libc::size_t = 0;
        // SAFETY: önce boyut sorulur, sonra o kadar yer ayrılıp doldurulur.
        if unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                std::ptr::null_mut(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        } != 0
        {
            return None;
        }
        let mut buf = vec![0u8; size];
        if unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                buf.as_mut_ptr().cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        } != 0
        {
            return None;
        }
        buf.truncate(size);
        let argc = i32::from_ne_bytes(buf.get(..4)?.try_into().ok()?).max(0) as usize;
        let mut rest = &buf[4..];
        // exec yolu ve ardından gelen NUL dolgusu
        let end = rest.iter().position(|&b| b == 0)?;
        rest = &rest[end..];
        let start = rest.iter().position(|&b| b != 0)?;
        rest = &rest[start..];
        Some(
            rest.split(|&b| b == 0)
                .take(argc)
                .map(|a| String::from_utf8_lossy(a).into_owned())
                .collect(),
        )
    }

    /// Kontrol terminalinin aygıt yolu (`/dev/ttys003`); yoksa `None`.
    pub fn tty(pid: u32) -> Option<String> {
        let ipid = libc::c_int::try_from(pid).ok()?;
        let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
        // SAFETY: düz C yapısı; en fazla `size` bayt yazılır.
        let mut b: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let got = unsafe {
            libc::proc_pidinfo(ipid, libc::PROC_PIDTBSDINFO, 0, (&raw mut b).cast(), size)
        };
        if got != size || b.e_tdev == u32::MAX || b.e_tdev == 0 {
            return None;
        }
        // SAFETY: devname statik bir arabellek döndürür (ya da null); hemen kopyalanır.
        let name = unsafe { libc::devname(b.e_tdev as libc::dev_t, libc::S_IFCHR) };
        if name.is_null() {
            return None;
        }
        let n = unsafe { CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned();
        Some(format!("/dev/{n}"))
    }
}

#[cfg(windows)]
mod imp {
    use super::ProcInfo;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    fn snapshot() -> Vec<(u32, u32, String)> {
        let mut out = Vec::new();
        // SAFETY: anlık görüntü tutamacı sonunda kapatılır; yapı boyutu dwSize ile bildirilir.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snap, &mut e);
            while ok != 0 {
                let len = e
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(e.szExeFile.len());
                out.push((
                    e.th32ProcessID,
                    e.th32ParentProcessID,
                    String::from_utf16_lossy(&e.szExeFile[..len]),
                ));
                ok = Process32NextW(snap, &mut e);
            }
            CloseHandle(snap);
        }
        out
    }

    fn times(pid: u32) -> Option<(i64, u64)> {
        // SAFETY: tutamaç açılır ve kapanır; dört yazılabilir FILETIME.
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let z = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let (mut c, mut e, mut k, mut u) = (z, z, z, z);
            let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u);
            CloseHandle(h);
            if ok == 0 {
                return None;
            }
            let ft = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
            let start = (ft(c).saturating_sub(116_444_736_000_000_000) / 10_000_000) as i64;
            Some((start, (ft(k) + ft(u)) * 100))
        }
    }

    pub fn all_pids() -> Vec<u32> {
        snapshot().into_iter().map(|(p, _, _)| p).collect()
    }

    pub fn info(pid: u32) -> Option<ProcInfo> {
        let (_, ppid, name) = snapshot().into_iter().find(|(p, _, _)| *p == pid)?;
        let (start_secs, cpu_ns) = times(pid).unwrap_or((0, 0));
        Some(ProcInfo {
            pid,
            ppid,
            pgid: pid,
            uid: 0,
            has_tty: false,
            name,
            start_secs,
            cpu_ns,
        })
    }

    /// Bütün tablo tek anlık görüntüden (Windows'ta her süreç için ayrı anlık görüntü pahalı).
    pub fn table() -> Vec<ProcInfo> {
        snapshot()
            .into_iter()
            .map(|(pid, ppid, name)| {
                let (start_secs, cpu_ns) = times(pid).unwrap_or((0, 0));
                ProcInfo {
                    pid,
                    ppid,
                    pgid: pid,
                    uid: 0,
                    has_tty: false,
                    name,
                    start_secs,
                    cpu_ns,
                }
            })
            .collect()
    }

    pub fn path(_pid: u32) -> Option<String> {
        None
    }

    pub fn cwd(_pid: u32) -> Option<String> {
        None
    }

    pub fn current_uid() -> u32 {
        0
    }

    pub fn tty(_pid: u32) -> Option<String> {
        None
    }

    pub fn args(_pid: u32) -> Option<Vec<String>> {
        None
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod imp {
    use super::ProcInfo;
    pub fn all_pids() -> Vec<u32> {
        Vec::new()
    }
    pub fn info(_pid: u32) -> Option<ProcInfo> {
        None
    }
    pub fn path(_pid: u32) -> Option<String> {
        None
    }
    pub fn cwd(_pid: u32) -> Option<String> {
        None
    }
    pub fn current_uid() -> u32 {
        0
    }
    pub fn tty(_pid: u32) -> Option<String> {
        None
    }
    pub fn args(_pid: u32) -> Option<Vec<String>> {
        None
    }
}

pub use imp::{all_pids, args, current_uid, cwd, info, path, tty};

/// Bütün süreçlerin bilgisi.
pub fn table() -> Vec<ProcInfo> {
    #[cfg(windows)]
    {
        imp::table()
    }
    #[cfg(not(windows))]
    {
        all_pids().into_iter().filter_map(info).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, ppid: u32) -> ProcInfo {
        ProcInfo {
            pid,
            ppid,
            pgid: pid,
            uid: 0,
            has_tty: false,
            name: String::new(),
            start_secs: 0,
            cpu_ns: 0,
        }
    }

    #[test]
    fn finds_all_descendants() {
        let t = [p(1, 0), p(10, 1), p(11, 10), p(12, 11), p(13, 10), p(20, 1)];
        let mut d = descendants(10, &t);
        d.sort();
        assert_eq!(d, [11, 12, 13]);
        assert!(descendants(20, &t).is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reads_own_process() {
        let me = std::process::id();
        let i = info(me).expect("kendi sürecimiz okunmalı");
        assert_eq!(i.pid, me);
        assert_eq!(i.uid, current_uid());
        assert!(!i.name.is_empty());
        assert!(path(me).is_some_and(|p| p.contains("kaans_terminal") || p.contains("deps")));
        assert!(cwd(me).is_some());
        assert!(all_pids().contains(&me));
        let argv = args(me).expect("kendi argümanlarımız okunmalı");
        assert!(argv[0].contains("kaans_terminal"), "{argv:?}");
        let t = table();
        assert!(t.iter().any(|x| x.pid == me));
        // CPU zamanı nanosaniye: test süreci en az biraz CPU harcamıştır, ama saatler değil.
        assert!(i.cpu_ns > 0 && i.cpu_ns < 3_600_000_000_000);
    }
}
