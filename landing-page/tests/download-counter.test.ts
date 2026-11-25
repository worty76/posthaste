import { describe, expect, it } from "vitest";
import {
  buildReport,
  cleanCountry,
  cleanRef,
  dayOf,
  hasSkipCookie,
  parseTarget,
  sameSecret,
  shouldCount,
  trackedUrl,
} from "../src/lib/tracking";
import { visitPayload } from "../src/lib/analytics";

const MAC =
  "https://github.com/worty76/posthaste/releases/download/v0.7.3/Posthaste_0.7.3_aarch64.dmg";
const BROWSER =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Safari/605.1.15";

describe("download target", () => {
  it("accepts each installer kind of a published release", () => {
    expect(parseTarget(MAC)).toEqual({ url: MAC, kind: "dmg", version: "0.7.3" });
    const base = "https://github.com/worty76/posthaste/releases/download/v0.7.3/";
    expect(parseTarget(`${base}Posthaste_0.7.3_x64-setup.exe`)?.kind).toBe("exe");
    expect(parseTarget(`${base}Posthaste_0.7.3_amd64.AppImage`)?.kind).toBe("appimage");
    expect(parseTarget(`${base}Posthaste_0.7.3_amd64.deb`)?.kind).toBe("deb");
    expect(parseTarget(`${base}Posthaste-0.7.3-1.x86_64.rpm`)?.kind).toBe("rpm");
    // The site's config spells the repository in mixed case.
    expect(parseTarget(MAC.replace("posthaste", "Posthaste"))?.kind).toBe("dmg");
  });

  it("refuses anything that is not one of our installers, so it is no open redirect", () => {
    for (const bad of [
      null,
      "",
      "not a url",
      "https://evil.example/Posthaste.dmg",
      "http://github.com/worty76/posthaste/releases/download/v0.7.3/a.dmg",
      "https://github.com/someone/else/releases/download/v0.7.3/a.dmg",
      "https://github.com/worty76/posthaste/releases/download/v0.7.3/a.zip",
      "https://github.com/worty76/posthaste/releases/download/latest/a.dmg",
      "https://github.com/worty76/posthaste/releases/download/v0.7.3/a.dmg?x=1",
      "https://github.com/worty76/posthaste/releases/download/v0.7.3/../../../x.dmg",
      "https://github.com/worty76/posthaste/releases/download/v0.7.3/%2e%2e/a.dmg",
      "https://github.com@evil.example/worty76/posthaste/releases/download/v0.7.3/a.dmg",
    ]) {
      expect(parseTarget(bad), String(bad)).toBeNull();
    }
  });
});

describe("what is stored and what is counted", () => {
  it("keeps only a short tag, a two-letter country and a UTC day", () => {
    expect(cleanRef("Reddit")).toBe("reddit");
    expect(cleanRef("hn_2026-11")).toBe("hn_2026-11");
    expect(cleanRef("a b")).toBe("direct");
    expect(cleanRef("x".repeat(25))).toBe("direct");
    expect(cleanRef(null)).toBe("direct");
    expect(cleanCountry("vn")).toBe("VN");
    expect(cleanCountry("Vietnam")).toBe("??");
    expect(cleanCountry(undefined)).toBe("??");
    expect(dayOf(Date.UTC(2026, 10, 25, 23, 59))).toBe("2026-11-25");
  });

  it("counts a person's click and nothing else", () => {
    const person = { method: "GET", userAgent: BROWSER, purpose: null, cookie: null };
    expect(shouldCount(person)).toBe(true);
    expect(shouldCount({ ...person, method: "HEAD" })).toBe(false);
    expect(shouldCount({ ...person, userAgent: "Googlebot/2.1" })).toBe(false);
    expect(shouldCount({ ...person, userAgent: "curl/8.7.1" })).toBe(false);
    expect(shouldCount({ ...person, userAgent: null })).toBe(false);
    expect(shouldCount({ ...person, purpose: "prefetch" })).toBe(false);
    expect(shouldCount({ ...person, cookie: "a=b; ph_skip=1" })).toBe(false);
    expect(hasSkipCookie("ph_skip=0")).toBe(false);
  });

  it("compares the skip key without accepting a wrong or missing one", () => {
    expect(sameSecret("abc", "abc")).toBe(true);
    expect(sameSecret("abd", "abc")).toBe(false);
    expect(sameSecret("ab", "abc")).toBe(false);
    expect(sameSecret(null, "abc")).toBe(false);
    expect(sameSecret("abc", undefined)).toBe(false);
  });
});

describe("daily report", () => {
  it("says what happened, grouped as people read it", () => {
    const text = buildReport({
      day: "2026-11-25",
      byKind: [
        { kind: "dmg", n: 3 },
        { kind: "exe", n: 2 },
        { kind: "deb", n: 1 },
        { kind: "rpm", n: 1 },
      ],
      byCountry: [
        { country: "VN", n: 4 },
        { country: "US", n: 3 },
      ],
      byRef: [
        { ref: "direct", n: 5 },
        { ref: "reddit", n: 2 },
      ],
      dayTotal: 7,
      allTime: 120,
    });
    expect(text).toContain("**7** downloads on 2026-11-25 (UTC).");
    // Ties are listed alphabetically, so the report is stable.
    expect(text).toContain("Platforms: macOS 3 · Linux 2 · Windows 2");
    expect(text).toContain("Countries: VN 4 · US 3");
    expect(text).toContain("Sources: direct 5 · reddit 2");
    expect(text).toContain("**120** counted in total.");
  });

  it("says plainly when a day had none", () => {
    expect(
      buildReport({
        day: "2026-11-25",
        byKind: [],
        byCountry: [],
        byRef: [],
        dayTotal: 0,
        allTime: 9,
      }),
    ).toBe("No downloads on 2026-11-25 (UTC). **9** counted in total.");
  });
});

describe("page visit beacon", () => {
  it("sends a first-party visit payload only when enabled and not exempt", () => {
    expect(visitPayload(true, false, "?ref=Reddit", "https://news.ycombinator.com/")).toBe(
      JSON.stringify({ ref: "reddit", referrer: "https://news.ycombinator.com/" }),
    );
    expect(visitPayload(true, false, "", "")).toBe(JSON.stringify({ ref: null, referrer: null }));
    expect(visitPayload(false, false, "?ref=reddit", "")).toBeNull();
    expect(visitPayload(true, true, "?ref=reddit", "")).toBeNull();
  });
});

describe("download links on the site", () => {
  it("leaves links alone while the counter is off", () => {
    expect(trackedUrl(MAC, undefined, null)).toBe(MAC);
  });

  it("routes installers through the counter and keeps a clean campaign tag", () => {
    expect(trackedUrl(MAC, "/dl", null)).toBe(`/dl?to=${encodeURIComponent(MAC)}`);
    expect(trackedUrl(MAC, "/dl", "Reddit")).toBe(`/dl?to=${encodeURIComponent(MAC)}&ref=reddit`);
    expect(trackedUrl(MAC, "/dl", "<script>")).toBe(`/dl?to=${encodeURIComponent(MAC)}`);
  });

  it("never rewrites a link that is not an installer", () => {
    const other = "https://github.com/worty76/Posthaste/releases/latest";
    expect(trackedUrl(other, "/dl", null)).toBe(other);
  });
});
