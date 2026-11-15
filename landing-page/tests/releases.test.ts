import { describe, expect, it } from "vitest";
import { FALLBACK_RELEASES } from "../src/config";
import { cleanNotes, fetchReleases, parseReleases } from "../src/lib/releases";
import { formatIsoDate } from "../src/lib/dates";

const payload = [
  {
    tag_name: "v9.9.9",
    published_at: "2027-01-02T03:04:05Z",
    html_url: "https://x/9",
    body: "",
    draft: true,
  },
  {
    tag_name: "v9.9.8",
    published_at: "2027-01-01T00:00:00Z",
    html_url: "https://x/8",
    body: "",
    prerelease: true,
  },
  {
    tag_name: "v1.4.0",
    published_at: "2026-12-01T10:00:00Z",
    html_url: "https://github.com/worty76/Posthaste/releases/tag/v1.4.0",
    body: "## What's changed\n\n- feat: tags in the inbox (abc1234)\n- build: bump (def5678)\n- Merge pull request #3\n- fix(capture): keep the draft on crash\n\n**Full changelog**: x",
  },
];

describe("releases", () => {
  it("parses a GitHub releases payload, dropping drafts and prereleases", () => {
    const r = parseReleases(payload, 5);
    expect(r).toHaveLength(1);
    expect(r[0]).toMatchObject({ version: "1.4.0", date: "2026-12-01" });
  });

  it("turns commit subjects into plain sentences", () => {
    expect(cleanNotes(payload[2]?.body as string)).toEqual([
      "Tags in the inbox.",
      "Keep the draft on crash.",
    ]);
    expect(cleanNotes("- chore: bump\n- build: x")).toEqual(["Maintenance release."]);
  });

  it("prefers notes written by hand for a known version", () => {
    const r = parseReleases(
      [
        {
          tag_name: "v0.2.1",
          published_at: "2026-11-14T17:28:45Z",
          html_url: "https://x",
          body: "- fix: no more blank",
        },
      ],
      1,
    );
    expect(r[0]?.notes[0]).toMatch(/no longer shows a blank window/);
  });

  it("falls back to the committed list when the request fails", async () => {
    const failing = (() => Promise.reject(new Error("offline"))) as unknown as typeof fetch;
    expect(await fetchReleases(3, failing)).toEqual(FALLBACK_RELEASES.slice(0, 3));
    const notOk = (() =>
      Promise.resolve(new Response("", { status: 403 }))) as unknown as typeof fetch;
    expect(await fetchReleases(2, notOk)).toHaveLength(2);
  });

  it("keeps a fallback whose newest entry is the configured version, with notes", async () => {
    const { PRODUCT } = await import("../src/config");
    expect(FALLBACK_RELEASES[0]?.version).toBe(PRODUCT.version);
    for (const r of FALLBACK_RELEASES) expect(r.notes.length).toBeGreaterThan(0);
  });

  it("formats dates without a locale", () => {
    expect(formatIsoDate("2026-11-15T05:43:49Z")).toBe("15 Nov 2026");
  });
});
