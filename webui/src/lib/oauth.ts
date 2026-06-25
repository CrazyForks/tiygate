/** Parse `code` and `state` from a pasted callback URL. Returns
 * `null` if either parameter is missing or the URL is malformed. */
export function parseCallbackUrl(raw: string): {
  code: string;
  state: string;
} | null {
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
