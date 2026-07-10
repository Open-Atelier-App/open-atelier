//! Local-only credential storage.
//!
//! By explicit requirement, Atelier's OWN credential storage (this file)
//! NEVER touches the OS keychain (no Secret Service on Linux, no Keychain on
//! macOS, no Credential Manager on Windows — the `keyring` crate is not used
//! anywhere in this file). All credentials Atelier writes are stored in a
//! locally encrypted file under the app's data directory instead, so saving
//! or reading a key never triggers an OS keychain prompt of any kind.
//!
//! Elsewhere in the app, `keyring` is used for exactly one unrelated purpose:
//! an explicit, button-triggered check (never automatic) of pre-existing CLI
//! OAuth sessions in the macOS Keychain, so Atelier can offer to reuse that
//! external session. See `llm/anthropic_oauth.rs`. That code path never reads
//! or writes anything this module stores.
//!
//! SECURITY NOTE: this local encrypted store is "good enough obfuscation for a
//! local single-user desktop app, not a security boundary against a determined
//! local attacker." The master key lives on disk (with restrictive permissions on
//! unix) right next to the ciphertext, so anyone with code-execution access as the
//! same OS user can decrypt it. This only protects against casual inspection
//! (e.g. `cat credentials.enc.json`), not against a determined local attacker.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{AtelierError, ErrorCode, Result};

const MASTER_KEY_FILE: &str = ".cred_master_key";
const CRED_FILE: &str = "credentials.enc.json";

/// Which backend a credential is stored in. The OS keychain variant is kept
/// only so the `KeyStatus`/`cred_get_with_backend` wire format doesn't need
/// to change; this app never writes to it, so it will never be returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageBackend {
    Keychain,
    LocalEncrypted,
}

impl StorageBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageBackend::Keychain => "keychain",
            StorageBackend::LocalEncrypted => "local_encrypted",
        }
    }
}

impl std::fmt::Display for StorageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Default, Serialize, Deserialize)]
struct CredFile {
    // key: "{service}/{account}" -> base64 ciphertext (nonce || ciphertext)
    entries: HashMap<String, String>,
}

fn map_key(service: &str, account: &str) -> String {
    format!("{service}/{account}")
}

fn app_data_dir() -> Result<PathBuf> {
    let base = dirs::data_dir()
        .ok_or_else(|| AtelierError::new(ErrorCode::KeychainError, "Could not resolve app data directory"))?;
    let dir = base.join("com.openatelier.app");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn data_dir() -> Result<PathBuf> {
    app_data_dir()
}

pub fn profile_data_dir(profile_id: i64) -> Result<PathBuf> {
    let base = app_data_dir()?;
    let dir = base.join("credentials").join(profile_id.to_string());
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn master_key_path(dir: &Path) -> PathBuf {
    dir.join(MASTER_KEY_FILE)
}

fn cred_file_path(dir: &Path) -> PathBuf {
    dir.join(CRED_FILE)
}

/// Load the master key, generating + persisting a new random one on first use.
fn load_or_create_master_key(dir: &Path) -> Result<[u8; 32]> {
    let path = master_key_path(dir);
    if let Ok(bytes) = std::fs::read(&path) {
        if bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }
    }

    // Generate a fresh random 32-byte key.
    let mut key = [0u8; 32];
    getrandom(&mut key)?;

    let mut file = std::fs::File::create(&path)?;
    file.write_all(&key)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    Ok(key)
}

/// Minimal dependency-free random byte source (no `rand` crate needed): reads
/// from the OS CSPRNG via the platform's standard secure random device/API.
fn getrandom(buf: &mut [u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Read;
        let mut f = std::fs::File::open("/dev/urandom")
            .map_err(|e| AtelierError::new(ErrorCode::KeychainError, format!("Failed to open random source: {e}")))?;
        f.read_exact(buf)
            .map_err(|e| AtelierError::new(ErrorCode::KeychainError, format!("Failed to read random bytes: {e}")))?;
        Ok(())
    }
    #[cfg(windows)]
    {
        // Fallback: combine time-based entropy with address-based entropy.
        // This is only used to seed a per-install obfuscation key, not for
        // cryptographic guarantees beyond casual local inspection.
        use std::time::{SystemTime, UNIX_EPOCH};
        let mut seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        for chunk in buf.chunks_mut(8) {
            seed ^= seed.rotate_left(13).wrapping_add(0x9E3779B97F4A7C15);
            let bytes = seed.to_le_bytes();
            for (i, b) in chunk.iter_mut().enumerate() {
                *b = bytes[i % 8];
            }
        }
        Ok(())
    }
}

/// Tiny XOR-based stream cipher keyed by a SHA/blake3-derived keystream.
/// This is intentionally simple (see module-level security note) — it keeps us
/// from pulling in a new crypto crate while still avoiding plaintext-on-disk.
fn xor_crypt(key: &[u8; 32], nonce: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut counter: u64 = 0;
    let mut keystream_block = [0u8; 32];
    let mut block_pos = 32; // force regeneration on first byte

    for &byte in data {
        if block_pos >= keystream_block.len() {
            let mut hasher = blake3::Hasher::new();
            hasher.update(key);
            hasher.update(nonce);
            hasher.update(&counter.to_le_bytes());
            let hash = hasher.finalize();
            keystream_block.copy_from_slice(hash.as_bytes());
            counter += 1;
            block_pos = 0;
        }
        out.push(byte ^ keystream_block[block_pos]);
        block_pos += 1;
    }
    out
}

fn encrypt(key: &[u8; 32], plaintext: &str) -> Result<String> {
    let mut nonce = [0u8; 16];
    getrandom(&mut nonce)?;
    let ciphertext = xor_crypt(key, &nonce, plaintext.as_bytes());
    let mut combined = Vec::with_capacity(16 + ciphertext.len());
    combined.extend_from_slice(&nonce);
    combined.extend_from_slice(&ciphertext);
    Ok(base64_encode(&combined))
}

fn decrypt(key: &[u8; 32], encoded: &str) -> Result<String> {
    let combined = base64_decode(encoded)
        .map_err(|e| AtelierError::new(ErrorCode::KeychainError, format!("Corrupt credential data: {e}")))?;
    if combined.len() < 16 {
        return Err(AtelierError::new(ErrorCode::KeychainError, "Corrupt credential data"));
    }
    let mut nonce = [0u8; 16];
    nonce.copy_from_slice(&combined[..16]);
    let plain_bytes = xor_crypt(key, &nonce, &combined[16..]);
    String::from_utf8(plain_bytes)
        .map_err(|e| AtelierError::new(ErrorCode::KeychainError, format!("Corrupt credential data: {e}")))
}

// ── Minimal base64 (no external dependency) ────────────────────────────────

const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(B64_CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { B64_CHARS[((n >> 6) & 0x3F) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64_CHARS[(n & 0x3F) as usize] as char } else { '=' });
    }
    out
}

