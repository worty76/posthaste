"use client";

// The hero's two actions: the installer for this computer (or a jump to the
// full list when we can't tell), and the demo film.

import { useOs, OS_LABEL } from "./os";
import { DEMO_VIDEO, type Download, type Platform } from "@/config";
import { primaryDownload } from "@/lib/platform";

export function HeroActions({ downloads }: { downloads: Record<Platform, Download | null> }) {
  const os = useOs();
  const main = primaryDownload(downloads, os);
  return (
    <div className="hero__actions">
      {main && os ? (
        <a className="btn btn--cta" href={main.url}>
          Download for {OS_LABEL[os]}
        </a>
      ) : (
        <a className="btn btn--cta" href="#get">
          Download free
        </a>
      )}
      <a className="btn btn--ghost" href="#demo">
        Watch the demo, {DEMO_VIDEO.seconds} s
      </a>
    </div>
  );
}
