//! Local client configuration persistence.
//!
//! The config file lives at `<app_local_data_dir>/config.json` and stores the
//! admin token, master key, first-run flag, and last-used server port.
//! The master key is a 32-byte random hex string generated on first
//! launch and used to encrypt provider secrets inside the SQLite
//! database managed by the sidecar.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rand::RngCore;
use serde::{Deserialize, Serialize};

const CONFIG_FILENAME: &str = "config.json";

/// On-disk client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Bearer token injected as `TIYGATE_ADMIN_TOKEN`. Generated on
    /// first launch so the sidecar is never in the 503 "unconfigured"
    /// state, even when the user picks passwordless mode.
    pub admin_token: String,
    /// 64-char hex string injected as `TIYGATE_MASTER_KEY`.
    pub master_key: String,
    /// `false` until the user completes the setup wizard.
    pub first_run_completed: bool,
    /// Last port the sidecar was started on (informational).
    #[serde(default)]
    pub server_port: Option<u16>,
}

impl ClientConfig {
    /// Load the config from `<data_dir>/config.json`, or create a new
    /// one with freshly generated secrets when the file does not exist.
    pub fn load_or_init(data_dir: &Path) -> Result<Self> {
        let path = config_path(data_dir);
        if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let cfg: ClientConfig = serde_json::from_str(&raw)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            Ok(cfg)
        } else {
            let cfg = Self::generate();
            cfg.save(data_dir)?;
            tracing::info!("created initial client config at {}", path.display());
            Ok(cfg)
        }
    }

    /// Generate a fresh config with random secrets.
    fn generate() -> Self {
        Self {
            admin_token: generate_random_token(32),
            master_key: generate_random_hex(32),
            first_run_completed: false,
            server_port: None,
        }
    }

    /// Persist the config to `<data_dir>/config.json` atomically.
    pub fn save(&self, data_dir: &Path) -> Result<()> {
        let path = config_path(data_dir);
        let raw =
            serde_json::to_string_pretty(self).context("failed to serialize client config")?;
        // Write to a temp file then rename for atomicity.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, raw).with_context(|| format!("failed to write {}", tmp.display()))?;
        std::fs::rename(&tmp, &path)
            .with_context(|| format!("failed to rename {} -> {}", tmp.display(), path.display()))?;
        Ok(())
    }

    /// Mark the first-run wizard as completed and persist.
    pub fn mark_first_run_done(&mut self, data_dir: &Path) -> Result<()> {
        self.first_run_completed = true;
        self.save(data_dir)
    }

    /// Update the admin token, persist, and return the new value.
    pub fn update_admin_token(&mut self, token: String, data_dir: &Path) -> Result<()> {
        self.admin_token = token;
        self.save(data_dir)
    }
}

fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CONFIG_FILENAME)
}

/// Generate a cryptographically random hex string of `num_bytes` bytes
/// (i.e. `num_bytes * 2` hex chars).
fn generate_random_hex(num_bytes: usize) -> String {
    let mut buf = vec![0u8; num_bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(&buf)
}

/// Generate a URL-safe random token from `num_bytes` random bytes.
fn generate_random_token(num_bytes: usize) -> String {
    let mut buf = vec![0u8; num_bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    // Use hex for simplicity and readability.
    hex::encode(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_hex_is_correct_length() {
        let key = generate_random_hex(32);
        assert_eq!(key.len(), 64);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn config_round_trips() {
        let tmp = tempfile_dir();
        std::fs::create_dir_all(&tmp).unwrap();
        let cfg = ClientConfig::generate();
        cfg.save(&tmp).unwrap();
        let loaded = ClientConfig::load_or_init(&tmp).unwrap();
        assert_eq!(cfg.admin_token, loaded.admin_token);
        assert_eq!(cfg.master_key, loaded.master_key);
        assert_eq!(cfg.first_run_completed, loaded.first_run_completed);
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn load_or_init_creates_on_first_run() {
        let tmp = tempfile_dir();
        std::fs::create_dir_all(&tmp).unwrap();
        let cfg = ClientConfig::load_or_init(&tmp).unwrap();
        assert!(!cfg.first_run_completed);
        assert!(!cfg.admin_token.is_empty());
        assert_eq!(cfg.master_key.len(), 64);
        // Second load should read the same values.
        let cfg2 = ClientConfig::load_or_init(&tmp).unwrap();
        assert_eq!(cfg.admin_token, cfg2.admin_token);
        std::fs::remove_dir_all(&tmp).ok();
    }

    fn tempfile_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "tiygate-test-{}-{}",
            std::process::id(),
            rand::thread_rng().next_u32()
        ));
        dir
    }
}
