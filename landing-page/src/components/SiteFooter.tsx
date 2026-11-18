import { PRODUCT } from "@/config";
import { storyVisible } from "@/lib/story";

export function SiteFooter() {
  return (
    <footer className="page foot">
      <span>This page has no analytics, no cookies, and asks no other site for anything.</span>
      <span className="foot__meta">
        {storyVisible() && <a href="/story/">Why Posthaste</a>}
        <a href={PRODUCT.repoUrl}>Source on GitHub</a>
        <span>Posthaste {PRODUCT.version}</span>
      </span>
    </footer>
  );
}
