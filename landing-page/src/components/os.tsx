"use client";

import { useEffect, useState } from "react";
import type { Platform } from "@/config";

export type Os = Platform;

/** Best-effort OS detection from the user agent. Null until hydrated, so
    the server render never guesses. */
export function useOs(): Os | null {
  const [os, setOs] = useState<Os | null>(null);
  useEffect(() => {
    const ua = navigator.userAgent;
    if (/Mac|iPhone|iPad/i.test(ua)) setOs("mac");
    else if (/Win/i.test(ua)) setOs("windows");
    else setOs("linux");
  }, []);
  return os;
}

/** The app's default shortcuts (README, Shortcuts table). */
export const SHORTCUTS: Record<Os, { capture: string[]; today: string[]; inbox: string[] }> = {
  mac: { capture: ["⌘", "⇧", "Space"], today: ["⌘", "⌃", "G"], inbox: ["⌘", "⌃", "N"] },
  windows: {
    capture: ["Ctrl", "Shift", "Space"],
    today: ["Ctrl", "Alt", "G"],
    inbox: ["Ctrl", "Alt", "N"],
  },
  linux: {
    capture: ["Ctrl", "Shift", "Space"],
    today: ["Ctrl", "Alt", "G"],
    inbox: ["Ctrl", "Alt", "N"],
  },
};

export const OS_LABEL: Record<Os, string> = {
  mac: "macOS",
  windows: "Windows",
  linux: "Linux",
};

/** The app labels a note with the app you were in; here, that's the browser. */
export function useBrowserName(): string {
  const [name, setName] = useState("your browser");
  useEffect(() => {
    const ua = navigator.userAgent;
    const found = /Edg\//.test(ua)
      ? "Edge"
      : /OPR\//.test(ua)
        ? "Opera"
        : /Firefox\//.test(ua)
          ? "Firefox"
          : /Chrome\//.test(ua)
            ? "Chrome"
            : /Safari\//.test(ua)
              ? "Safari"
              : null;
    if (found) setName(found);
  }, []);
  return name;
}

/** A key combination as keycaps. */
export function Keys({ keys }: { keys: string[] }) {
  return (
    <span className="keys">
      {keys.map((k) => (
        <kbd key={k}>{k}</kbd>
      ))}
    </span>
  );
}

/** The shortcut for the visitor's system; macOS until we know. */
export function OsKeys({ which }: { which: "capture" | "today" | "inbox" }) {
  const os = useOs() ?? "mac";
  return <Keys keys={SHORTCUTS[os][which]} />;
}
