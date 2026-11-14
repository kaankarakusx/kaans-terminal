//! Kayıtlı sunucular (Termius gibi): tek tıkla `ssh` terminali.
//!
//! - Liste `<app_data>/servers.json` (0600). Parola ve anahtar içeriği SAKLANMAZ: kimlik doğrulamayı kullanıcının
//!   `ssh`'ı yapar (ssh-agent, Anahtar Zinciri, `~/.ssh/config`); parola gerekiyorsa terminalde sorar.
//! - `~/.ssh/config` yalnızca okunur: `Host` takma adları içe aktarma önerisi olarak gösterilir.
//! - Bağlantı = uygulamanın normal terminal oturumu, komutu `ssh [-p port] [-i anahtar] -- [kullanıcı@]host`.
//!   Argümanlar kabuktan geçmez; host/kullanıcı `-` ile başlayamaz (ssh seçeneği sayılmasın).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const FILE: &str = "servers.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    /// Boşsa kaydederken üretilir
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// Adres ya da `~/.ssh/config` takma adı
    pub host: String,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    /// Özel anahtar dosyasının yolu (içeriği okunmaz)
    #[serde(default)]
    pub identity_file: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    /// Son bağlanma (ms, Unix)
    #[serde(default)]
    pub last_used: Option<u64>,
}

/// `~/.ssh/config` içindeki bir `Host` girdisi (joker karaktersiz).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshHost {
    pub alias: String,
    pub host_name: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    /// ProxyJump/ProxyCommand var: doğrudan TCP ölçümü anlamsız
    #[serde(default)]
    pub proxy: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServersState {
    pub servers: Vec<Server>,
    pub ssh_config: Vec<SshHost>,
}

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(FILE))
}

fn load(path: &Path) -> Vec<Server> {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn store(path: &Path, list: &[Server]) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?;
    crate::social::write_private(path, &bytes).map_err(|e| e.to_string())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn home() -> Option<PathBuf> {
    std::env::home_dir()
}

fn expand_tilde(p: &str) -> String {
    match (p.strip_prefix("~/"), home()) {
        (Some(rest), Some(h)) => h.join(rest).to_string_lossy().into_owned(),
        _ => p.to_string(),
    }
}

/// Host ve kullanıcı adı: boşluk/denetim karakteri yok, `-` ile başlamaz.
fn valid_token(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && s.len() <= 255
        && s.chars().all(|c| !c.is_whitespace() && !c.is_control())
}

/// Kaydetmeden önce temizler ve denetler.
fn normalize(mut s: Server) -> Result<Server, String> {
    s.name = s.name.trim().to_string();
    s.host = s.host.trim().to_string();
    let opt = |v: Option<String>| v.map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
    s.user = opt(s.user);
    s.identity_file = opt(s.identity_file);
    s.group = opt(s.group);
    // "kullanıcı@host" tek alana yazıldıysa ayır
    if s.user.is_none()
        && let Some((u, h)) = s.host.split_once('@')
    {
        s.user = Some(u.to_string());
        s.host = h.to_string();
    }
    if !valid_token(&s.host) || s.host.contains('@') {
        return Err("invalid host".into());
    }
    if let Some(u) = &s.user
        && (!valid_token(u) || u.contains('@'))
    {
        return Err("invalid user".into());
    }
    if s.port == Some(0) {
        return Err("invalid port".into());
    }
    if let Some(k) = &s.identity_file
        && (k.starts_with('-') || k.chars().any(|c| c.is_control()))
    {
        return Err("invalid key path".into());
    }
    if s.name.is_empty() {
        s.name = s.host.clone();
    }
    s.name = s.name.chars().take(80).collect();
    if s.id.is_empty() {
        s.id = format!("srv-{:x}", now_ms());
    }
    Ok(s)
}

/// `ssh` argümanları (kabuk yok).
pub fn ssh_argv(s: &Server) -> Vec<String> {
    let mut argv = vec!["ssh".to_string()];
    if let Some(p) = s.port {
        argv.extend(["-p".into(), p.to_string()]);
    }
    if let Some(k) = &s.identity_file {
        argv.extend(["-i".into(), expand_tilde(k)]);
    }
    argv.push("--".into());
    argv.push(match &s.user {
        Some(u) => format!("{u}@{}", s.host),
        None => s.host.clone(),
    });
    argv
}

/// `~/.ssh/config`'ten joker içermeyen `Host` girdileri. `Include` izlenmez.
pub fn parse_ssh_config(text: &str) -> Vec<SshHost> {
    let mut out: Vec<SshHost> = vec![];
    // Aynı `Host` satırındaki bütün takma adlar o bloğun ayarlarını paylaşır
    let mut block: Vec<usize> = vec![];
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, val) = match line.split_once(|c: char| c.is_whitespace() || c == '=') {
            Some((k, v)) => (
                k.to_ascii_lowercase(),
                v.trim_start_matches(['=', ' ', '\t']).trim(),
            ),
            None => continue,
        };
        let val = val.trim_matches('"');
        match key.as_str() {
            "host" => {
                block.clear();
                for alias in val.split_whitespace() {
                    if alias.contains(['*', '?', '!']) || out.iter().any(|h| h.alias == alias) {
                        continue;
                    }
                    block.push(out.len());
                    out.push(SshHost {
                        alias: alias.to_string(),
                        host_name: None,
                        user: None,
                        port: None,
                        proxy: false,
                    });
                }
            }
            "match" => block.clear(),
            // ssh'ta ilk değer geçerlidir
            "hostname" => block.iter().for_each(|&i| {
                out[i].host_name.get_or_insert_with(|| val.to_string());
            }),
            "user" => block.iter().for_each(|&i| {
                out[i].user.get_or_insert_with(|| val.to_string());
            }),
            "port" => {
                if let Ok(p) = val.parse() {
                    block.iter().for_each(|&i| {
                        out[i].port.get_or_insert(p);
                    });
                }
            }
            "proxyjump" | "proxycommand" if !val.eq_ignore_ascii_case("none") => {
                block.iter().for_each(|&i| out[i].proxy = true)
            }
            _ => {}
        }
    }
    out
}

