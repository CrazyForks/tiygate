import { isTauri, tauriOpenExternalUrl } from "@/auth/setup";

export async function openExternalUrl(url: string): Promise<boolean> {
  if (isTauri()) {
    return tauriOpenExternalUrl(url);
  }

  const opened = window.open(url, "_blank", "noopener,noreferrer");
  if (opened) return true;

  const link = document.createElement("a");
  link.href = url;
  link.target = "_blank";
  link.rel = "noopener noreferrer";
  document.body.appendChild(link);
  link.click();
  link.remove();
  return true;
}
