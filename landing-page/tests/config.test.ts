import { existsSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  DEMO_VIDEO,
  FALLBACK_RELEASES,
  PRODUCT,
  RELEASE_NOTES,
  SYNC_VIDEO,
} from "../src/config";

// A cache-busting "?v=3" is part of the URL, not of the file name.
const pub = (url: string) => join(__dirname, "..", "public", url.replace(/\?.*$/, ""));

describe("product links and release version sync", () => {
  it("points the community link at a Discord invite over https", () => {
    expect(PRODUCT.discordUrl).toMatch(/^https:\/\/discord\.gg\/[A-Za-z0-9]+$/);
  });

  it("matches the workspace version in src-tauri/Cargo.toml across all release fields", () => {
    const cargoToml = readFileSync(join(__dirname, "..", "..", "src-tauri", "Cargo.toml"), "utf8");
    const match = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
    expect(match?.[1]).toBeDefined();
    const v = match![1]!;

    expect(PRODUCT.version).toBe(v);
    const base = `https://github.com/worty76/posthaste/releases/download/v${v}`;
    expect(PRODUCT.downloads.mac?.url).toBe(`${base}/Posthaste_${v}_aarch64.dmg`);
    expect(PRODUCT.downloads.windows?.url).toBe(`${base}/Posthaste_${v}_x64-setup.exe`);
    expect(PRODUCT.downloads.linux?.url).toBe(`${base}/Posthaste_${v}_amd64.AppImage`);
    expect(PRODUCT.linuxExtras.map((d) => d.url)).toEqual([
      `${base}/Posthaste_${v}_amd64.deb`,
      `${base}/Posthaste-${v}-1.x86_64.rpm`,
    ]);
    expect(RELEASE_NOTES[v]?.length).toBeGreaterThan(0);
    expect(FALLBACK_RELEASES[0]?.version).toBe(v);
  });
});

describe.each([
  ["demo video", DEMO_VIDEO],
  ["sync video", SYNC_VIDEO],
])("%s", (_name, film) => {
  it("uses same-origin files that exist", () => {
    for (const url of [film.src, film.poster]) {
      expect(url.startsWith("/")).toBe(true);
      expect(existsSync(pub(url))).toBe(true);
    }
  });

  it("stays light enough to serve from a small host", () => {
    expect(statSync(pub(film.src)).size).toBeLessThan(6 * 1024 * 1024);
    expect(statSync(pub(film.poster)).size).toBeLessThan(200 * 1024);
  });
});