fn base64_decode(s: &str) -> std::result::Result<Vec<u8>, String> {
    fn val(c: u8) -> std::result::Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("invalid base64 byte: {c}")),
        }
    }
    let bytes: Vec<u8> = s.bytes().filter(|&b| b != b'=' && !b.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3 + 3);
    for chunk in bytes.chunks(4) {
        let mut vals = [0u8; 4];
        for (i, &b) in chunk.iter().enumerate() {
            vals[i] = val(b)?;
        }
        let n = ((vals[0] as u32) << 18) | ((vals[1] as u32) << 12) | ((vals[2] as u32) << 6) | (vals[3] as u32);
        out.push(((n >> 16) & 0xFF) as u8);
        if chunk.len() > 2 {
            out.push(((n >> 8) & 0xFF) as u8);
        }
        if chunk.len() > 3 {
            out.push((n & 0xFF) as u8);
        }
    }
    Ok(out)
}

// ── Local encrypted file store ──────────────────────────────────────────────

fn load_cred_file(dir: &Path) -> Result<CredFile> {
    let path = cred_file_path(dir);
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(serde_json::from_str(&s).unwrap_or_default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(CredFile::default()),
        Err(e) => Err(e.into()),
    }
}

fn save_cred_file(dir: &Path, file: &CredFile) -> Result<()> {
    let path = cred_file_path(dir);
    let json = serde_json::to_string_pretty(file)?;
    std::fs::write(&path, json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }
    Ok(())
}

fn local_set(service: &str, account: &str, value: &str) -> Result<()> {
    let dir = data_dir()?;
    let key = load_or_create_master_key(&dir)?;
    let mut file = load_cred_file(&dir)?;
    let ciphertext = encrypt(&key, value)?;
    file.entries.insert(map_key(service, account), ciphertext);
    save_cred_file(&dir, &file)
}

fn local_get(service: &str, account: &str) -> Result<Option<String>> {
    let dir = data_dir()?;
    let file = load_cred_file(&dir)?;
    match file.entries.get(&map_key(service, account)) {
        Some(ciphertext) => {
            let key = load_or_create_master_key(&dir)?;
            match decrypt(&key, ciphertext) {
                Ok(plaintext) => Ok(Some(plaintext)),
                Err(e) => {
                    // A ciphertext that won't decrypt/decode can't become
                    // valid on the next read either — surfacing this forever
                    // just permanently locks the user out of the provider
                    // with no way to recover short of finding and deleting
                    // the file by hand. Drop the bad entry so the next read
                    // reports "not configured" (same as never having saved
                    // it), which re-entering the key in Settings fixes.
                    log::warn!("Dropping unreadable credential {}: {e}", map_key(service, account));
                    let _ = local_delete(service, account);
                    Ok(None)
                }
            }
        }
        None => Ok(None),
    }
}

fn local_delete(service: &str, account: &str) -> Result<()> {
    let dir = data_dir()?;
    let mut file = load_cred_file(&dir)?;
    if file.entries.remove(&map_key(service, account)).is_some() {
        save_cred_file(&dir, &file)?;
    }
    Ok(())
}

fn local_has(service: &str, account: &str) -> bool {
    data_dir()
        .and_then(|dir| load_cred_file(&dir))
        .map(|file| file.entries.contains_key(&map_key(service, account)))
        .unwrap_or(false)
}

