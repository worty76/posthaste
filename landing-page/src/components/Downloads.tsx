"use client";

// One big button for the visitor's system, the others as rows. Without
// JavaScript (or before hydration) all three are rows, none guessed.

import { useState } from "react";
import { useOs, OS_LABEL, type Os } from "./os";
import type { Download } from "@/config";

const ORDER: Os[] = ["mac", "windows", "linux"];
const MAC_FIX_CMD = "xattr -cr /Applications/Posthaste.app";

export function Downloads({
  downloads,
  linuxExtras,
  releasesUrl,
}: {
  downloads: Record<Os, Download | null>;
  linuxExtras: Download[];
  releasesUrl: string;
}) {
  const os = useOs();
  const [copied, setCopied] = useState(false);
  const main = os ? downloads[os] : null;
  const rest = ORDER.filter((p) => !(main && p === os));

  const copyMacCmd = async () => {
    try {
      await navigator.clipboard.writeText(MAC_FIX_CMD);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard unavailable; command stays selectable.
    }
  };

  return (
    <div className="downloads">
      {main && os && (
        <a className="btn btn--big" href={main.url}>
          Download for {OS_LABEL[os]}
          <span className="btn__sub">{main.label}</span>
        </a>
      )}
      {main && os === "mac" && (
        <aside className="downloads__notice" aria-label="macOS first-open note">
          <div className="downloads__notice-head">
            <span className="downloads__notice-tag">First open on macOS</span>
          </div>
          <p className="downloads__notice-body">
            Posthaste is an unsigned indie build, so macOS may say it can&apos;t be verified or is
            damaged. After dragging it to <strong>Applications</strong>, run this once in Terminal
            (or allow it in <strong>System Settings → Privacy &amp; Security</strong>):
          </p>
          <div className="downloads__cmd">
            <code>{MAC_FIX_CMD}</code>
            <button type="button" className="downloads__copy" onClick={copyMacCmd}>
              {copied ? "Copied" : "Copy"}
            </button>
          </div>
        </aside>
      )}
      {main && os === "windows" && (
        <aside className="downloads__notice" aria-label="Windows first-open note">
          <div className="downloads__notice-head">
            <span className="downloads__notice-tag">First open on Windows</span>
          </div>
          <p className="downloads__notice-body">
            Windows SmartScreen may warn that Posthaste is unrecognized because it is a new indie
            release. Click <strong>More info</strong>, then <strong>Run anyway</strong> to install.
          </p>
        </aside>
      )}
      {main && os === "linux" && (
        <aside className="downloads__notice" aria-label="Linux first-open note">
          <div className="downloads__notice-head">
            <span className="downloads__notice-tag">First open on Linux</span>
          </div>
          <p className="downloads__notice-body">
            For the <code>.AppImage</code>, mark it executable with{" "}
            <code>chmod +x Posthaste_*.AppImage</code> before running — or install the{" "}
            <code>.deb</code> / <code>.rpm</code> package directly.
          </p>
        </aside>
      )}
      {main && os === "linux" && linuxExtras.length > 0 && (
        <p className="downloads__also">
          Also as{" "}
          {linuxExtras.map((x, i) => (
            <span key={x.url}>
              {i > 0 && " or "}
              <a href={x.url}>{x.label}</a>
            </span>
          ))}
          .
        </p>
      )}
      <ul className="downloads__rows">
        {rest.map((p) => {
          const d = downloads[p];
          return (
            <li className="downloads__row" key={p}>
              <span className="downloads__os">{OS_LABEL[p]}</span>
              {d ? (
                <a href={d.url}>{d.label}</a>
              ) : (
                <a href={releasesUrl}>Not in this release yet</a>
              )}
              {p === "linux" &&
                linuxExtras.map((x) => (
                  <a key={x.url} href={x.url}>
                    {x.label}
                  </a>
                ))}
            </li>
          );
        })}
      </ul>
      <p className="downloads__hint">
        Free Windows code signing provided by{" "}
        <a href="https://signpath.io" rel="noreferrer">
          SignPath.io
        </a>
        , certificate by{" "}
        <a href="https://signpath.org" rel="noreferrer">
          SignPath Foundation
        </a>
        .
      </p>
    </div>
  );
}
