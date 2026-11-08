//! Kalıcı terminal oturumları (MIMARI.md §4): ayrı bir sunucu süreci PTY'leri tutar.
//! Uygulama kapansa da güncellense de terminaller yaşar; uygulama yeniden bağlanır.

pub mod client;
pub mod protocol;
pub mod server;

#[cfg(all(test, unix))]
mod tests;

use interprocess::local_socket::{Name, Stream};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

/// Okuma ve yazma iş parçacıklarının paylaştığı akış (`&Stream` hem `Read` hem `Write`).
#[derive(Clone)]
pub struct SharedStream(pub Arc<Stream>);

impl Read for SharedStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self.0).read(buf)
    }
}

impl Write for SharedStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self.0).write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}

/// Sunucu kipini başlatan argüman (`kaans-terminal --session-host <soket>`).
pub const HOST_ARG: &str = "--session-host";

/// Kullanıcıya özel soket yolu. Mac'te `$TMPDIR` kullanıcıya özeldir (0700); Unix soket yolu 104 baytla sınırlı.
pub fn default_socket() -> PathBuf {
    #[cfg(unix)]
    {
        std::env::temp_dir().join("kaans-terminal-host.sock")
    }
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap_or_default();
        PathBuf::from(format!("kaans-terminal-host-{user}"))
    }
}

pub fn socket_name(socket: &Path) -> io::Result<Name<'static>> {
    #[cfg(unix)]
    {
        use interprocess::local_socket::{GenericFilePath, ToFsName};
        socket.to_path_buf().to_fs_name::<GenericFilePath>()
    }
    #[cfg(windows)]
    {
        use interprocess::local_socket::{GenericNamespaced, ToNsName};
        socket
            .to_string_lossy()
            .into_owned()
            .to_ns_name::<GenericNamespaced>()
    }
}

/// Sunucuyu uygulamadan bağımsız (ayrı oturum / süreç grubu) başlatır; uygulama kapanınca ölmez.
pub fn spawn_host(socket: &Path) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let log = std::env::temp_dir().join("kaans-terminal-host.log");
    let mut cmd = Command::new(exe);
    cmd.arg(HOST_ARG)
        .arg(socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(
            std::fs::File::create(log)
                .map(Stdio::from)
                .unwrap_or_else(|_| Stdio::null()),
        );
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: fork sonrası yalnızca async-signal-safe setsid çağrılır.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let mut child = cmd.spawn()?;
    // Sunucu bizden önce biterse zombi kalmasın.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
