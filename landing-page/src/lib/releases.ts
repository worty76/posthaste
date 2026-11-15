// The changelog and the release banner, read from GitHub at build time.
// When the API can't be reached the committed list in config.ts stays, so
// the page always builds. Notes written by hand in config.ts win over the
// automatic release body, which is a list of commit subjects.

import { FALLBACK_RELEASES, RELEASE_NOTES } from "@/config";

export interface Release {
  version: string;
  /** ISO date of publication. */
  date: string;
  url: string;
  notes: string[];
}

interface GitHubRelease {
  tag_name?: unknown;
  published_at?: unknown;
  html_url?: unknown;
  body?: unknown;
  draft?: unknown;
  prerelease?: unknown;
}

const SKIP = /^(build|chore|ci|docs|test|style|refactor)(\([^)]*\))?!?:|^merge\b|full changelog/i;

/** Commit-subject bullets into plain sentences a visitor can read. */
export function cleanNotes(body: string): string[] {
  const out: string[] = [];
  for (const raw of body.split("\n")) {
    const m = raw.match(/^\s*[-*]\s+(.*)$/);
    if (!m) continue;
    let line = (m?.[1] ?? "").trim();
    if (!line || SKIP.test(line)) continue;
    line = line
      .replace(/^(feat|fix|perf)(\([^)]*\))?!?:\s*/i, "")
      .replace(/\s*\(([0-9a-f]{7,40})\)\s*$/i, "")
      .trim();
    if (!line) continue;
    line = line.charAt(0).toUpperCase() + line.slice(1);
    if (!/[.!?]$/.test(line)) line += ".";
    out.push(line);
  }
  return out.length > 0 ? out : ["Maintenance release."];
}

export function parseReleases(payload: unknown, limit: number): Release[] {
  if (!Array.isArray(payload)) return [];
  const out: Release[] = [];
  for (const r of payload as GitHubRelease[]) {
    if (r.draft === true || r.prerelease === true) continue;
    if (typeof r.tag_name !== "string" || typeof r.published_at !== "string") continue;
    if (typeof r.html_url !== "string") continue;
    const version = r.tag_name.replace(/^v/, "");
    out.push({
      version,
      date: r.published_at.slice(0, 10),
      url: r.html_url,
      notes: RELEASE_NOTES[version] ?? cleanNotes(typeof r.body === "string" ? r.body : ""),
    });
    if (out.length >= limit) break;
  }
  return out;
}

export async function fetchReleases(
  limit: number,
  fetchImpl: typeof fetch = fetch,
): Promise<Release[]> {
  const fallback = FALLBACK_RELEASES.slice(0, limit);
  try {
    const res = await fetchImpl(
      `https://api.github.com/repos/worty76/Posthaste/releases?per_page=${limit + 5}`,
      { cache: "no-store", headers: { Accept: "application/vnd.github+json" } },
    );
    if (!res.ok) return fallback;
    const found = parseReleases(await res.json(), limit);
    return found.length > 0 ? found : fallback;
  } catch {
    return fallback;
  }
}
