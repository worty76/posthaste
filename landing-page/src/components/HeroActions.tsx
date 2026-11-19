"use client";

import { useOs, OS_LABEL } from "./os";
import { useSite } from "./SiteState";
import { type Download, type Platform } from "@/config";
import { primaryDownload } from "@/lib/platform";

export function HeroActions({ downloads }: { downloads: Record<Platform, Download | null> }) {
  const os = useOs();
  const { copy } = useSite();
  const main = primaryDownload(downloads, os);
  return (
    <div className="hero__actions">
      {main && os ? (
        <a className="btn btn--cta" href={main.url}>
          {copy.hero.downloadFor(OS_LABEL[os])}
        </a>
      ) : (
        <a className="btn btn--cta" href="#get">
          {copy.hero.downloadFree}
        </a>
      )}
      <a className="btn btn--ghost" href="#demo">
        {copy.hero.watchDemo}
      </a>
    </div>
  );
}
