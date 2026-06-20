// Tauri environment bridge.
//
// This module detects whether the webui is running inside a Tauri
// webview and, when so, provides helpers that invoke the Rust-side
// Tauri commands for first-run detection, token retrieval, and setup.
//
// In a plain browser (non-Tauri) environment all helpers degrade to
// no-ops / null returns so the existing login flow is unaffected.

// We import dynamically so that the webui still builds without
// @tauri-apps/api installed (browser-only builds).

/** Whether the app is running inside a Tauri webview. */
export function isTauri(): boolean {
  return (
    typeof window !== "undefined" &&
    "__TAURI_INTERNALS__" in window
  );
}

// Lazily load the Tauri invoke shim. We use a dynamic import wrapped in
// a helper so the module graph stays optional.
async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const mod = await import("@tauri-apps/api/core");
  return mod.invoke<T>(cmd, args);
}

/**
 * Ask the Rust backend whether the setup wizard should be shown.
 * Returns `false` in non-Tauri environments.
 */
export async function checkIsFirstRun(): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    return await invoke<boolean>("is_first_run");
  } catch {
    return false;
  }
}

/**
 * Retrieve the stored admin token from the Rust backend (for auto-login).
 * Returns `null` in non-Tauri environments or when no token is stored.
 */
export async function tauriGetAdminToken(): Promise<string | null> {
  if (!isTauri()) return null;
  try {
    return await invoke<string | null>("get_admin_token");
  } catch {
    return null;
  }
}

/**
 * Get the port the sidecar is listening on.
 * Returns `null` in non-Tauri environments.
 */
export async function tauriGetServerPort(): Promise<number | null> {
  if (!isTauri()) return null;
  try {
    const port = await invoke<number>("get_server_port");
    return port > 0 ? port : null;
  } catch {
    return null;
  }
}

/**
 * Set a user-chosen admin token. The Rust backend persists it, marks
 * first-run as complete, and restarts the sidecar. After this resolves,
 * the caller should redirect to the login page.
 */
export async function tauriSetAdminToken(token: string): Promise<void> {
  await invoke<void>("set_admin_token", { token });
}

/**
 * Enable passwordless mode. The Rust backend marks first-run as complete
 * and returns the auto-generated token so the frontend can auto-login.
 */
export async function tauriEnablePasswordless(): Promise<string> {
  return await invoke<string>("enable_passwordless");
}

/**
 * Retrieve the auto-generated master key so the setup wizard can
 * display it to the user. Returns `null` in non-Tauri environments.
 */
export async function tauriGetMasterKey(): Promise<string | null> {
  if (!isTauri()) return null;
  try {
    return await invoke<string | null>("get_master_key");
  } catch {
    return null;
  }
}

/**
 * Apply a master key (persist + restart sidecar). Called when the user
 * clicks "continue" on the master-key step.
 */
export async function tauriApplyMasterKey(key: string): Promise<void> {
  await invoke<void>("apply_master_key", { key });
}
