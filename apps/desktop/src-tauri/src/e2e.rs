//! Mesajlarda uçtan uca şifreleme (OZELLIKLER.md §6.2, MIMARI.md §9). Her cihazın bir X25519 anahtar çifti vardır;
//! gizli anahtar yalnızca bu cihazda (`<app_data>/keys.json`, 0600), sunucuya yalnızca açık anahtar gider.
//! Mesajlar NaCl box (X25519 + XSalsa20-Poly1305) ile şifrelenir: ortak anahtar = DH(benim gizli, onun açık), bu
//! yüzden gönderen de kendi mesajını (alıcının açık anahtarıyla) çözebilir. Web sayfası ne anahtarı ne şifreli
//! metni görür.
use crypto_box::aead::{Aead, AeadCore, OsRng};
use crypto_box::{PublicKey, SalsaBox, SecretKey};
use serde::{Deserialize, Serialize};
use std::path::Path;

const FILE: &str = "keys.json";

#[derive(Serialize, Deserialize)]
struct Stored {
    secret: String,
}

pub struct Keys {
    secret: SecretKey,
    public_hex: String,
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

fn key32(hex: &str) -> Option<[u8; 32]> {
    from_hex(hex)?.try_into().ok()
}

impl Keys {
    /// Diskteki anahtarı yükler; yoksa (ya da bozuksa) yenisini üretip 0600 kaydeder.
    pub fn load_or_create(dir: &Path) -> Self {
        let path = dir.join(FILE);
        let secret = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Stored>(&b).ok())
            .and_then(|s| key32(&s.secret))
            .map(SecretKey::from)
            .unwrap_or_else(|| {
                let sk = SecretKey::generate(&mut OsRng);
                let stored = Stored {
                    secret: to_hex(&sk.to_bytes()),
                };
                if let Ok(b) = serde_json::to_vec(&stored) {
                    let _ = crate::social::write_private(&path, &b);
                }
                sk
            });
        let public_hex = to_hex(secret.public_key().as_bytes());
        Self { secret, public_hex }
    }

    pub fn public_hex(&self) -> &str {
        &self.public_hex
    }

    fn salsa(&self, their_public_hex: &str) -> Option<SalsaBox> {
        Some(SalsaBox::new(
            &PublicKey::from(key32(their_public_hex)?),
            &self.secret,
        ))
    }

    /// (şifreli metin hex, nonce hex)
    pub fn encrypt(&self, their_public_hex: &str, plaintext: &str) -> Option<(String, String)> {
        let b = self.salsa(their_public_hex)?;
        let nonce = SalsaBox::generate_nonce(&mut OsRng);
        let ct = b.encrypt(&nonce, plaintext.as_bytes()).ok()?;
        Some((to_hex(&ct), to_hex(&nonce)))
    }

    /// Mesajı çözer. `mine`: ben gönderdiysem karşı tarafın anahtarı `to_key`, değilse `from_key`. Kendi anahtarım
    /// o mesajdaki ile aynı değilse (yeniden kurulum) çözülemez → None.
    pub fn decrypt(
        &self,
        mine: bool,
        from_key: &str,
        to_key: &str,
        nonce_hex: &str,
        ct_hex: &str,
    ) -> Option<String> {
        let (me, other) = if mine {
            (from_key, to_key)
        } else {
            (to_key, from_key)
        };
        if me != self.public_hex {
            return None;
        }
        let nonce: [u8; 24] = from_hex(nonce_hex)?.try_into().ok()?;
        let pt = self
            .salsa(other)?
            .decrypt(&nonce.into(), from_hex(ct_hex)?.as_slice())
            .ok()?;
        String::from_utf8(pt).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sides_can_read_and_nobody_else() {
        let (da, db, de) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        let alice = Keys::load_or_create(da.path());
        let bob = Keys::load_or_create(db.path());
        let eve = Keys::load_or_create(de.path());
        let (ct, nonce) = alice
            .encrypt(bob.public_hex(), "selam 👋 build geçti")
            .unwrap();
        assert!(
            !ct.contains(&to_hex("selam".as_bytes())),
            "düz metin görünmez"
        );
        let (a, b) = (alice.public_hex(), bob.public_hex());
        assert_eq!(
            bob.decrypt(false, a, b, &nonce, &ct).as_deref(),
            Some("selam 👋 build geçti")
        );
        assert_eq!(
            alice.decrypt(true, a, b, &nonce, &ct).as_deref(),
            Some("selam 👋 build geçti"),
            "gönderen de okur"
        );
        // üçüncü kişi (kendi anahtarı bu mesajda yok) ve bozulmuş metin
        assert_eq!(eve.decrypt(false, a, eve.public_hex(), &nonce, &ct), None);
        let mut bad = ct.clone();
        bad.replace_range(0..2, if &ct[0..2] == "00" { "01" } else { "00" });
        assert_eq!(bob.decrypt(false, a, b, &nonce, &bad), None);
        // anahtar diskte kalıcı; gizli anahtar dosyası yalnızca kullanıcıya açık
        assert_eq!(Keys::load_or_create(da.path()).public_hex(), a);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(da.path().join(FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
