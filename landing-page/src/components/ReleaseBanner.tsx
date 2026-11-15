import type { Release } from "@/lib/releases";

/** One line above the header: the newest published version, linked to its
    notes. Read from GitHub at build time; nothing renders without one. */
export function ReleaseBanner({ release }: { release: Release | undefined }) {
  if (!release) return null;
  return (
    <p className="banner">
      <a href="#changelog">
        <span className="banner__tag">New</span>
        Posthaste {release.version} is out. <span className="banner__more">See what changed</span>
      </a>
    </p>
  );
}
