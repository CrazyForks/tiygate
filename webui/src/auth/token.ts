// Per-instance token storage.
//
// The admin token is the only credential the UI holds. We scope it by
// instance key so that switching between the local sidecar and remote
// instances preserves each instance's "remember me" token without
// cross-contamination.
//
// - instanceKey `"local"` → local sidecar
// - instanceKey `"<uuid>"` → a user-added remote instance
//
// We store in sessionStorage by default (cleared when the tab closes);
// operators can opt into localStorage ("remember me") so the token
// survives reloads. The single super-user token model means we keep
// the surface minimal and never log the value.

const PREFIX = "tiygate.admin.token";

function storageKey(instanceKey: string): string {
  return instanceKey ? `${PREFIX}.${instanceKey}` : PREFIX;
}

export function getToken(instanceKey = ""): string | null {
  const key = storageKey(instanceKey);
  return window.sessionStorage.getItem(key) ?? window.localStorage.getItem(key);
}

export function setToken(
  token: string,
  remember: boolean,
  instanceKey = "",
): void {
  const key = storageKey(instanceKey);
  if (remember) {
    window.localStorage.setItem(key, token);
    window.sessionStorage.removeItem(key);
  } else {
    window.sessionStorage.setItem(key, token);
    window.localStorage.removeItem(key);
  }
}

export function clearToken(instanceKey = ""): void {
  const key = storageKey(instanceKey);
  window.sessionStorage.removeItem(key);
  window.localStorage.removeItem(key);
}
