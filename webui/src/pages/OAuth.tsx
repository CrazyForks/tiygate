import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ExternalLink, Info, RefreshCw, Play, Copy, ClipboardPaste } from "lucide-react";
import { oauthApi, providersApi } from "@/api/resources";
import {
  Button,
  Card,
  CardBody,
  CardHeader,
  EmptyState,
  ErrorBox,
  Field,
  Select,
  Spinner,
  Tooltip,
  Alert,
  useToast,
} from "@/components/ui";
import { PageHeader } from "@/components/PageHeader";

export default function OAuth() {
  const { t } = useTranslation();
  const toast = useToast();
  const { data: providers, isLoading } = useQuery({
    queryKey: ["providers"],
    queryFn: providersApi.list,
  });

  const [providerId, setProviderId] = useState("");
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [callbackUrl, setCallbackUrl] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const startMutation = useMutation({
    mutationFn: () => oauthApi.start(providerId),
    onSuccess: (res) => {
      setError(null);
      setAuthUrl(res.url);
      setCallbackUrl("");
      setMessage(t("oauth.started"));
    },
    onError: (e: Error) => {
      setError(e.message);
      setAuthUrl(null);
      setMessage(null);
    },
  });

  const callbackMutation = useMutation({
    mutationFn: () => {
      const parsed = parseCallbackUrl(callbackUrl);
      if (!parsed) {
        throw new Error(t("oauth.callbackUrlInvalid"));
      }
      return oauthApi.callback(parsed.code, parsed.state);
    },
    onSuccess: (res) => {
      setError(null);
      const label = providerLabel(res.provider_id);
      setMessage(t("oauth.callbackSuccess", { provider: label }));
      toast.success(t("oauth.callbackSuccess", { provider: label }));
      setAuthUrl(null);
      setCallbackUrl("");
    },
    onError: (e: Error) => {
      setError(e.message);
      setMessage(null);
    },
  });

  const refreshMutation = useMutation({
    mutationFn: () => oauthApi.refresh(providerId),
    onSuccess: (res) => {
      setError(null);
      const label = providerLabel(res.provider_id);
      setMessage(t("oauth.refreshed", { provider: label }));
      toast.success(t("oauth.refreshed", { provider: label }));
    },
    onError: (e: Error) => {
      setError(e.message);
      setMessage(null);
    },
  });

  async function copyUrl() {
    if (!authUrl) return;
    try {
      await navigator.clipboard.writeText(authUrl);
      toast.success(t("oauth.urlCopied"));
    } catch {
      toast.error(t("common.copyFailed"));
    }
  }

  async function pasteFromClipboard() {
    try {
      const text = await navigator.clipboard.readText();
      setCallbackUrl(text.trim());
      toast.success(t("oauth.callbackUrlPasted"));
    } catch {
      toast.error(t("oauth.callbackUrlPasteFailed"));
    }
  }

  // Only show OAuth-mode providers — others can't use the OAuth flow.
  const oauthProviders = (providers ?? []).filter(
    (p) => p.auth_mode === "oauth",
  );

  /** Format a provider as "Name (id)" for display in messages. Falls
   * back to just the id if the provider list isn't loaded yet. */
  function providerLabel(id: string): string {
    const p = (providers ?? []).find((p) => p.id === id);
    return p ? `${p.name} (${p.id})` : id;
  }

  const providerOptions = [
    { value: "", label: t("oauth.selectPlaceholder") },
    ...oauthProviders.map((p) => ({
      value: p.id,
      label: `${p.name} (${p.id})`,
    })),
  ];

  return (
    <div>
      <PageHeader title={t("oauth.title")} />
      <Card className="max-w-xl">
        <CardHeader title={t("oauth.subtitle")} />
        {isLoading ? (
          <Spinner />
        ) : oauthProviders.length === 0 ? (
          <EmptyState title={t("oauth.noProviders")} />
        ) : (
          <CardBody className="space-y-4">
            <Field
              label={
                <span className="inline-flex items-center gap-1.5">
                  {t("oauth.selectProvider")}
                  <Tooltip content={t("oauth.requirements")} side="top">
                    <button
                      type="button"
                      aria-label={t("oauth.requirements")}
                      className="inline-flex h-5 w-5 items-center justify-center rounded text-text-muted transition hover:bg-surface-muted hover:text-text focus:outline-none focus:ring-2 focus:ring-primary/40"
                    >
                      <Info size={14} />
                    </button>
                  </Tooltip>
                </span>
              }
            >
              <Select
                value={providerId}
                onValueChange={(v) => {
                  setProviderId(v);
                  setAuthUrl(null);
                  setCallbackUrl("");
                  setMessage(null);
                  setError(null);
                }}
                ariaLabel={t("oauth.selectProvider")}
                placeholder={t("oauth.selectPlaceholder")}
                options={providerOptions}
              />
            </Field>

            <div className="flex flex-wrap gap-2">
              <Button
                variant="primary"
                icon={<Play size={16} />}
                disabled={!providerId}
                loading={startMutation.isPending}
                onClick={() => startMutation.mutate()}
              >
                {t("oauth.start")}
              </Button>
              <Button
                variant="secondary"
                icon={<RefreshCw size={16} />}
                disabled={!providerId}
                loading={refreshMutation.isPending}
                onClick={() => refreshMutation.mutate()}
              >
                {t("oauth.refresh")}
              </Button>
            </div>

            {error ? <ErrorBox message={error} /> : null}
            {message ? <Alert tone="success">{message}</Alert> : null}

            {authUrl ? (
              <Field label={t("oauth.authorizeUrl")}>
                <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
                  <code className="min-w-0 flex-1 break-all rounded-md bg-surface-muted px-3 py-2 font-mono text-xs text-text">
                    {authUrl}
                  </code>
                  <div className="flex gap-2">
                    <Button
                      variant="secondary"
                      icon={<Copy size={14} />}
                      onClick={copyUrl}
                    >
                      {t("oauth.copyUrl")}
                    </Button>
                    <a href={authUrl} target="_blank" rel="noreferrer">
                      <Button
                        variant="accent"
                        icon={<ExternalLink size={14} />}
                      >
                        {t("oauth.openUrl")}
                      </Button>
                    </a>
                  </div>
                </div>
              </Field>
            ) : null}

            {authUrl ? (
              <Field label={t("oauth.callbackHint")}>
                <div className="space-y-2">
                  <div className="flex items-start gap-2">
                    <textarea
                      className="min-h-[60px] min-w-0 flex-1 resize-y rounded-md border border-border bg-surface px-3 py-2 font-mono text-xs text-text placeholder:text-text-muted focus:outline-none focus:ring-2 focus:ring-primary/40"
                      placeholder={t("oauth.callbackUrlPlaceholder")}
                      value={callbackUrl}
                      onChange={(e) => setCallbackUrl(e.target.value)}
                    />
                    <Button
                      variant="secondary"
                      icon={<ClipboardPaste size={14} />}
                      onClick={pasteFromClipboard}
                    >
                      {t("oauth.pasteCallbackUrl")}
                    </Button>
                  </div>
                  <Button
                    variant="primary"
                    disabled={!callbackUrl.trim()}
                    loading={callbackMutation.isPending}
                    onClick={() => callbackMutation.mutate()}
                  >
                    {t("oauth.submitCallback")}
                  </Button>
                </div>
              </Field>
            ) : null}
          </CardBody>
        )}
      </Card>
    </div>
  );
}

/** Parse `code` and `state` from a pasted callback URL. Returns
 * `null` if either parameter is missing or the URL is malformed. */
function parseCallbackUrl(raw: string): { code: string; state: string } | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  try {
    const url = new URL(trimmed);
    const code = url.searchParams.get("code");
    const state = url.searchParams.get("state");
    if (!code || !state) return null;
    return { code, state };
  } catch {
    return null;
  }
}
