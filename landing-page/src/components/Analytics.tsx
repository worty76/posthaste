"use client";

import { useEffect } from "react";
import { SKIP_FLAG, visitPayload } from "@/lib/analytics";

// Turned on together with the download counter (`NEXT_PUBLIC_DL_COUNTER=/dl`).
const ENABLED = Boolean(process.env.NEXT_PUBLIC_DL_COUNTER);

/** Sends one first-party aggregate visit ping (`/visit`), unless this browser
    carries the maintainer's opt-out flag. No cookies, no third-party requests.
    Renders nothing. */
export function Analytics() {
  useEffect(() => {
    let skipped = false;
    try {
      skipped = window.localStorage.getItem(SKIP_FLAG) === "1";
    } catch {
      // Storage blocked: the visit is counted like any other.
    }
    const body = visitPayload(ENABLED, skipped, window.location.search, document.referrer);
    if (!body) return;
    void fetch("/visit", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body,
      keepalive: true,
    }).catch(() => {});
  }, []);
  return null;
}
