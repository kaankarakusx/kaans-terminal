//! Sosyal sunucu istemcisi (MIMARI.md §6). Bütün istekler Rust'tan gider: cihaz token'ı web sayfasına hiç girmez
//! ve sayfanın CSP'si değişmez. Sunucuya giden: hacker name, tag, uygulama/OS sürümü, dakikalık aktiflik, saat
//! dilimi, tema (MIMARI.md §9). Oturum verisi gitmez.
//!
//! Cihaz kimliği: donanım UUID'sinin (Mac `gethostuuid`, Windows MachineGuid) uygulamaya özel tuzla SHA-256'sı;
//! okunamazsa uygulama klasöründe saklanan rastgele bir kimlik.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const ACCOUNT: &str = "account.json";
const DEVICE_ID: &str = "device-id";
const SALT: &str = "kaans-terminal/device/v1:";
const PROD: &str = "https://kaansterminal.karakuslabs.com";
const DEV: &str = "http://127.0.0.1:8787";

/// Sunucu adresi: `KAANS_SERVER`, yoksa geliştirmede yerel sunucu, yayında alan adı.
pub fn server_url() -> String {
    std::env::var("KAANS_SERVER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| if cfg!(debug_assertions) { DEV } else { PROD }.to_string())
        .trim_end_matches('/')
        .to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Account {
    pub token: String,
    pub name: String,
    pub tag: String,
    /// Hesabın açıldığı sunucu. Başka sunucunun hesabı kullanılmaz (geliştirmede yerel sunucuda açılan hesap, kurulu
    /// sürümde gerçek sunucuya gönderilmesin). Eski dosyalarda boş: sunucu 401 derse sıfırlanır.
    #[serde(default)]
    pub server: String,
}

/// Sayfaya giden durum (token yok)
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SocialState {
    pub server: String,
    pub registered: bool,
    pub name: Option<String>,
    pub tag: Option<String>,
}

pub struct Social {
    base: String,
    dir: PathBuf,
    agent: ureq::Agent,
    account: Mutex<Option<Account>>,
    /// Uçtan uca şifreleme anahtarı (gizli anahtar bu cihazda)
    keys: crate::e2e::Keys,
    /// Arkadaşların açık anahtarları (name#tag → hex); gönderirken, gerekirse sunucudan tazelenir
    friend_keys: Mutex<std::collections::HashMap<String, String>>,
    /// Açık anahtar bu oturumda sunucuya yüklendi mi
    key_uploaded: std::sync::atomic::AtomicBool,
}

/// Mesaj metni sınırı (karakter); şifreli hali sunucunun sınırına sığar
const MAX_MESSAGE: usize = 2000;

/// Arayüze giden hata: `offline` (sunucuya ulaşılamadı) ya da sunucunun mesajı
#[derive(Debug, PartialEq)]
pub enum SocialError {
    Offline,
    Unauthorized,
    Server(u16, String),
}

impl std::fmt::Display for SocialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Offline => f.write_str("offline"),
            Self::Unauthorized => f.write_str("unauthorized"),
            Self::Server(_, m) => f.write_str(m),
        }
    }
}

