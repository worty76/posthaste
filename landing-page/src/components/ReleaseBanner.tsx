"use client";

import { useSite } from "./SiteState";
import type { Release } from "@/lib/releases";

/** One line above the header: the newest published version, linked to its
    notes. Read from GitHub at build time; nothing renders without one. */
export function ReleaseBanner({ release }: { release: Release | undefined }) {
  const { lang } = useSite();
  if (!release) return null;
  const vi = lang === "vi";
  return (
    <p className="banner">
      <a href="#changelog">
        <span className="banner__tag">{vi ? "Mới" : "New"}</span>
        {vi ? `Đã có Posthaste ${release.version}.` : `Posthaste ${release.version} is out.`}{" "}
        <span className="banner__more">{vi ? "Xem điểm mới" : "See what changed"}</span>
      </a>
    </p>
  );
}