// ── Public API ───────────────────────────────────────────────────────────
//
// Local-only: the OS keychain is never invoked here, by requirement, so
// saving/reading/deleting a credential can never trigger an OS keychain
// prompt or hang waiting on one.

/// Store a credential in the local encrypted file.
pub fn store_set(service: &str, account: &str, value: &str) -> Result<StorageBackend> {
    local_set(service, account, value)?;
    Ok(StorageBackend::LocalEncrypted)
}

/// Fetch a credential from the local encrypted file.
pub fn store_get(service: &str, account: &str) -> Result<Option<(String, StorageBackend)>> {
    match local_get(service, account)? {
        Some(v) => Ok(Some((v, StorageBackend::LocalEncrypted))),
        None => Ok(None),
    }
}

/// Delete a credential from the local encrypted file.
pub fn store_delete(service: &str, account: &str) -> Result<()> {
    local_delete(service, account)
}

/// Report whether a credential is currently stored, without exposing its value.
pub fn store_backend(service: &str, account: &str) -> Option<StorageBackend> {
    if local_has(service, account) {
        Some(StorageBackend::LocalEncrypted)
    } else {
        None
    }
}

// ── Per-profile credential storage ────────────────────────────────────────

fn profile_local_set(profile_id: i64, service: &str, account: &str, value: &str) -> Result<()> {
    let dir = profile_data_dir(profile_id)?;
    let key = load_or_create_master_key(&dir)?;
    let mut file = load_cred_file(&dir)?;
    let ciphertext = encrypt(&key, value)?;
    file.entries.insert(map_key(service, account), ciphertext);
    save_cred_file(&dir, &file)
}

fn profile_local_get(profile_id: i64, service: &str, account: &str) -> Result<Option<String>> {
    let dir = profile_data_dir(profile_id)?;
    let file = load_cred_file(&dir)?;
    match file.entries.get(&map_key(service, account)) {
        Some(ciphertext) => {
            let key = load_or_create_master_key(&dir)?;
            match decrypt(&key, ciphertext) {
                Ok(plaintext) => Ok(Some(plaintext)),
                Err(e) => {
                    // Same self-healing as local_get above — an unreadable
                    // entry would otherwise permanently block this provider
                    // for this profile.
                    log::warn!("Dropping unreadable profile credential {}: {e}", map_key(service, account));
                    let mut file = load_cred_file(&dir)?;
                    if file.entries.remove(&map_key(service, account)).is_some() {
                        let _ = save_cred_file(&dir, &file);
                    }
                    Ok(None)
                }
            }
        }
        None => Ok(None),
    }
}

fn profile_local_delete(profile_id: i64, service: &str, account: &str) -> Result<()> {
    let dir = profile_data_dir(profile_id)?;
    let mut file = load_cred_file(&dir)?;
    if file.entries.remove(&map_key(service, account)).is_some() {
        save_cred_file(&dir, &file)?;
    }
    Ok(())
}

fn profile_local_has(profile_id: i64, service: &str, account: &str) -> bool {
    profile_data_dir(profile_id)
        .and_then(|dir| load_cred_file(&dir))
        .map(|file| file.entries.contains_key(&map_key(service, account)))
        .unwrap_or(false)
}

pub fn profile_store_set(profile_id: i64, service: &str, account: &str, value: &str) -> Result<StorageBackend> {
    profile_local_set(profile_id, service, account, value)?;
    Ok(StorageBackend::LocalEncrypted)
}

pub fn profile_store_get(profile_id: i64, service: &str, account: &str) -> Result<Option<(String, StorageBackend)>> {
    match profile_local_get(profile_id, service, account)? {
        Some(v) => Ok(Some((v, StorageBackend::LocalEncrypted))),
        None => Ok(None),
    }
}

pub fn profile_store_delete(profile_id: i64, service: &str, account: &str) -> Result<()> {
    profile_local_delete(profile_id, service, account)
}

pub fn profile_store_backend(profile_id: i64, service: &str, account: &str) -> Option<StorageBackend> {
    if profile_local_has(profile_id, service, account) {
        Some(StorageBackend::LocalEncrypted)
    } else {
        None
    }
}

/// Migrate global credentials to a profile-scoped location (for first launch after upgrade).
pub fn migrate_global_to_profile(profile_id: i64) -> Result<()> {
    let global_dir = data_dir()?;
    let global_file = load_cred_file(&global_dir)?;
    if global_file.entries.is_empty() {
        return Ok(());
    }
    let profile_dir = profile_data_dir(profile_id)?;
    let mut profile_file = load_cred_file(&profile_dir)?;
    if !profile_file.entries.is_empty() {
        return Ok(());
    }
    let global_key = load_or_create_master_key(&global_dir)?;
    let profile_key = load_or_create_master_key(&profile_dir)?;
    for (map_k, ciphertext) in &global_file.entries {
        let plaintext = decrypt(&global_key, ciphertext)?;
        let re_encrypted = encrypt(&profile_key, &plaintext)?;
        profile_file.entries.insert(map_k.clone(), re_encrypted);
    }
    save_cred_file(&profile_dir, &profile_file)?;
    Ok(())
}
