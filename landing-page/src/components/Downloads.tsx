"use client";

// One big button for the visitor's system, the others as rows. Without
// JavaScript (or before hydration) all three are rows, none guessed.

import { useOs, OS_LABEL, type Os } from "./os";
import type { Download } from "@/config";

const ORDER: Os[] = ["mac", "windows", "linux"];

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
  const main = os ? downloads[os] : null;
  const rest = ORDER.filter((p) => !(main && p === os));

  return (
    <div className="downloads">
      {main && os && (
        <a className="btn btn--big" href={main.url}>
          Download for {OS_LABEL[os]}
          <span className="btn__sub">{main.label}</span>
        </a>
      )}
      {main && os === "mac" && (
        <p className="downloads__hint">
          Unsigned build. If macOS blocks it on first open, run{" "}
          <code>xattr -cr /Applications/Posthaste.app</code> in Terminal or allow it in{" "}
          <strong>System Settings → Privacy &amp; Security</strong>.
        </p>
      )}
      {main && os === "windows" && (
        <p className="downloads__hint">
          If Windows SmartScreen warns you, click <strong>More info → Run anyway</strong>.
        </p>
      )}
      {main && os === "linux" && (
        <p className="downloads__hint">
          For the <code>.AppImage</code>, mark it executable (
          <code>chmod +x Posthaste_*.AppImage</code>) before running.
        </p>
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
