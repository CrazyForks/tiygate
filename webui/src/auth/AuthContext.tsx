import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type PropsWithChildren,
} from "react";
import { useQueryClient } from "@tanstack/react-query";
import { setUnauthorizedHandler } from "@/api/client";
import { clearToken, getToken, setToken } from "./token";
import {
  isTauri,
  tauriGetAdminToken,
  checkIsFirstRun,
  tauriGetActiveInstance,
} from "./setup";

interface AuthState {
  token: string | null;
  isAuthenticated: boolean;
  /** Whether the app is running inside the Tauri desktop client. */
  isTauri: boolean;
  /**
   * `true` until the initial Tauri auto-login check completes. In
   * non-Tauri environments this is always `false`.
   */
  isInitializing: boolean;
  /** Whether the user is in passwordless mode (no logout button). */
  isPasswordless: boolean;
  setPasswordless: (v: boolean) => void;
  login: (token: string, remember: boolean) => void;
  logout: () => void;
}

const AuthContext = createContext<AuthState | null>(null);

export function AuthProvider({ children }: PropsWithChildren) {
  const tauri = isTauri();
  const [token, setTokenState] = useState<string | null>(() => getToken());
  const [isInitializing, setIsInitializing] = useState(tauri);
  const [isPasswordless, setIsPasswordless] = useState(false);
  const queryClient = useQueryClient();

  const logout = useCallback(() => {
    clearToken();
    setTokenState(null);
    queryClient.clear();
  }, [queryClient]);

  const login = useCallback((newToken: string, remember: boolean) => {
    setToken(newToken, remember);
    setTokenState(newToken);
  }, []);

  // In Tauri environments, attempt to auto-login on mount:
  // - If first-run is not complete, do nothing (the Setup page handles it).
  // - If first-run is complete, check the active instance:
  //   - Local sidecar → fetch the stored token and auto-login
  //     (passwordless flow). Mark it so the logout button is hidden.
  //   - Remote instance → do NOT auto-login; the user must enter the
  //     Admin Token manually on the Login page.
  useEffect(() => {
    if (!tauri) {
      setIsInitializing(false);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const firstRun = await checkIsFirstRun();
        if (!firstRun) {
          const active = await tauriGetActiveInstance();
          // Only auto-login for the local sidecar. Remote instances
          // require manual token entry.
          if (active?.kind === "local") {
            const storedToken = await tauriGetAdminToken();
            if (storedToken && !cancelled) {
              setToken(storedToken, true);
              setTokenState(storedToken);
              setIsPasswordless(true);
            }
          } else if (active?.kind === "remote") {
            // Clear any cached React Query data from a previous
            // instance to prevent cross-instance data leakage.
            queryClient.clear();
          }
          // For remote instances, isInitializing completes without
          // setting a token, so ProtectedRoute will redirect to /login.
        }
      } catch {
        // Degrade gracefully — user can use the login page manually.
      } finally {
        if (!cancelled) setIsInitializing(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [tauri, queryClient]);

  // Wire the API client's 401 handler so any rejected request drops
  // the session and bounces the user back to login.
  useEffect(() => {
    setUnauthorizedHandler(() => {
      setTokenState(null);
      queryClient.clear();
    });
    return () => setUnauthorizedHandler(null);
  }, [queryClient]);

  const setPasswordless = useCallback((v: boolean) => setIsPasswordless(v), []);

  const value = useMemo<AuthState>(
    () => ({
      token,
      isAuthenticated: token !== null,
      isTauri: tauri,
      isInitializing,
      isPasswordless,
      setPasswordless,
      login,
      logout,
    }),
    [
      token,
      tauri,
      isInitializing,
      isPasswordless,
      setPasswordless,
      login,
      logout,
    ],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
