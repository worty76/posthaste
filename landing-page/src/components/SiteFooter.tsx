"use client";

import { PRODUCT } from "@/config";
import { storyVisible } from "@/lib/story";
import { useCopy } from "./SiteState";

export function SiteFooter() {
  const copy = useCopy();
  return (
    <footer className="page foot">
      <span>{copy.footer.privacy}</span>
      <span className="foot__meta">
        {storyVisible() && <a href="/story/">{copy.footer.why}</a>}
        <a href={PRODUCT.repoUrl}>{copy.footer.source}</a>
        <span>Posthaste {PRODUCT.version}</span>
      </span>
    </footer>
  );
}
