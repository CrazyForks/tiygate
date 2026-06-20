import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { ArrowLeftRight } from "lucide-react";
import {
  tauriGetActiveInstance,
  tauriCheckInstanceHealth,
  tauriListInstances,
  isTauri,
  type ActiveInstance,
  type HealthStatus,
} from "@/auth/setup";
import { cn } from "@/lib/cn";

const POLL_INTERVAL_MS = 30_000;

/** Colored status dot for the instance health indicator. */
function StatusDot({ status }: { status: HealthStatus | null }) {
  const color =
    status === "ok"
      ? "bg-success"
      : status === "warning"
        ? "bg-warning"
        : status === "error"
          ? "bg-danger"
          : "bg-text-subtle";
  const label =
    status === "ok"
      ? "ok"
      : status === "warning"
        ? "warning"
        : status === "error"
          ? "error"
          : "unreachable";
  return (
    <span
      className={cn("inline-block h-2 w-2 shrink-0 rounded-full", color)}
      title={label}
      aria-label={label}
    />
  );
}

/**
 * Dashboard header indicator showing the currently active instance
 * (local or remote), a health status dot, and a switch icon that
 * navigates to the Setup wizard's instance-selection step.
 *
 * Only rendered in Tauri environments — in a browser deployment the
 * API origin is always the same as the SPA origin.
 */
export function InstanceIndicator() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [active, setActive] = useState<ActiveInstance | null>(null);
  const [health, setHealth] = useState<HealthStatus | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;

    async function probe() {
      try {
        const inst = await tauriGetActiveInstance();
        if (cancelled) return;
        setActive(inst);
        if (inst?.url) {
          let skipTls = false;
          if (inst.kind === "remote" && inst.id) {
            const instances = await tauriListInstances();
            const match = instances.find((i) => i.id === inst.id);
            if (match) skipTls = match.skip_tls_verify;
          }
          const status = await tauriCheckInstanceHealth(inst.url, skipTls);
          if (!cancelled) setHealth(status);
        }
      } catch {
        // Silently degrade — the indicator just shows no dot.
      }
    }

    probe();
    const timer = setInterval(probe, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  if (!active) return null;

  const displayLabel =
    active.kind === "local"
      ? t("setup.localInstance")
      : (active.label ?? active.url ?? "remote");
  const displayUrl = active.url ?? "";

  return (
    <button
      type="button"
      onClick={() => navigate("/setup")}
      className="flex items-center gap-2 rounded-md border border-border bg-surface px-3 py-1.5 text-left transition-colors hover:border-accent hover:bg-accent/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      title={t("setup.switchInstance")}
    >
      <StatusDot status={health} />
      <span className="text-xs font-medium text-text whitespace-nowrap">
        {displayLabel}
        {displayUrl && (
          <span className="text-text-subtle font-normal">（{displayUrl}）</span>
        )}
      </span>
      <ArrowLeftRight
        size={14}
        className="shrink-0 text-text-muted"
        aria-label={t("setup.switchInstance")}
      />
    </button>
  );
}
