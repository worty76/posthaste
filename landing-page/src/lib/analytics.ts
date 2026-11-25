import { SKIP_FLAG, cleanRef } from "./tracking";

export { SKIP_FLAG };

/**
 * The payload sent to our own `/visit` endpoint, or null when counting is off
 * or this browser carries the maintainer's opt-out flag.
 */
export function visitPayload(
  enabled: boolean,
  skipped: boolean,
  search: string,
  referrer: string,
): string | null {
  if (!enabled || skipped) return null;
  const ref = cleanRef(new URLSearchParams(search).get("ref"));
  return JSON.stringify({
    ref: ref === "direct" ? null : ref,
    referrer: referrer || null,
  });
}
