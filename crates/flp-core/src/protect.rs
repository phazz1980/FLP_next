//! Password protection of the project copy kept in the board's flash.
//!
//! The firmware only stores and returns the bytes; the editor seals them with a
//! key derived from the project password and opens them after reading the board.
//! Without a password the archive travels as is (it starts with the ZIP magic).
//!
//! Salt and nonce are derived from the content, so sealing is repeatable.
//!
//! Sealed layout: `"CRB1" | salt[16] | nonce[12] | ChaCha20-Poly1305(archive)`.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

use crate::error::{Error, Result};

const MAGIC: &[u8; 4] = b"CRB1";
const SALT: usize = 16;
const NONCE: usize = 12;
const HEADER: usize = 4 + SALT + NONCE;
const ZIP_MAGIC: &[u8; 2] = b"PK";
/// Slows down guessing a password from a dump of the flash.
const ROUNDS: u32 = 100_000;

fn key(password: &str, salt: &[u8]) -> Key {
    let mut key = Key::default();
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt, ROUNDS, &mut key);
    key
}

/// Salt and nonce from the content itself, so the same project and password give the
/// same bytes (the sketch does not change on every generation). Another archive gets
/// another salt, hence another key: a nonce is never reused with one key.
fn derive(archive: &[u8], password: &str) -> [u8; SALT + NONCE] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"crablogic-seal");
    hash.update((password.len() as u64).to_le_bytes());
    hash.update(password.as_bytes());
    hash.update(archive);
    let mut out = [0u8; SALT + NONCE];
    out.copy_from_slice(&hash.finalize()[..SALT + NONCE]);
    out
}

/// Archive as it goes to the board: encrypted when `password` is not empty.
pub fn seal(archive: &[u8], password: &str) -> Result<Vec<u8>> {
    if password.is_empty() {
        return Ok(archive.to_vec());
    }
    let random = derive(archive, password);
    let (salt, nonce) = random.split_at(SALT);
    let cipher = ChaCha20Poly1305::new(&key(password, salt));
    let body = cipher
        .encrypt(Nonce::from_slice(nonce), archive)
        .map_err(|_| Error::Crypto("не удалось зашифровать проект".into()))?;
    let mut out = Vec::with_capacity(HEADER + body.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&random);
    out.extend_from_slice(&body);
    Ok(out)
}

/// What the debug gate in the firmware keeps to check a password: the salt (sent to the
/// host with every challenge) and the key derived from the password with it.
///
/// The key is derived with a salt of its own, so it differs from the one that seals the
/// project copy: a dump of the flash lets one try passwords, but does not open the copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthMaterial {
    pub salt: [u8; SALT],
    pub key: [u8; 32],
}

/// Debug access material for `archive`; none when `password` is empty (no gate).
pub fn auth_material(archive: &[u8], password: &str) -> Option<AuthMaterial> {
    use sha2::{Digest, Sha256};
    if password.is_empty() {
        return None;
    }
    let mut hash = Sha256::new();
    hash.update(b"crablogic-auth-salt");
    hash.update(derive(archive, password));
    let mut salt = [0u8; SALT];
    salt.copy_from_slice(&hash.finalize()[..SALT]);
    Some(AuthMaterial {
        key: auth_key(password, &salt),
        salt,
    })
}

/// The access key for `password` and the `salt` the board announced.
pub fn auth_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt, ROUNDS, &mut key);
    key
}

/// The answer to a challenge: HMAC-SHA256 of the board's `nonce` under `key`.
pub fn auth_proof(key: &[u8; 32], nonce: &[u8]) -> [u8; 32] {
    use hmac::{Hmac, Mac as _};
    let mut mac = <Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(key).expect("any key length");
    mac.update(nonce);
    mac.finalize().into_bytes().into()
}

/// True when `data` is a sealed archive and needs a password.
pub fn is_sealed(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}

/// Archive from the bytes read from the board. A plain archive is returned as is;
/// a sealed one needs the right `password`.
pub fn open(data: &[u8], password: &str) -> Result<Vec<u8>> {
    if data.starts_with(ZIP_MAGIC) {
        return Ok(data.to_vec());
    }
    if !is_sealed(data) || data.len() < HEADER {
        return Err(Error::Crypto("в плате нет проекта CrabLogic".into()));
    }
    if password.is_empty() {
        return Err(Error::Crypto("проект на плате защищён паролем".into()));
    }
    let salt = &data[4..4 + SALT];
    let nonce = &data[4 + SALT..HEADER];
    ChaCha20Poly1305::new(&key(password, salt))
        .decrypt(Nonce::from_slice(nonce), &data[HEADER..])
        .map_err(|_| Error::Crypto("неверный пароль или повреждённые данные".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARCHIVE: &[u8] = b"PK\x03\x04[project]\nname = \"pump\"\n";

    #[test]
    fn no_password_keeps_the_archive() {
        let sealed = seal(ARCHIVE, "").unwrap();
        assert_eq!(sealed, ARCHIVE);
        assert!(!is_sealed(&sealed));
        assert_eq!(open(&sealed, "").unwrap(), ARCHIVE);
    }

    #[test]
    fn right_password_round_trip() {
        let sealed = seal(ARCHIVE, "секрет").unwrap();
        assert!(is_sealed(&sealed));
        let text = String::from_utf8_lossy(&sealed);
        assert!(!text.contains("pump"), "plain text leaks into the dump");
        assert_eq!(open(&sealed, "секрет").unwrap(), ARCHIVE);
    }

    #[test]
    fn wrong_or_missing_password_is_an_error() {
        let sealed = seal(ARCHIVE, "секрет").unwrap();
        let wrong = open(&sealed, "другой").unwrap_err().to_string();
        assert!(wrong.contains("неверный пароль"), "{wrong}");
        let missing = open(&sealed, "").unwrap_err().to_string();
        assert!(missing.contains("защищён паролем"), "{missing}");
    }

    #[test]
    fn tampered_data_is_rejected() {
        let mut sealed = seal(ARCHIVE, "секрет").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert!(open(&sealed, "секрет").is_err());
    }

    #[test]
    fn sealing_is_repeatable_per_content() {
        assert_eq!(seal(ARCHIVE, "a").unwrap(), seal(ARCHIVE, "a").unwrap());
        assert_ne!(seal(ARCHIVE, "a").unwrap(), seal(ARCHIVE, "b").unwrap());
        assert_ne!(seal(ARCHIVE, "a").unwrap(), seal(b"PK other", "a").unwrap());
    }

    #[test]
    fn auth_proof_needs_the_same_password() {
        let m = auth_material(ARCHIVE, "секрет").unwrap();
        let nonce = [7u8; 16];
        let good = auth_proof(&auth_key("секрет", &m.salt), &nonce);
        assert_eq!(good, auth_proof(&m.key, &nonce));
        assert_ne!(good, auth_proof(&auth_key("другой", &m.salt), &nonce));
        assert_ne!(good, auth_proof(&m.key, &[8u8; 16]));
    }

    #[test]
    fn no_password_no_gate_and_keys_differ_from_sealing() {
        assert!(auth_material(ARCHIVE, "").is_none());
        let m = auth_material(ARCHIVE, "a").unwrap();
        let sealed = seal(ARCHIVE, "a").unwrap();
        assert_ne!(&m.salt[..], &sealed[4..4 + SALT]);
        assert_eq!(auth_material(ARCHIVE, "a").unwrap(), m);
    }

    #[test]
    fn foreign_data_is_reported() {
        assert!(open(b"hello", "x").is_err());
    }
}