/// TCP ile ulaşılacak adres: ssh config takma adıysa onun HostName/Port'u. Vekil (ProxyJump) varsa None.
fn probe_target(s: &Server, config: &[SshHost]) -> Option<(String, u16)> {
    match config.iter().find(|h| h.alias == s.host) {
        Some(h) if h.proxy => None,
        Some(h) => Some((
            h.host_name.clone().unwrap_or_else(|| s.host.clone()),
            s.port.or(h.port).unwrap_or(22),
        )),
        None => Some((s.host.clone(), s.port.unwrap_or(22))),
    }
}

/// Ulaşılabilirlik: TCP bağlantısı kurulana kadar geçen süre (ms). Ulaşılamazsa null.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub id: String,
    /// "up" | "down" | "unknown" (vekil arkasında, ölçülemez)
    pub state: &'static str,
    pub ms: Option<u32>,
}

fn probe_one(host: &str, port: u16) -> Option<u32> {
    use std::net::{TcpStream, ToSocketAddrs};
    let timeout = std::time::Duration::from_millis(2500);
    let addrs = (host, port).to_socket_addrs().ok()?;
    for a in addrs.take(3) {
        let t = std::time::Instant::now();
        if TcpStream::connect_timeout(&a, timeout).is_ok() {
            return Some(t.elapsed().as_millis().min(u32::MAX as u128) as u32);
        }
    }
    None
}

/// Kayıtlı sunucuları paralel yoklar (her biri en çok ~3 sn; DNS takılırsa "down" sayılır).
#[tauri::command]
pub async fn servers_probe(app: AppHandle) -> Result<Vec<Probe>, String> {
    let path = store_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let config = read_ssh_config();
        let (tx, rx) = std::sync::mpsc::channel();
        let list = load(&path);
        for s in &list {
            let tx = tx.clone();
            let id = s.id.clone();
            match probe_target(s, &config) {
                None => {
                    let _ = tx.send(Probe {
                        id,
                        state: "unknown",
                        ms: None,
                    });
                }
                Some((host, port)) => {
                    std::thread::spawn(move || {
                        let ms = probe_one(&host, port);
                        let state = if ms.is_some() { "up" } else { "down" };
                        let _ = tx.send(Probe { id, state, ms });
                    });
                }
            }
        }
        drop(tx);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        let mut out = vec![];
        while out.len() < list.len() {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match rx.recv_timeout(left) {
                Ok(p) => out.push(p),
                Err(_) => break,
            }
        }
        // Süresi dolanlar: ulaşılamadı
        for s in &list {
            if !out.iter().any(|p| p.id == s.id) {
                out.push(Probe {
                    id: s.id.clone(),
                    state: "down",
                    ms: None,
                });
            }
        }
        out
    })
    .await
    .map_err(|e| e.to_string())
}

fn read_ssh_config() -> Vec<SshHost> {
    home()
        .map(|h| h.join(".ssh").join("config"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse_ssh_config(&t))
        .unwrap_or_default()
}

#[tauri::command]
pub async fn servers_list(app: AppHandle) -> Result<ServersState, String> {
    let path = store_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || ServersState {
        servers: load(&path),
        ssh_config: read_ssh_config(),
    })
    .await
    .map_err(|e| e.to_string())
}

/// Ekler ya da (aynı kimlik) günceller; güncel listeyi döner.
#[tauri::command]
pub fn servers_save(app: AppHandle, server: Server) -> Result<Vec<Server>, String> {
    let path = store_path(&app)?;
    let s = normalize(server)?;
    let mut list = load(&path);
    match list.iter_mut().find(|x| x.id == s.id) {
        Some(x) => {
            let last = x.last_used;
            *x = Server {
                last_used: s.last_used.or(last),
                ..s
            };
        }
        None => list.push(s),
    }
    store(&path, &list)?;
    Ok(list)
}

