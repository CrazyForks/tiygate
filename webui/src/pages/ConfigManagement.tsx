import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useMutation } from "@tanstack/react-query";
import { Download, Upload, FileJson, Info } from "lucide-react";
import { configApi } from "@/api/resources";
import type { ConfigExport, ImportReport } from "@/api/types";
import {
  Alert,
  Button,
  Card,
  CardBody,
  CardHeader,
  ErrorBox,
  Field,
  PasswordInput,
  useToast,
} from "@/components/ui";
import { PageHeader } from "@/components/PageHeader";

export default function ConfigManagement() {
  const { t } = useTranslation();
  const toast = useToast();

  const [masterKey, setMasterKey] = useState("");
  const [fileName, setFileName] = useState<string | null>(null);
  const [parsedBackup, setParsedBackup] = useState<ConfigExport | null>(null);
  const [parseError, setParseError] = useState<string | null>(null);
  const [restoreResult, setRestoreResult] = useState<ImportReport | null>(null);

  const exportMutation = useMutation({
    mutationFn: () => configApi.export(),
    onSuccess: (bundle) => {
      const json = JSON.stringify(bundle, null, 2);
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      const ts = new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-");
      a.download = `tiygate-backup-${ts}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      toast.success(t("backup.exportSuccess"));
    },
    onError: (e: Error) => {
      toast.error(e.message);
    },
  });

  const restoreMutation = useMutation({
    mutationFn: () => {
      if (!parsedBackup) {
        throw new Error(t("backup.noFile"));
      }
      return configApi.import(masterKey, parsedBackup);
    },
    onSuccess: (report) => {
      setRestoreResult(report);
      toast.success(t("backup.importSuccess"));
      // Reset the form so the operator cannot accidentally re-import
      // the same file.
      setParsedBackup(null);
      setFileName(null);
      setMasterKey("");
    },
    onError: (e: Error) => {
      toast.error(e.message);
    },
  });

  function handleFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) {
      setParsedBackup(null);
      setFileName(null);
      setParseError(null);
      return;
    }
    setFileName(file.name);
    setParseError(null);
    setRestoreResult(null);
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const text = String(reader.result);
        const parsed = JSON.parse(text) as ConfigExport;
        if (
          typeof parsed.schema_version !== "number" ||
          !Array.isArray(parsed.providers) ||
          !Array.isArray(parsed.routes) ||
          !Array.isArray(parsed.api_keys)
        ) {
          setParseError(t("backup.invalidFormat"));
          setParsedBackup(null);
          return;
        }
        setParsedBackup(parsed);
      } catch {
        setParseError(t("backup.invalidFormat"));
        setParsedBackup(null);
      }
    };
    reader.onerror = () => {
      setParseError(t("backup.invalidFormat"));
      setParsedBackup(null);
    };
    reader.readAsText(file);
  }

  return (
    <div>
      <PageHeader
        title={t("backup.title")}
        description={t("backup.subtitle")}
      />

      <div className="grid gap-5 lg:grid-cols-2">
        {/* Export */}
        <Card>
          <CardHeader title={t("backup.exportTitle")} />
          <CardBody className="space-y-4">
            <p className="text-sm text-text-muted">
              {t("backup.exportDesc")}
            </p>
            <Alert tone="info">
              <div className="flex items-start gap-2">
                <Info size={16} className="mt-0.5 shrink-0" />
                <span>{t("backup.exportNote")}</span>
              </div>
            </Alert>
            <Button
              variant="primary"
              onClick={() => exportMutation.mutate()}
              disabled={exportMutation.isPending}
            >
              <Download size={16} />
              {exportMutation.isPending
                ? t("common.loading")
                : t("backup.exportBtn")}
            </Button>
          </CardBody>
        </Card>

        {/* Restore */}
        <Card>
          <CardHeader title={t("backup.importTitle")} />
          <CardBody className="space-y-4">
            <p className="text-sm text-text-muted">
              {t("backup.importDesc")}
            </p>

            <Field label={t("backup.selectFile")}>
              <label className="flex cursor-pointer items-center gap-2 rounded-sm border border-dashed border-border-strong bg-surface px-3 py-2 text-sm text-text-muted transition-colors hover:border-primary hover:text-text">
                <FileJson size={16} />
                <span className="truncate">
                  {fileName ?? t("backup.noFileSelected")}
                </span>
                <input
                  type="file"
                  accept="application/json,.json"
                  className="hidden"
                  onChange={handleFileChange}
                />
              </label>
            </Field>

            {parseError && <ErrorBox message={parseError} />}

            {parsedBackup && (
              <div className="rounded-sm bg-surface-muted px-3 py-2 text-xs text-text-muted">
                {t("backup.fileSummary", {
                  providers: parsedBackup.providers.length,
                  routes: parsedBackup.routes.length,
                  apiKeys: parsedBackup.api_keys.length,
                  encrypted: parsedBackup.encrypted
                    ? t("common.yes")
                    : t("common.no"),
                })}
              </div>
            )}

            <Field
              label={t("backup.masterKey")}
              hint={t("backup.masterKeyHint")}
            >
              <PasswordInput
                value={masterKey}
                onChange={(e) => setMasterKey(e.target.value)}
                placeholder="TIYGATE_MASTER_KEY"
                toggleLabel={t("backup.masterKey")}
                autoComplete="off"
              />
            </Field>

            <Button
              variant="primary"
              onClick={() => restoreMutation.mutate()}
              disabled={
                restoreMutation.isPending ||
                !parsedBackup ||
                (parsedBackup?.encrypted && !masterKey)
              }
            >
              <Upload size={16} />
              {restoreMutation.isPending
                ? t("common.loading")
                : t("backup.importBtn")}
            </Button>

            {restoreResult && (
              <div className="space-y-2 rounded-sm border border-border bg-surface-muted px-3 py-2">
                <p className="text-sm font-medium text-text">
                  {t("backup.importResultTitle")}
                </p>
                <ul className="space-y-1 text-xs text-text-muted">
                  <li>
                    {t("backup.providersResult", {
                      imported: restoreResult.providers_imported,
                      skipped: restoreResult.providers_skipped,
                    })}
                  </li>
                  <li>
                    {t("backup.routesResult", {
                      imported: restoreResult.routes_imported,
                      skipped: restoreResult.routes_skipped,
                    })}
                  </li>
                  <li>
                    {t("backup.apiKeysResult", {
                      imported: restoreResult.api_keys_imported,
                      skipped: restoreResult.api_keys_skipped,
                    })}
                  </li>
                </ul>
              </div>
            )}
          </CardBody>
        </Card>
      </div>
    </div>
  );
}
