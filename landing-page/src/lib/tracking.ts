// The download and visit counter's decisions as plain functions: what may be
// counted, what is stored, and what the daily report says. Only aggregates are
// ever kept (day, file kind, version, country, source tag, and count).

export const INSTALLER_PREFIX = "https://github.com/worty76/posthaste/releases/download/";
const INSTALLER_PATH = "/worty76/posthaste/releases/download/";

export type Kind = "dmg" | "exe" | "appimage" | "deb" | "rpm";

export interface Target {
  /** Where the visitor is sent: a published release asset, nothing else. */
  url: string;
  kind: Kind;
  version: string;
}

const KINDS: Record<string, Kind> = {
  dmg: "dmg",
  exe: "exe",
  appimage: "appimage",
  deb: "deb",
  rpm: "rpm",
};

/** The destination, if it is exactly one of our release installers. Anything
    else (another host, another repository, a path trick, a query) is refused,
    so the counter can never be used as an open redirect. */
export function parseTarget(raw: string | null): Target | null {
  if (!raw) return null;
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" || url.host !== "github.com") return null;
  if (url.search || url.hash || url.username || url.password) return null;
  if (!url.pathname.toLowerCase().startsWith(INSTALLER_PATH)) return null;
  const rest = url.pathname.slice(INSTALLER_PATH.length).split("/");
  if (rest.length !== 2) return null;
  const [tag = "", file = ""] = rest;
  const version = /^v(\d+\.\d+\.\d+)$/.exec(tag)?.[1];
  const extension = /\.([A-Za-z]+)$/.exec(file)?.[1]?.toLowerCase();
  const kind = extension ? KINDS[extension] : undefined;
  if (!version || !kind || !/^[A-Za-z0-9._-]+$/.test(file)) return null;
  return { url: url.href, kind, version };
}

export function platformOf(kind: string): "macOS" | "Windows" | "Linux" {
  if (kind === "dmg") return "macOS";
  if (kind === "exe") return "Windows";
  return "Linux";
}

/** A campaign tag from `?ref=` on the visitor's own page address, such as a
    link shared on a forum. Anything unusual counts as "direct". */
export function cleanRef(raw: string | null | undefined): string {
  const value = (raw ?? "").trim().toLowerCase();
  return /^[a-z0-9_-]{1,24}$/.test(value) ? value : "direct";
}

export function cleanCountry(raw: string | null | undefined): string {
  const value = (raw ?? "").trim().toUpperCase();
  return /^[A-Z]{2}$/.test(value) ? value : "??";
}

const BOT =
  /bot|crawl|spider|slurp|curl|wget|python|go-http|java\/|headless|preview|facebookexternalhit/i;

export function isBot(userAgent: string | null): boolean {
  return !userAgent || BOT.test(userAgent);
}

/** The maintainer's own browsers carry this cookie (and the same flag in
    localStorage, which the site reads to skip `/visit`), so their test
    visits and downloads are not counted. Only ever set on request at
    `/dl/skip`, never for a visitor. */
export const SKIP_FLAG = "ph_skip";

export function hasSkipCookie(cookieHeader: string | null): boolean {
  return /(?:^|;\s*)ph_skip=1(?:;|$)/.test(cookieHeader ?? "");
}

export interface Visit {
  method: string;
  userAgent: string | null;
  purpose: string | null;
  cookie: string | null;
}

/** Only a real person's navigation counts: not a HEAD probe, not a browser
    prefetch, not a crawler, not the maintainer. */
export function shouldCount(visit: Visit): boolean {
  if (visit.method !== "GET") return false;
  if (/prefetch|preview/i.test(visit.purpose ?? "")) return false;
  if (isBot(visit.userAgent)) return false;
  return !hasSkipCookie(visit.cookie);
}

export function dayOf(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10);
}

export function sameSecret(given: string | null, secret: string | undefined): boolean {
  if (!secret || given === null || given.length !== secret.length) return false;
  let diff = 0;
  for (let i = 0; i < secret.length; i++) diff |= given.charCodeAt(i) ^ secret.charCodeAt(i);
  return diff === 0;
}

export interface DayReport {
  day: string;
  byKind: { kind: string; n: number }[];
  byCountry: { country: string; n: number }[];
  byRef: { ref: string; n: number }[];
  dayTotal: number;
  allTime: number;
}

function top(pairs: [string, number][], limit = 5): string {
  return pairs
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, limit)
    .map(([name, n]) => `${name} ${n}`)
    .join(" · ");
}

/** The daily Discord text. */
export function buildReport(report: DayReport): string {
  if (report.dayTotal === 0) {
    return `No downloads on ${report.day} (UTC). **${report.allTime}** counted in total.`;
  }
  const platforms = new Map<string, number>();
  for (const row of report.byKind) {
    const name = platformOf(row.kind);
    platforms.set(name, (platforms.get(name) ?? 0) + row.n);
  }
  return [
    `**${report.dayTotal}** downloads on ${report.day} (UTC).`,
    `Platforms: ${top([...platforms])}`,
    `Countries: ${top(report.byCountry.map((r) => [r.country, r.n]))}`,
    `Sources: ${top(report.byRef.map((r) => [r.ref, r.n]))}`,
    `**${report.allTime}** counted in total.`,
  ].join("\n");
}

/**
 * The address a download button points at. With the counter on, release
 * installers go through our own `/dl`, which counts an aggregate and sends the
 * visitor on to GitHub. Any other link, and every link while the counter is
 * off, is returned as it is.
 */
export function trackedUrl(
  url: string,
  counterPath: string | undefined,
  ref: string | null,
): string {
  if (!counterPath || !url.toLowerCase().startsWith(INSTALLER_PREFIX)) return url;
  const params = new URLSearchParams({ to: url });
  const tag = cleanRef(ref);
  if (tag !== "direct") params.set("ref", tag);
  return `${counterPath}?${params}`;
}
