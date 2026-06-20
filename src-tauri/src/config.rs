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

/// A user-configured remote TiyGate instance.
///
/// Remote instances connect to an already-running TiyGate server
/// (e.g. deployed on a VPS). Unlike the local sidecar, they skip
/// token/master-key initialization — the Admin Token is entered on
/// the Login page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceEntry {
    /// Unique identifier (random hex string).
    pub id: String,
    /// Human-friendly label shown in the instance selector.
    pub label: String,
    /// Base URL of the remote instance, e.g. `https://gateway.example.com`.
    /// Stored without a trailing slash.
    pub url: String,
    /// When `true`, TLS certificate errors are ignored during health
    /// checks. The frontend always uses the browser's default TLS
    /// behaviour for API calls; this flag only affects the Rust-side
    /// healthz probe that feeds the status indicator.
    #[serde(default)]
    pub skip_tls_verify: bool,
}

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
    /// User-added remote instances. The local sidecar is always
    /// available implicitly and is not stored in this list.
    #[serde(default)]
    pub instances: Vec<InstanceEntry>,
    /// The currently active instance id. `None` means the local
    /// sidecar is active.
    #[serde(default)]
    pub active_instance_id: Option<String>,
    /// The last instance the user selected, so the Setup wizard can
    /// default to it on the next launch. `None` means local.
    #[serde(default)]
    pub last_instance_id: Option<String>,
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
            instances: Vec::new(),
            active_instance_id: None,
            last_instance_id: None,
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

    // ---- Instance management ----

    /// Add a new remote instance and return it. The caller is
    /// responsible for persisting via `save`.
    pub fn add_instance(&mut self, mut entry: InstanceEntry) -> &InstanceEntry {
        if entry.id.is_empty() {
            entry.id = generate_random_token(8);
        }
        entry.url = normalize_url(&entry.url);
        self.instances.push(entry);
        self.instances.last().expect("just pushed")
    }

    /// Update an existing instance by id. Returns `true` if found.
    pub fn update_instance(
        &mut self,
        id: &str,
        label: String,
        url: String,
        skip_tls_verify: bool,
    ) -> bool {
        if let Some(inst) = self.instances.iter_mut().find(|i| i.id == id) {
            inst.label = label;
            inst.url = normalize_url(&url);
            inst.skip_tls_verify = skip_tls_verify;
            true
        } else {
            false
        }
    }

    /// Remove an instance by id. If the removed instance was active,
    /// `active_instance_id` is reset to `None` (falls back to local).
    /// Returns `true` if an instance was removed.
    pub fn remove_instance(&mut self, id: &str) -> bool {
        let before = self.instances.len();
        self.instances.retain(|i| i.id != id);
        let removed = self.instances.len() < before;
        if removed {
            if self.active_instance_id.as_deref() == Some(id) {
                self.active_instance_id = None;
            }
            if self.last_instance_id.as_deref() == Some(id) {
                self.last_instance_id = None;
            }
        }
        removed
    }

    /// Set the active instance. `None` selects the local sidecar.
    pub fn set_active_instance(&mut self, id: Option<String>) {
        self.active_instance_id = id;
    }

    /// Record the last-selected instance so the Setup wizard can
    /// default to it on the next launch.
    pub fn set_last_instance(&mut self, id: Option<String>) {
        self.last_instance_id = id;
    }

    /// Validate that `active_instance_id` still points to an existing
    /// remote instance. If it doesn't (e.g. the instance was deleted),
    /// reset it to `None`. Called at startup after loading config.
    pub fn reconcile_active_instance(&mut self) {
        if let Some(ref id) = self.active_instance_id {
            if !self.instances.iter().any(|i| &i.id == id) {
                self.active_instance_id = None;
            }
        }
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

/// Normalize a user-entered URL: ensure it has a scheme (default
/// https), and strip any trailing slash so that `{url}/admin/v1`
/// produces a clean path.
fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Prepend https:// if the user omitted the scheme.
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    // Strip trailing slashes.
    with_scheme.trim_end_matches('/').to_string()
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

    #[test]
    fn old_config_deserializes_with_defaults() {
        // Simulate an old config.json that lacks the new instance fields.
        let json = r#"{
            "admin_token": "abc",
            "master_key": "def",
            "first_run_completed": true,
            "server_port": 13000
        }"#;
        let cfg: ClientConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.instances.is_empty());
        assert_eq!(cfg.active_instance_id, None);
        assert_eq!(cfg.last_instance_id, None);
    }

    #[test]
    fn add_and_remove_instance() {
        let mut cfg = ClientConfig::generate();
        let entry = InstanceEntry {
            id: String::new(),
            label: "Prod".into(),
            url: "gateway.example.com/".into(),
            skip_tls_verify: false,
        };
        let added = cfg.add_instance(entry);
        assert!(!added.id.is_empty());
        assert_eq!(added.url, "https://gateway.example.com");
        let id = added.id.clone();
        let url = added.url.clone();
        let _ = added;
        assert_eq!(cfg.instances.len(), 1);
        assert_eq!(cfg.instances[0].id, id);
        assert_eq!(cfg.instances[0].url, url);

        // Set active to the remote, then remove it → should fall back.
        cfg.set_active_instance(Some(id.clone()));
        assert_eq!(cfg.active_instance_id, Some(id.clone()));
        assert!(cfg.remove_instance(&id));
        assert_eq!(cfg.active_instance_id, None);
        assert!(cfg.instances.is_empty());
    }

    #[test]
    fn normalize_url_adds_scheme_and_strips_slash() {
        assert_eq!(normalize_url("example.com/"), "https://example.com");
        assert_eq!(normalize_url("http://foo.io/bar/"), "http://foo.io/bar");
        assert_eq!(normalize_url("https://x.y/"), "https://x.y");
        assert_eq!(normalize_url("https://a.com///"), "https://a.com");
        assert_eq!(normalize_url("  a.com  "), "https://a.com");
    }

    #[test]
    fn reconcile_active_instance_resets_orphan() {
        let mut cfg = ClientConfig::generate();
        cfg.active_instance_id = Some("ghost".into());
        cfg.reconcile_active_instance();
        assert_eq!(cfg.active_instance_id, None);

        // Existing instance is preserved.
        let entry = InstanceEntry {
            id: "real".into(),
            label: "Real".into(),
            url: "https://real.io".into(),
            skip_tls_verify: false,
        };
        cfg.add_instance(entry);
        cfg.active_instance_id = Some("real".into());
        cfg.reconcile_active_instance();
        assert_eq!(cfg.active_instance_id.as_deref(), Some("real"));
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
