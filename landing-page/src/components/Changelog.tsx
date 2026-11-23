"use client";

import { useSite } from "./SiteState";
import { PRODUCT } from "@/config";
import { formatIsoDate } from "@/lib/dates";
import type { Release } from "@/lib/releases";

/** The last few releases, from GitHub at build time (config.ts if offline). */
export function Changelog({ releases }: { releases: Release[] }) {
  const { lang } = useSite();
  const vi = lang === "vi";
  if (releases.length === 0) return null;
  return (
    <section className="page changelog" id="changelog" aria-labelledby="changelog-title">
      <h2 className="h2" id="changelog-title">
        {vi ? "Cập nhật gần đây." : "What changed lately."}
      </h2>
      <ol className="changelog__list">
        {releases.map((r) => (
          <li key={r.version} className="changelog__item">
            <p className="changelog__head">
              <a href={r.url}>{r.version}</a>
              <time dateTime={r.date}>{formatIsoDate(r.date)}</time>
            </p>
            <ul className="changelog__notes">
              {r.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          </li>
        ))}
      </ol>
      <p className="quiet">
        <a href={PRODUCT.releasesUrl}>
          {vi ? "Tất cả phiên bản và bộ cài đặt" : "Every release, with installers"}
        </a>
      </p>
    </section>
  );
}
