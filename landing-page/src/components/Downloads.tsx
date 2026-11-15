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
    </div>
  );
}