#[tauri::command]
pub fn servers_delete(app: AppHandle, id: String) -> Result<Vec<Server>, String> {
    let path = store_path(&app)?;
    let mut list = load(&path);
    list.retain(|x| x.id != id);
    store(&path, &list)?;
    Ok(list)
}

/// Sunucuya bağlanan yeni terminal oturumu açar; oturum kimliğini döner (pencereyi sayfa açar).
#[tauri::command]
pub async fn server_connect(
    app: AppHandle,
    t: tauri::State<'_, crate::term::Terminals>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<String, String> {
    let path = store_path(&app)?;
    let mut list = load(&path);
    let s = list
        .iter_mut()
        .find(|x| x.id == id)
        .ok_or("server not found")?;
    s.last_used = Some(now_ms());
    let argv = ssh_argv(s);
    store(&path, &list)?;
    crate::term::term_create(app, t, None, Some(argv), cols, rows).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srv(host: &str) -> Server {
        Server {
            name: String::new(),
            host: host.into(),
            ..Default::default()
        }
    }

    #[test]
    fn argv_is_plain_and_ends_options() {
        let s = normalize(Server {
            port: Some(2222),
            identity_file: Some("/k/id_ed25519".into()),
            ..srv("deploy@example.com")
        })
        .unwrap();
        assert_eq!(s.user.as_deref(), Some("deploy"));
        assert_eq!(s.name, "example.com");
        assert_eq!(
            ssh_argv(&s),
            [
                "ssh",
                "-p",
                "2222",
                "-i",
                "/k/id_ed25519",
                "--",
                "deploy@example.com"
            ]
        );
        assert_eq!(
            ssh_argv(&normalize(srv("box")).unwrap()),
            ["ssh", "--", "box"]
        );
    }

    #[test]
    fn rejects_option_like_and_spaced_values() {
        for h in ["", "-oProxyCommand=x", "a b", "a@b@c", "x\ny"] {
            assert!(normalize(srv(h)).is_err(), "{h:?}");
        }
        assert!(
            normalize(Server {
                user: Some("-x".into()),
                ..srv("h")
            })
            .is_err()
        );
        assert!(
            normalize(Server {
                port: Some(0),
                ..srv("h")
            })
            .is_err()
        );
        assert!(
            normalize(Server {
                identity_file: Some("-F/x".into()),
                ..srv("h")
            })
            .is_err()
        );
    }

    #[test]
    fn parses_ssh_config_hosts() {
        let cfg = "\
# yorum
Host *
  User everyone
Host web web-alias
  HostName 10.0.0.5
  User deploy
  Port 2201
Host db
  HostName=db.internal
  Port bad
Host !nope *.corp
Match host x
  User ignored
Host web
  User second
";
        let h = parse_ssh_config(cfg);
        assert_eq!(h.len(), 3);
        assert_eq!(
            h[0],
            SshHost {
                alias: "web".into(),
                host_name: Some("10.0.0.5".into()),
                user: Some("deploy".into()),
                port: Some(2201),
                proxy: false,
            }
        );
        assert_eq!(h[1].alias, "web-alias");
        assert_eq!(h[1].port, Some(2201));
        assert_eq!(h[2].host_name.as_deref(), Some("db.internal"));
        assert_eq!(h[2].port, None);
        assert_eq!(h[2].user, None);
    }

    #[test]
    fn probe_targets_follow_ssh_config() {
        let cfg = parse_ssh_config(
            "Host web\n HostName 10.0.0.5\n Port 2201\nHost inner\n ProxyJump web\nHost direct\n ProxyCommand none\n",
        );
        assert!(cfg[1].proxy && !cfg[2].proxy);
        let t = |s: Server| probe_target(&s, &cfg);
        assert_eq!(t(srv("web")), Some(("10.0.0.5".into(), 2201)));
        assert_eq!(
            t(Server {
                port: Some(9),
                ..srv("web")
            }),
            Some(("10.0.0.5".into(), 9))
        );
        assert_eq!(t(srv("inner")), None);
        assert_eq!(t(srv("example.com")), Some(("example.com".into(), 22)));
    }

    #[test]
    fn probe_measures_local_listener() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        assert!(probe_one("127.0.0.1", port).is_some());
        drop(l);
        assert!(probe_one("127.0.0.1", port).is_none());
    }

    #[test]
    fn store_round_trip_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(FILE);
        assert!(load(&p).is_empty());
        let s = normalize(srv("h")).unwrap();
        store(&p, std::slice::from_ref(&s)).unwrap();
        assert_eq!(load(&p), vec![s]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
