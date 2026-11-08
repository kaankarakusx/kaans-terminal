//! Canlı oturum kayıtları: `$CLAUDE_DIR/sessions/<pid>.json` (MIMARI.md §3.1).

use super::paths::is_registry_file;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub pid: u32,
    pub session_id: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub spare: Option<bool>,
    #[serde(default)]
    pub host_session_id: Option<String>,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub waiting_for: Option<Value>,
    #[serde(default)]
    pub status_updated_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub started_at: Option<i64>,
    /// Sürecin başlangıcı (UTC, `"Sun Nov  8 10:24:45 2026"`): pid yeniden kullanımını ayırt etmek için.
    #[serde(default)]
    pub proc_start: Option<String>,
}

impl SessionRecord {
    /// Yalnızca kullanıcının sohbetleri: `kind == "interactive"`, `spare != true`, `hostSessionId` boş.
    pub fn is_user_chat(&self) -> bool {
        self.kind.as_deref().unwrap_or("interactive") == "interactive"
            && self.spare != Some(true)
            && self.host_session_id.as_deref().is_none_or(str::is_empty)
    }

    /// `waitingFor` alanını kısa bir metne çevirir (biçimi sürüme göre değişebilir).
    pub fn waiting_for_text(&self) -> Option<String> {
        match self.waiting_for.as_ref()? {
            Value::Null => None,
            Value::String(s) if s.is_empty() => None,
            Value::String(s) => Some(s.clone()),
            Value::Object(o) => ["tool", "type", "kind", "reason"]
                .iter()
                .find_map(|k| o.get(*k).and_then(Value::as_str).map(str::to_string))
                .or_else(|| Some("input".into())),
            other => Some(other.to_string()),
        }
    }
}

/// `sessions/` klasörünü tarar; `mtime` değişmeyen dosyayı yeniden okumaz.
pub struct Registry {
    dir: PathBuf,
    cache: HashMap<PathBuf, (SystemTime, SessionRecord)>,
}

impl Registry {
    pub fn new(claude_dir: &Path) -> Self {
        Self {
            dir: claude_dir.join("sessions"),
            cache: HashMap::new(),
        }
    }

    /// Geçerli kayıtlar. Aynı `sessionId`'yi tutan iki kayıt varsa `updatedAt` yeni olan kalır.
    pub fn scan(&mut self) -> Vec<SessionRecord> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            self.cache.clear();
            return Vec::new();
        };
        let mut present = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            // `.key` dosyaları burada elenir ve hiçbir zaman açılmaz.
            if !name.to_str().is_some_and(is_registry_file) {
                continue;
            }
            let path = entry.path();
            let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            present.push(path.clone());
            if self.cache.get(&path).is_some_and(|(t, _)| *t == mtime) {
                continue;
            }
            // Yazılırken okunan yarım dosya parse edilemez; eski değer kalır, sonraki turda tekrar denenir.
            if let Some(rec) = fs::read(&path)
                .ok()
                .and_then(|b| serde_json::from_slice::<SessionRecord>(&b).ok())
            {
                self.cache.insert(path, (mtime, rec));
            }
        }
        self.cache.retain(|p, _| present.contains(p));

        let mut by_session: HashMap<&str, &SessionRecord> = HashMap::new();
        for (_, rec) in self.cache.values() {
            by_session
                .entry(rec.session_id.as_str())
                .and_modify(|cur| {
                    if rec.updated_at > cur.updated_at {
                        *cur = rec;
                    }
                })
                .or_insert(rec);
        }
        by_session.into_values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write(dir: &Path, name: &str, v: Value) {
        fs::write(dir.join(name), serde_json::to_vec(&v).unwrap()).unwrap();
    }

    #[test]
    fn scans_records_ignores_key_files_and_dedupes() {
        let tmp = tempfile::tempdir().unwrap();
        let sessions = tmp.path().join("sessions");
        fs::create_dir(&sessions).unwrap();
        write(
            &sessions,
            "100.json",
            json!({"pid":100,"sessionId":"a","status":"busy","updatedAt":5}),
        );
        write(
            &sessions,
            "200.json",
            json!({"pid":200,"sessionId":"a","status":"idle","updatedAt":9}),
        );
        write(
            &sessions,
            "300.json",
            json!({"pid":300,"sessionId":"b","kind":"interactive"}),
        );
        fs::write(sessions.join("100.deadbeef.key"), b"SECRET-NOT-JSON").unwrap();
        fs::write(sessions.join("400.json"), b"{ yarim").unwrap();

        let mut reg = Registry::new(tmp.path());
        let mut recs = reg.scan();
        recs.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].pid, 200, "updatedAt yeni olan kazanır");
        assert_eq!(recs[1].session_id, "b");

        fs::remove_file(sessions.join("300.json")).unwrap();
        assert_eq!(reg.scan().len(), 1);
    }

    #[test]
    fn user_chat_filter() {
        let r = |v: Value| serde_json::from_value::<SessionRecord>(v).unwrap();
        assert!(r(json!({"pid":1,"sessionId":"s"})).is_user_chat());
        assert!(r(json!({"pid":1,"sessionId":"s","hostSessionId":""})).is_user_chat());
        assert!(!r(json!({"pid":1,"sessionId":"s","kind":"sdk"})).is_user_chat());
        assert!(!r(json!({"pid":1,"sessionId":"s","spare":true})).is_user_chat());
        assert!(!r(json!({"pid":1,"sessionId":"s","hostSessionId":"h"})).is_user_chat());
    }
}
