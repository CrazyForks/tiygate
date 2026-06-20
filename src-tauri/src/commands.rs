//! Tauri commands exposed to the frontend.
//!
//! These commands let the Setup wizard and AuthContext interact with
//! the local client configuration and sidecar process:
//!
//! - `is_first_run` — whether the setup wizard should be shown.
//! - `get_admin_token` — retrieve the stored token (for auto-login).
//! - `set_admin_token` — set a user-chosen token and restart the sidecar.
//! - `enable_passwordless` — keep the auto-generated token and mark
//!   setup as done.
//! - `get_server_port` — the port the sidecar is listening on.

use tauri::{AppHandle, Manager, State};

use crate::sidecar;
use crate::AppState;

/// Returns `true` when the setup wizard has not been completed yet.
/// On lock failure returns `true` (conservative: show setup).
#[tauri::command]
pub fn is_first_run(state: State<'_, AppState>) -> bool {
    match state.config.lock() {
        Ok(cfg) => !cfg.first_run_completed,
        Err(_) => true,
    }
}

/// Returns the stored admin token so the frontend can auto-login
/// without showing the Login page. Returns `None` when no token is
/// configured or the lock is poisoned.
#[tauri::command]
pub fn get_admin_token(state: State<'_, AppState>) -> Option<String> {
    state.config.lock().ok().map(|cfg| cfg.admin_token.clone())
}

/// Returns the port the sidecar is listening on. Returns 0 on failure.
#[tauri::command]
pub fn get_server_port(state: State<'_, AppState>) -> u16 {
    state.server_port.lock().map(|p| *p).unwrap_or(0)
}

/// Returns the master key used to encrypt provider API keys and other
/// secrets at rest. The setup wizard displays this to the user after
/// first-run so they can save it for future data migration / restore.
/// Returns `None` on lock failure or in non-Tauri environments.
#[tauri::command]
pub fn get_master_key(state: State<'_, AppState>) -> Option<String> {
    state.config.lock().ok().map(|cfg| cfg.master_key.clone())
}

/// Apply a master key (e.g. generated or rotated on the frontend),
/// persist it to config, and restart the sidecar so the new
/// `TIYGATE_MASTER_KEY` takes effect.
#[tauri::command]
pub async fn apply_master_key(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("master key cannot be empty".into());
    }
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        let data_dir = app
            .path()
            .app_local_data_dir()
            .map_err(|e| format!("failed to resolve app_local_data_dir: {e}"))?;
        cfg.master_key = key.trim().to_string();
        cfg.save(&data_dir)
            .map_err(|e| format!("failed to save config: {e}"))?;
    }
    restart_with_current_config(&app, &state).await
}

/// Set a user-chosen admin token, persist it, and restart the sidecar
/// so the new `TIYGATE_ADMIN_TOKEN` takes effect. Marks first-run as
/// complete. After this call, the frontend should redirect the user to
/// the Login page so they can authenticate with the token they chose.
#[tauri::command]
pub async fn set_admin_token(
    app: AppHandle,
    state: State<'_, AppState>,
    token: String,
) -> Result<(), String> {
    if token.trim().is_empty() {
        return Err("token cannot be empty".into());
    }
    let token = token.trim().to_string();

    // Update config and persist.
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        let data_dir = app
            .path()
            .app_local_data_dir()
            .map_err(|e| format!("failed to resolve app_local_data_dir: {e}"))?;
        cfg.update_admin_token(token.clone(), &data_dir)
            .map_err(|e| format!("failed to save config: {e}"))?;
        cfg.mark_first_run_done(&data_dir)
            .map_err(|e| format!("failed to save config: {e}"))?;
    }

    restart_with_current_config(&app, &state).await
}

/// Enable passwordless mode: keep the auto-generated token, mark
/// first-run as complete, and return the token so the frontend can
/// auto-login. No sidecar restart is needed because the token was
/// already injected at startup.
#[tauri::command]
pub async fn enable_passwordless(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let token = {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        let data_dir = app
            .path()
            .app_local_data_dir()
            .map_err(|e| format!("failed to resolve app_local_data_dir: {e}"))?;
        cfg.mark_first_run_done(&data_dir)
            .map_err(|e| format!("failed to save config: {e}"))?;
        cfg.admin_token.clone()
    };
    Ok(token)
}

/// Restart the sidecar using the current configuration values. This is
/// called after `set_admin_token` changes the token, since
/// `TIYGATE_ADMIN_TOKEN` is read at sidecar startup.
async fn restart_with_current_config(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<(), String> {
    let (port, admin_token, master_key, db_url) = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        let port = *state.server_port.lock().map_err(|e| e.to_string())?;
        let data_dir = app
            .path()
            .app_local_data_dir()
            .map_err(|e| format!("failed to resolve app_local_data_dir: {e}"))?;
        let db_path = data_dir.join("tiygate.db");
        let db_url = format!(
            "sqlite://{}?mode=rwc",
            db_path.to_string_lossy().replace('\\', "/")
        );
        (
            port,
            cfg.admin_token.clone(),
            cfg.master_key.clone(),
            db_url,
        )
    };

    // Take the old sidecar manager out of state, then release the lock
    // before awaiting (MutexGuard is not Send).
    let old_mgr = {
        let mut guard = state.sidecar.lock().map_err(|e| e.to_string())?;
        guard.take()
    };
    if let Some(mut old) = old_mgr {
        old.shutdown().await;
    }

    // Brief pause to let the old process release the port.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let new_mgr = sidecar::spawn_sidecar(app, port, &admin_token, &master_key, &db_url)
        .await
        .map_err(|e| format!("failed to restart sidecar: {e}"))?;

    let mut guard = state.sidecar.lock().map_err(|e| e.to_string())?;
    *guard = Some(new_mgr);
    Ok(())
}
