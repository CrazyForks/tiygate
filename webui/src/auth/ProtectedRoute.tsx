import type { PropsWithChildren } from "react";
import { Navigate } from "react-router-dom";
import { useAuth } from "@/auth/AuthContext";
import { checkIsFirstRun } from "@/auth/setup";
import { useEffect, useState } from "react";
import { Spinner } from "@/components/ui";

export default function ProtectedRoute({ children }: PropsWithChildren) {
  const { isAuthenticated, isTauri, isInitializing } = useAuth();
  const [firstRun, setFirstRun] = useState<boolean | null>(null);

  // In Tauri mode, check if this is the first run (setup needed).
  useEffect(() => {
    if (!isTauri || isInitializing) return;
    if (isAuthenticated) {
      setFirstRun(false);
      return;
    }
    let cancelled = false;
    (async () => {
      const fr = await checkIsFirstRun();
      if (!cancelled) setFirstRun(fr);
    })();
    return () => {
      cancelled = true;
    };
  }, [isTauri, isInitializing, isAuthenticated]);

  // In Tauri mode, show a spinner while initializing or while the
  // first-run check is still pending (firstRun === null).
  if (isTauri && (isInitializing || (!isAuthenticated && firstRun === null))) {
    return (
      <div className="flex min-h-full items-center justify-center bg-bg">
        <Spinner />
      </div>
    );
  }

  // In Tauri mode, if not authenticated and first run, go to setup.
  if (isTauri && !isAuthenticated && firstRun === true) {
    return <Navigate to="/setup" replace />;
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }
  return <>{children}</>;
}
