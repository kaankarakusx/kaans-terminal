//! Canlı bağlantı (WS `/ws`, MIMARI.md §6.1): arkadaşların durumu, yeni mesaj ve istek olayları. Token yalnızca
//! burada kullanılır; sayfa olayları `social://live` ile alır. Cloudflare boştaki bağlantıyı kapatmasın diye 30 sn'de
//! bir ping; kullanıcı 5 dk klavyeden uzaksa sunucuya "away". Koparsa artan beklemeyle yeniden bağlanır.
use crate::activity;
use crate::social::Social;
use serde_json::{Value, json};
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tungstenite::client::IntoClientRequest;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

const PING: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_secs(2);
const BACKOFF_MAX: Duration = Duration::from_secs(60);
/// Sunucunun "geçersiz token" kapanış kodu
const UNAUTHORIZED: u16 = 4401;

pub const EV_LIVE: &str = "social://live";
pub const EV_LIVE_STATE: &str = "social://live-state";

/// `http(s)://host` → `ws(s)://host/ws`
pub fn ws_url(base: &str) -> String {
    let b = base.trim_end_matches('/');
    let b = if let Some(rest) = b.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = b.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        b.to_string()
    };
    format!("{b}/ws")
}

fn set_timeout(ws: &mut WebSocket<MaybeTlsStream<TcpStream>>, d: Duration) {
    let tcp = match ws.get_mut() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::Rustls(s) => s.get_mut(),
        _ => return,
    };
    let _ = tcp.set_read_timeout(Some(d));
}

pub enum End {
    /// Token geçersiz: yeniden kayıt gerekene kadar bekle
    Unauthorized,
    Lost,
}

