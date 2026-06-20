mod commands;
mod config;
mod sidecar;

use std::sync::Mutex;

use tauri::Manager;

use crate::config::ClientConfig;
use crate::sidecar::SidecarManager;

/// Shared state managed by Tauri, holding the sidecar handle and the
/// resolved client configuration.
pub struct AppState {
    pub sidecar: Mutex<Option<SidecarManager>>,
    pub config: Mutex<ClientConfig>,
    pub server_port: Mutex<u16>,
}

/// Entry point for the Tauri client application.
pub fn run() {
    init_tracing();

    let app = match tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle().clone();

            // Resolve the data directory and load (or create) the local
            // client configuration before spawning the sidecar.
            // Use app_local_data_dir (~/Library/Application Support/ on
            // macOS) to avoid triggering the macOS "Documents" TCC
            // permission prompt that app_data_dir can cause in unsigned
            // / debug builds.
            let data_dir = handle
                .path()
                .app_local_data_dir()
                .map_err(|e| anyhow::anyhow!("failed to resolve app_local_data_dir: {e}"))?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| anyhow::anyhow!("failed to create data dir: {e}"))?;

            let mut client_config = ClientConfig::load_or_init(&data_dir)?;

            // Scan for an available port starting from 13000.
            let port = sidecar::find_available_port(13000)
                .ok_or_else(|| anyhow::anyhow!("no available port in range 13000-13099"))?;

            let db_path = data_dir.join("tiygate.db");
            let db_url = format!(
                "sqlite://{}?mode=rwc",
                db_path.to_string_lossy().replace('\\', "/")
            );

            // Use the admin token already stored in the config (generated
            // during load_or_init when missing). The sidecar inherits it
            // through the TIYGATE_ADMIN_TOKEN environment variable.
            let admin_token = client_config.admin_token.clone();
            let master_key = client_config.master_key.clone();

            let sidecar_mgr = tauri::async_runtime::block_on(async {
                sidecar::spawn_sidecar(&handle, port, &admin_token, &master_key, &db_url).await
            })?;

            client_config.server_port = Some(port);
            client_config.save(&data_dir)?;

            app.manage(AppState {
                sidecar: Mutex::new(Some(sidecar_mgr)),
                config: Mutex::new(client_config),
                server_port: Mutex::new(port),
            });

            // The webview loads frontendDist (tauri://localhost) which
            // has Tauri IPC. The frontend uses Tauri commands to get
            // the sidecar port and makes cross-origin fetch calls to
            // http://127.0.0.1:{port}/admin/v1/* for the API.
            // No window.eval redirect needed.

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::is_first_run,
            commands::get_admin_token,
            commands::set_admin_token,
            commands::enable_passwordless,
            commands::get_server_port,
            commands::get_master_key,
            commands::apply_master_key,
        ])
        .on_window_event(|window, event| {
            // When the main window is closed (e.g. clicking the red
            // traffic-light button on macOS), shut down the sidecar.
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                shutdown_sidecar(window.app_handle());
            }
        })
        .build(tauri::generate_context!())
    {
        Ok(app) => app,
        Err(e) => {
            tracing::error!("failed to build Tauri application: {e}");
            return;
        }
    };

    // Handle application-level exit events (Cmd+Q, dock quit, etc.).
    // These do NOT trigger WindowEvent::CloseRequested, so the sidecar
    // must be cleaned up here as well.
    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            shutdown_sidecar(app_handle);
        }
    });
}

/// Shut down the sidecar process if it is still running. Safe to call
/// multiple times — the second call is a no-op because the manager is
/// `take()`n from the mutex on the first call.
fn shutdown_sidecar(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut guard) = state.sidecar.lock() {
            if let Some(mut mgr) = guard.take() {
                tracing::info!("shutting down sidecar on exit");
                tauri::async_runtime::block_on(async {
                    mgr.shutdown().await;
                });
            }
        }
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}
