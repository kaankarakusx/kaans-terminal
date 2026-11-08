//! Oturum sunucusu ↔ uygulama protokolü (yerel soket / named pipe).
//!
//! Çerçeve: `[u32 BE uzunluk][u8 tür][u16 BE id uzunluğu][id][veri]`; uzunluk türden sonrasını sayar.
//! - `CONTROL`: id boş, veri JSON (`Request` / `Event`)
//! - `INPUT` / `OUTPUT`: id = oturum kimliği, veri = ham bayt (klavye / terminal çıktısı)

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// Sunucu ve uygulama farklı sürümdeyse (güncelleme sonrası) bunu karşılaştırır.
pub const PROTOCOL_VERSION: u32 = 1;
/// Tek bir çerçevenin en büyük boyutu (bozuk akışa karşı).
const MAX_FRAME: usize = 16 * 1024 * 1024;

pub const CONTROL: u8 = 1;
pub const INPUT: u8 = 2;
pub const OUTPUT: u8 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpec {
    pub cwd: String,
    /// Boşsa kullanıcının kabuğu (login shell).
    #[serde(default)]
    pub command: Vec<String>,
    pub cols: u16,
    pub rows: u16,
    #[serde(default)]
    pub env: Vec<(String, String)>,
}

/// Bir terminal oturumu (uygulama ve engine bu bilgiyi kullanır).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub cwd: String,
    pub command: Vec<String>,
    /// Kabuk (ya da komut) sürecinin pid'i
    pub pid: Option<u32>,
    /// Ön plandaki süreç grubunun lideri ve adı (kabuk boştaysa kabuğun kendisi)
    pub fg_pid: Option<u32>,
    pub fg_name: Option<String>,
    /// Ön planda kabuktan başka bir şey çalışıyor (komut koşuyor)
    pub busy: bool,
    pub started: i64,
    pub cols: u16,
    pub rows: u16,
    pub attached: u32,
}

/// Kapanan bir terminalden geriye kalan süreç (örn. port tutan dev server).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub pid: u32,
    pub name: String,
    pub cwd: String,
    /// Hangi terminalden kaldığı
    pub session: String,
    pub since: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum Request {
    Hello {
        version: u32,
    },
    List,
    Create {
        req: u64,
        spec: CreateSpec,
    },
    Attach {
        id: String,
    },
    Detach {
        id: String,
    },
    Resize {
        id: String,
        cols: u16,
        rows: u16,
    },
    /// End Session: SIGHUP → SIGTERM → SIGKILL, alt süreçler dahil.
    Kill {
        id: String,
    },
    /// Kapanmış terminalden kalan bir süreci bitir.
    KillLeftover {
        pid: u32,
    },
    /// Oturum yoksa sunucuyu kapat (sürüm uyuşmazlığında).
    ShutdownIfIdle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum Event {
    Hello {
        version: u32,
        pid: u32,
    },
    Sessions {
        list: Vec<SessionInfo>,
        leftovers: Vec<Leftover>,
    },
    Created {
        req: u64,
        id: String,
    },
    /// Bağlanınca geçmişin sonu: bundan sonrası canlı çıktı.
    Attached {
        id: String,
    },
    /// Ayrılma onayı: istemci bunu alınca bağlantıyı kapatır.
    Detached {
        id: String,
    },
    /// `ended`: End Session ile bitirildi (haritada araba patlar); false = kabuk kendisi kapandı.
    Exit {
        id: String,
        code: Option<u32>,
        #[serde(default)]
        ended: bool,
    },
    Error {
        req: Option<u64>,
        message: String,
    },
}

pub fn write_frame(w: &mut impl Write, kind: u8, id: &str, payload: &[u8]) -> io::Result<()> {
    let id = id.as_bytes();
    let id_len = u16::try_from(id.len()).map_err(|_| io::Error::other("id çok uzun"))?;
    let len = 1 + 2 + id.len() + payload.len();
    let len32 = u32::try_from(len).map_err(|_| io::Error::other("çerçeve çok büyük"))?;
    let mut buf = Vec::with_capacity(4 + len);
    buf.extend_from_slice(&len32.to_be_bytes());
    buf.push(kind);
    buf.extend_from_slice(&id_len.to_be_bytes());
    buf.extend_from_slice(id);
    buf.extend_from_slice(payload);
    w.write_all(&buf)?;
    w.flush()
}

pub fn write_json<T: Serialize>(w: &mut impl Write, msg: &T) -> io::Result<()> {
    let body = serde_json::to_vec(msg).map_err(io::Error::other)?;
    write_frame(w, CONTROL, "", &body)
}

/// Bir çerçeve okur: (tür, id, veri). Akış kapandıysa `UnexpectedEof`.
pub fn read_frame(r: &mut impl Read) -> io::Result<(u8, String, Vec<u8>)> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if !(3..=MAX_FRAME).contains(&len) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "geçersiz çerçeve boyu",
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    let kind = buf[0];
    let id_len = u16::from_be_bytes([buf[1], buf[2]]) as usize;
    if 3 + id_len > len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "geçersiz id boyu",
        ));
    }
    let id = String::from_utf8_lossy(&buf[3..3 + id_len]).into_owned();
    let payload = buf.split_off(3 + id_len);
    Ok((kind, id, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, OUTPUT, "t1", b"\x1b[31mhi\r\n").unwrap();
        write_json(
            &mut buf,
            &Request::Resize {
                id: "t1".into(),
                cols: 80,
                rows: 24,
            },
        )
        .unwrap();
        let mut r = buf.as_slice();
        assert_eq!(
            read_frame(&mut r).unwrap(),
            (OUTPUT, "t1".into(), b"\x1b[31mhi\r\n".to_vec())
        );
        let (k, id, body) = read_frame(&mut r).unwrap();
        assert_eq!((k, id.as_str()), (CONTROL, ""));
        assert_eq!(
            serde_json::from_slice::<Request>(&body).unwrap(),
            Request::Resize {
                id: "t1".into(),
                cols: 80,
                rows: 24
            }
        );
        assert_eq!(
            read_frame(&mut r).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn rejects_garbage_lengths() {
        let mut r: &[u8] = &[0xff, 0xff, 0xff, 0xff, 1];
        assert_eq!(
            read_frame(&mut r).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let mut r: &[u8] = &[0, 0, 0, 3, 1, 0, 9];
        assert_eq!(
            read_frame(&mut r).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