/// Tek bağlantı: olaylar `on_event`'e gider; bağlanınca `on_open`. Bağlantı bitince döner.
pub fn session(
    social: &Social,
    on_open: &mut dyn FnMut(),
    on_event: &mut dyn FnMut(Value),
) -> Result<End, String> {
    let auth = social.auth_header().ok_or("not registered")?;
    let mut req = ws_url(social.base())
        .into_client_request()
        .map_err(|e| e.to_string())?;
    req.headers_mut()
        .insert("authorization", auth.parse().map_err(|_| "bad token")?);
    let (mut ws, _) = tungstenite::connect(req).map_err(|e| e.to_string())?;
    set_timeout(&mut ws, POLL);
    on_open();
    let mut last_ping = Instant::now();
    let mut away: Option<bool> = None;
    loop {
        match ws.read() {
            Ok(Message::Text(t)) => {
                if let Ok(mut v) = serde_json::from_str::<Value>(&t) {
                    // Gelen şifreli mesaj burada çözülür; sayfa yalnızca düz metni görür
                    if v["type"] == "message" {
                        social.open_message(&mut v, false);
                    }
                    on_event(v);
                }
            }
            Ok(Message::Close(frame)) => {
                return Ok(match frame {
                    Some(f) if u16::from(f.code) == UNAUTHORIZED => End::Unauthorized,
                    _ => End::Lost,
                });
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return Ok(End::Lost),
        }
        // AFK durumu değişince bildir
        let now_away = activity::system_idle_secs().is_some_and(|s| s >= activity::WINDOW_SECS);
        if away != Some(now_away) {
            away = Some(now_away);
            if ws
                .send(Message::text(
                    json!({ "type": "away", "away": now_away }).to_string(),
                ))
                .is_err()
            {
                return Ok(End::Lost);
            }
        }
        if last_ping.elapsed() >= PING {
            last_ping = Instant::now();
            if ws.send(Message::Ping(Vec::new().into())).is_err() {
                return Ok(End::Lost);
            }
        }
    }
}

/// Uygulama boyunca çalışır: kayıtlı değilken bekler, kayıt olunca bağlanır.
pub fn run(app: AppHandle, social: Arc<Social>) {
    let mut backoff = Duration::from_secs(2);
    loop {
        if social.auth_header().is_none() {
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }
        let started = Instant::now();
        let end = session(
            &social,
            &mut || {
                let _ = app.emit(EV_LIVE_STATE, json!({ "connected": true }));
            },
            &mut |v| {
                let _ = app.emit(EV_LIVE, v);
            },
        );
        let _ = app.emit(EV_LIVE_STATE, json!({ "connected": false }));
        match end {
            Ok(End::Unauthorized) => {
                std::thread::sleep(BACKOFF_MAX);
                continue;
            }
            // Uzun süre bağlı kaldıysa bekleme baştan
            _ if started.elapsed() > Duration::from_secs(60) => backoff = Duration::from_secs(2),
            _ => {}
        }
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(BACKOFF_MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gerçek sunucuyla: iki cihaz arkadaş olur, biri mesaj atar, diğerinin canlı bağlantısına anında gelir.
    /// `KAANS_TEST_SERVER` yoksa atlanır (social.rs'deki uçtan uca testle aynı kurulum).
    #[test]
    fn live_message_round_trip() {
        let Ok(url) = std::env::var("KAANS_TEST_SERVER") else {
            eprintln!("KAANS_TEST_SERVER yok, atlandı");
            return;
        };
        let a_dir = tempfile::tempdir().unwrap();
        let b_dir = tempfile::tempdir().unwrap();
        let a = Arc::new(Social::new(a_dir.path(), url.clone()));
        let b = Social::new(b_dir.path(), url);
        // İkinci "cihaz": donanım kimliği yerine kendi kimlik dosyası (aynı makinede iki kayıt)
        let sa = a.register_with_hash("e2e_alice", &"a1".repeat(32)).unwrap();
        let sb = b.register_with_hash("e2e_bob", &"b2".repeat(32)).unwrap();
        let tag = |s: &crate::social::SocialState| {
            format!("{}#{}", s.name.clone().unwrap(), s.tag.clone().unwrap())
        };

        let (tx, rx) = std::sync::mpsc::channel::<Value>();
        let live_a = a.clone();
        std::thread::spawn(move || {
            let _ = session(&live_a, &mut || {}, &mut |v| {
                let _ = tx.send(v);
            });
        });
        let wait = |ty: &str| loop {
            let v = rx
                .recv_timeout(Duration::from_secs(5))
                .expect("olay gelmedi");
            if v["type"] == ty {
                return v;
            }
        };
        assert_eq!(wait("hello")["name"], "e2e_alice");
        b.post("friends.add", json!({ "nameTag": tag(&sa) }))
            .unwrap();
        assert_eq!(wait("request")["from"]["name"], "e2e_bob");
        a.post("friends.accept", json!({ "nameTag": tag(&sb) }))
            .unwrap();
        // Uçtan uca şifreli: alıcının açık anahtarı sunucuda olmalı
        a.ensure_key().unwrap();
        b.post(
            "messages.send",
            json!({ "to": tag(&sa), "body": "canlı mı? 🔒" }),
        )
        .unwrap();
        let m = wait("message");
        assert_eq!(
            (m["body"].as_str(), m["from"]["name"].as_str()),
            (Some("canlı mı? 🔒"), Some("e2e_bob"))
        );
        assert_eq!(m["locked"], false);
        assert!(
            m.get("ciphertext").is_none(),
            "şifreli alanlar sayfaya gitmez"
        );
        // Geçmiş iki tarafta da çözülür (gönderen kendi mesajını okur)
        for (who, mine) in [(&*a, false), (&b, true)] {
            let other = if mine { tag(&sa) } else { tag(&sb) };
            let h = who
                .get(&format!("messages?with={}", other.replace('#', "%23")))
                .unwrap();
            let last = h["messages"].as_array().unwrap().last().unwrap().clone();
            assert_eq!(
                (last["body"].as_str(), last["mine"].as_bool()),
                (Some("canlı mı? 🔒"), Some(mine))
            );
        }
        assert!(a.post("nope", json!({})).is_err(), "izin listesi dışı");
    }

    #[test]
    fn maps_http_base_to_ws_endpoint() {
        assert_eq!(
            ws_url("https://kaansterminal.karakuslabs.com"),
            "wss://kaansterminal.karakuslabs.com/ws"
        );
        assert_eq!(ws_url("http://127.0.0.1:8787/"), "ws://127.0.0.1:8787/ws");
    }
}