impl Social {
    pub fn new(app_data: &Path, base: String) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .http_status_as_error(false)
            .user_agent(format!("kaans-terminal/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .new_agent();
        let account = std::fs::read(app_data.join(ACCOUNT))
            .ok()
            .and_then(|b| serde_json::from_slice::<Account>(&b).ok())
            .filter(|a| a.server.is_empty() || a.server == base);
        Self {
            base,
            dir: app_data.to_path_buf(),
            agent,
            account: Mutex::new(account),
            keys: crate::e2e::Keys::load_or_create(app_data),
            friend_keys: Mutex::new(std::collections::HashMap::new()),
            key_uploaded: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// Canlı bağlantı için (yalnızca Rust içinde kullanılır)
    pub fn auth_header(&self) -> Option<String> {
        self.account().map(|a| format!("Bearer {}", a.token))
    }

    fn account(&self) -> Option<Account> {
        self.account
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn state(&self) -> SocialState {
        let a = self.account();
        SocialState {
            server: self.base.clone(),
            registered: a.is_some(),
            name: a.as_ref().map(|a| a.name.clone()),
            tag: a.map(|a| a.tag),
        }
    }

    fn save(&self, a: Option<Account>) -> Result<(), SocialError> {
        let path = self.dir.join(ACCOUNT);
        match &a {
            Some(acc) => write_private(&path, &serde_json::to_vec(acc).unwrap_or_default())
                .map_err(|e| SocialError::Server(0, e.to_string()))?,
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
        *self.account.lock().unwrap_or_else(|e| e.into_inner()) = a;
        Ok(())
    }

    fn drop_stale_account(&self) {
        let path = self.dir.join(ACCOUNT);
        let _ = std::fs::rename(&path, self.dir.join("account.stale.json"));
        *self.account.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.key_uploaded
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    fn call(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, SocialError> {
        let url = format!("{}/api/v1/{path}", self.base);
        let token = self.account().map(|a| a.token);
        let auth = token.map(|t| format!("Bearer {t}"));
        let res = match (method, body) {
            ("GET", _) => {
                let mut r = self.agent.get(&url);
                if let Some(a) = &auth {
                    r = r.header("authorization", a);
                }
                r.call()
            }
            (m, b) => {
                let mut r = match m {
                    "PATCH" => self.agent.patch(&url),
                    "PUT" => self.agent.put(&url),
                    _ => self.agent.post(&url),
                };
                if let Some(a) = &auth {
                    r = r.header("authorization", a);
                }
                r.send_json(b.unwrap_or_else(|| json!({})))
            }
        };
        let mut res = res.map_err(|_| SocialError::Offline)?;
        let status = res.status().as_u16();
        let value: Value = res.body_mut().read_json().unwrap_or(Value::Null);
        match status {
            200..=299 => Ok(value),
            401 => {
                // Sunucu bu token'ı tanımıyor (başka sunucuda açılmış ya da silinmiş hesap): hesap bırakılır,
                // sayfa hacker name penceresini yeniden açar. Dosya silinmez, yanına `account.stale.json` olarak taşınır.
                if auth.is_some() {
                    self.drop_stale_account();
                }
                Err(SocialError::Unauthorized)
            }
            _ => Err(SocialError::Server(
                status,
                value["error"]
                    .as_str()
                    .unwrap_or("server error")
                    .to_string(),
            )),
        }
    }

    /// Hacker name seç: cihazı kaydeder, token'ı yalnızca bu cihazda (0600) saklar.
    pub fn register(&self, name: &str) -> Result<SocialState, SocialError> {
        let v = self.call(
            "POST",
            "devices",
            Some(json!({
                "deviceHash": device_hash(&self.dir),
                "hackerName": name,
                "os": std::env::consts::OS,
                "appVersion": env!("CARGO_PKG_VERSION"),
            })),
        )?;
        let acc = Account {
            token: v["token"].as_str().unwrap_or_default().to_string(),
            name: v["name"].as_str().unwrap_or(name).to_string(),
            tag: v["tag"].as_str().unwrap_or_default().to_string(),
            server: self.base.clone(),
        };
        if acc.token.is_empty() {
            return Err(SocialError::Server(0, "bad response".into()));
        }
        self.save(Some(acc))?;
        let _ = self.ensure_key();
        Ok(self.state())
    }

    /// Açık anahtarı sunucuya yükler (oturumda bir kez; kayıt sonrası ve açılışta).
    pub fn ensure_key(&self) -> Result<(), SocialError> {
        use std::sync::atomic::Ordering;
        if self.account().is_none() || self.key_uploaded.load(Ordering::Relaxed) {
            return Ok(());
        }
        self.call(
            "PUT",
            "me/key",
            Some(json!({ "publicKey": self.keys.public_hex() })),
        )?;
        self.key_uploaded.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Arkadaşın açık anahtarı (önbellek; `refresh` ise sunucudan tazele)
    fn friend_key(&self, name_tag: &str, refresh: bool) -> Result<String, SocialError> {
        if !refresh
            && let Some(k) = self
                .friend_keys
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(name_tag)
        {
            return Ok(k.clone());
        }
        let v = self.call("GET", "friends", None)?;
        let mut map = self.friend_keys.lock().unwrap_or_else(|e| e.into_inner());
        map.clear();
        for f in v["friends"].as_array().into_iter().flatten() {
            if let (Some(n), Some(t), Some(k)) = (
                f["name"].as_str(),
                f["tag"].as_str(),
                f["publicKey"].as_str(),
            ) {
                map.insert(format!("{n}#{t}"), k.to_string());
            }
        }
        map.get(name_tag).cloned().ok_or_else(|| {
            SocialError::Server(
                409,
                format!(
                    "{} hasn't set up encryption yet",
                    name_tag.split('#').next().unwrap_or(name_tag)
                ),
            )
        })
    }

    /// Şifreli mesaj gönder. Alıcının anahtarı değiştiyse bir kez tazeleyip yeniden şifreler.
    fn send_message(&self, to: &str, text: &str) -> Result<Value, SocialError> {
        let text = text.replace("\r\n", "\n");
        if text.trim().is_empty() {
            return Err(SocialError::Server(400, "empty message".into()));
        }
        if text.chars().count() > MAX_MESSAGE {
            return Err(SocialError::Server(
                400,
                format!("messages are limited to {MAX_MESSAGE} characters"),
            ));
        }
        self.ensure_key()?;
        let mut refresh = false;
        loop {
            let key = self.friend_key(to, refresh)?;
            let (ct, nonce) = self
                .keys
                .encrypt(&key, &text)
                .ok_or_else(|| SocialError::Server(0, "bad key".into()))?;
            let r = self.call(
                "POST",
                "messages",
                Some(json!({ "to": to, "ciphertext": ct, "nonce": nonce, "toKey": key })),
            );
            match r {
                Err(SocialError::Server(409, m)) if m.contains("key changed") && !refresh => {
                    refresh = true
                }
                other => return other,
            }
        }
    }

    /// Şifreli bir mesaj nesnesini (geçmiş satırı ya da canlı olay) çözüp düz metne çevirir: `body` (çözülemezse
    /// null ve `locked: true`); şifreli alanlar sayfaya gitmez.
    pub fn open_message(&self, m: &mut Value, mine: bool) {
        let s = |k: &str| m[k].as_str().unwrap_or_default().to_string();
        let body = self.keys.decrypt(
            mine,
            &s("fromKey"),
            &s("toKey"),
            &s("nonce"),
            &s("ciphertext"),
        );
        if let Some(o) = m.as_object_mut() {
            for k in ["ciphertext", "nonce", "fromKey", "toKey"] {
                o.remove(k);
            }
            o.insert("locked".into(), json!(body.is_none()));
            o.insert("body".into(), body.map_or(Value::Null, Value::String));
        }
    }

    /// Testler: aynı makinede ikinci bir cihaz gibi kayıt
    #[cfg(test)]
    pub fn register_with_hash(&self, name: &str, hash: &str) -> Result<SocialState, SocialError> {
        let v = self.call(
            "POST",
            "devices",
            Some(json!({ "deviceHash": hash, "hackerName": name })),
        )?;
        self.save(Some(Account {
            token: v["token"].as_str().unwrap_or_default().to_string(),
            name: v["name"].as_str().unwrap_or(name).to_string(),
            tag: v["tag"].as_str().unwrap_or_default().to_string(),
            server: self.base.clone(),
        }))?;
        Ok(self.state())
    }

    pub fn rename(&self, name: &str) -> Result<SocialState, SocialError> {
        let v = self.call("PATCH", "me", Some(json!({ "hackerName": name })))?;
        if let Some(mut a) = self.account() {
            a.name = v["name"].as_str().unwrap_or(name).to_string();
            a.tag = v["tag"].as_str().unwrap_or(&a.tag).to_string();
            self.save(Some(a))?;
        }
        Ok(self.state())
    }

    /// Dakikalık atış. Kayıtlı değilse gönderilmez.
    pub fn heartbeat(
        &self,
        active: bool,
        tz_min: i32,
        theme: Option<&str>,
    ) -> Result<Option<Value>, SocialError> {
        if self.account().is_none() {
            return Ok(None);
        }
        let mut body =
            json!({ "active": active, "tz": tz_min, "appVersion": env!("CARGO_PKG_VERSION") });
        if let Some(t) = theme {
            body["theme"] = json!(t);
        }
        self.call("POST", "heartbeat", Some(body)).map(Some)
    }

    pub fn get(&self, path: &str) -> Result<Value, SocialError> {
        let mut v = self.call("GET", path, None)?;
        // Mesaj geçmişi: burada çözülür, sayfa düz metni görür
        if path.starts_with("messages?")
            && let Some(list) = v["messages"].as_array_mut()
        {
            for m in list {
                let mine = m["mine"].as_bool().unwrap_or(false);
                self.open_message(m, mine);
            }
        }
        Ok(v)
    }

    /// İzin listesindeki yazma uçları (sayfa yalnızca bunları çağırabilir)
    pub fn post(&self, what: &str, body: Value) -> Result<Value, SocialError> {
        let (method, path) = match what {
            "friends.add" => ("POST", "friends"),
            "friends.accept" => ("POST", "friends/accept"),
            "friends.remove" => ("POST", "friends/remove"),
            "messages.send" => ("POST", "messages"),
            "presence" => ("PATCH", "me/presence"),
            "feedback" => ("POST", "feedback"),
            _ => return Err(SocialError::Server(0, format!("not allowed: {what}"))),
        };
        let mut body = body;
        if what == "messages.send" {
            // Düz metin sunucuya gitmez: burada şifrelenir
            let to = body["to"].as_str().unwrap_or_default().to_string();
            let text = body["body"].as_str().unwrap_or_default().to_string();
            return self.send_message(&to, &text);
        }
        if what == "feedback" {
            body["appVersion"] = json!(env!("CARGO_PKG_VERSION"));
        }
        self.call(method, path, Some(body))
    }

    pub fn privacy(
        &self,
        hide_time: Option<bool>,
        hide_name: Option<bool>,
    ) -> Result<Value, SocialError> {
        self.call(
            "PATCH",
            "me/privacy",
            Some(json!({ "hideTime": hide_time, "hideName": hide_name })),
        )
    }
}

/// Yalnızca kullanıcının okuyabileceği dosya (token)
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)
}

/// Tuzlanmış cihaz özeti (64 hex). Donanım kimliği okunamazsa uygulama klasöründeki rastgele kimlik.
pub fn device_hash(app_data: &Path) -> String {
    let id = hardware_id().unwrap_or_else(|| {
        let p = app_data.join(DEVICE_ID);
        std::fs::read_to_string(&p)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| s.len() >= 32)
            .unwrap_or_else(|| {
                let mut b = [0u8; 32];
                let _ = getrandom(&mut b);
                let s: String = b.iter().map(|x| format!("{x:02x}")).collect();
                let _ = write_private(&p, s.as_bytes());
                s
            })
    });
    let mut h = Sha256::new();
    h.update(SALT.as_bytes());
    h.update(id.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Rastgele bayt: işletim sisteminin kaynağından (Unix /dev/urandom; Windows'ta zaman + süreç karışımı yedeği)
fn getrandom(buf: &mut [u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")?.read_exact(buf)
    }
    #[cfg(not(unix))]
    {
        let mut h = Sha256::new();
        h.update(format!("{:?}{}", std::time::SystemTime::now(), std::process::id()).as_bytes());
        let d = h.finalize();
        for (i, b) in buf.iter_mut().enumerate() {
            *b = d[i % d.len()];
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn hardware_id() -> Option<String> {
    let mut id = [0u8; 16];
    let wait = libc::timespec {
        tv_sec: 5,
        tv_nsec: 0,
    };
    // SAFETY: 16 baytlık tampon ve geçerli bir timespec.
    let r = unsafe { libc::gethostuuid(id.as_mut_ptr(), &wait) };
    (r == 0 && id.iter().any(|&b| b != 0)).then(|| id.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(windows)]
fn hardware_id() -> Option<String> {
    use windows_sys::Win32::System::Registry::{
        HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegGetValueW,
    };
    let wide = |s: &str| {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let key = wide(r"SOFTWARE\Microsoft\Cryptography");
    let name = wide("MachineGuid");
    let mut buf = [0u16; 128];
    let mut len = (buf.len() * 2) as u32;
    // SAFETY: sabit boyutlu tampon ve uzunluğu doğru veriliyor.
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut len,
        )
    };
    if r != 0 {
        return None;
    }
    let n = (len as usize / 2).saturating_sub(1);
    let s = String::from_utf16_lossy(&buf[..n]);
    (!s.trim().is_empty()).then_some(s)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn hardware_id() -> Option<String> {
    None
}

/// Yerel saat dilimi: UTC'ye göre dakika (doğuya artı)
pub fn tz_minutes() -> i32 {
    chrono::Local::now().offset().local_minus_utc() / 60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_hash_is_stable_salted_hex() {
        let tmp = tempfile::tempdir().unwrap();
        let a = device_hash(tmp.path());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(a, device_hash(tmp.path()), "aynı cihazda sabit");
        #[cfg(any(target_os = "macos", windows))]
        {
            let raw = hardware_id().expect("donanım kimliği okunamadı");
            assert!(!a.contains(&raw), "ham kimlik sunucuya gitmez");
        }
    }

    /// Gerçek sunucuyla uçtan uca: `KAANS_TEST_SERVER=http://127.0.0.1:18787` (server/ içinde `PORT=18787 npm run dev`)
    /// verilmezse atlanır.
    #[test]
    fn live_server_round_trip() {
        let Ok(url) = std::env::var("KAANS_TEST_SERVER") else {
            eprintln!("KAANS_TEST_SERVER yok, atlandı");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let s = Social::new(tmp.path(), url.clone());
        let st = match s.register("e2e_test") {
            Ok(st) => st,
            // Aynı sunucuya ikinci koşu: bu makine zaten kayıtlı (sunucuyu boş veritabanıyla başlat)
            Err(SocialError::Server(409, m)) => {
                panic!("cihaz zaten kayıtlı, sunucuyu temiz başlat: {m}")
            }
            Err(e) => panic!("{e}"),
        };
        assert!(st.registered);
        assert_eq!(st.name.as_deref(), Some("e2e_test"));
        // Token diske 0600 yazıldı, yeniden açınca hatırlanır
        let again = Social::new(tmp.path(), url);
        assert!(again.state().registered);
        let v = again
            .heartbeat(true, 180, Some("dracula"))
            .unwrap()
            .unwrap();
        // Canlı bağlantı (WS) açık değilken kredi yok: sunucunun hile önlemi (live.rs testi bağlantılı hali sınar)
        assert_eq!(v["credited"], 0);
        let status = again.get("me/status").unwrap();
        assert_eq!(status["name"], "e2e_test");
        assert_eq!(status["theme"], "dracula");
        let board = again.get("leaderboard?scope=week").unwrap();
        assert!(
            board["rows"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["you"] == true)
        );
        assert!(matches!(
            again.get("nope"),
            Err(SocialError::Server(404, _))
        ));
    }

    #[test]
    fn unreachable_server_is_offline_and_unregistered_sends_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        // Kapalı bir port
        let s = Social::new(tmp.path(), "http://127.0.0.1:9".into());
        assert!(!s.state().registered);
        assert_eq!(s.heartbeat(true, 180, None), Ok(None));
        assert_eq!(s.register("kaan").unwrap_err(), SocialError::Offline);
        assert!(!tmp.path().join(ACCOUNT).exists());
    }

    fn write_account(dir: &Path, server: &str) {
        let acc = Account {
            token: "t0k".into(),
            name: "kaan".into(),
            tag: "5282".into(),
            server: server.into(),
        };
        std::fs::write(dir.join(ACCOUNT), serde_json::to_vec(&acc).unwrap()).unwrap();
    }

    #[test]
    fn account_from_another_server_is_not_used() {
        let tmp = tempfile::tempdir().unwrap();
        write_account(tmp.path(), "http://127.0.0.1:8787");
        let s = Social::new(tmp.path(), "https://example.invalid".into());
        assert!(
            !s.state().registered,
            "yerel sunucunun hesabı gerçek sunucuda kullanılmaz"
        );
        let same = Social::new(tmp.path(), "http://127.0.0.1:8787".into());
        assert!(same.state().registered);
    }

    /// 401 dönen tek seferlik sahte sunucu
    fn unauthorized_server() -> String {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut c, _)) = l.accept() {
                let mut buf = [0u8; 4096];
                let _ = c.read(&mut buf);
                let body = r#"{"error":"unauthorized"}"#;
                let _ = write!(
                    c,
                    "HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn unknown_token_drops_the_account() {
        let tmp = tempfile::tempdir().unwrap();
        // Eski biçim: sunucu alanı yok (boş) → yüklenir, sunucu 401 deyince bırakılır
        write_account(tmp.path(), "");
        let s = Social::new(tmp.path(), unauthorized_server());
        assert!(s.state().registered);
        assert_eq!(s.get("me/status").unwrap_err(), SocialError::Unauthorized);
        assert!(!s.state().registered);
        assert!(!tmp.path().join(ACCOUNT).exists());
        assert!(
            tmp.path().join("account.stale.json").exists(),
            "dosya silinmez, kenara alınır"
        );
    }
}
