import { existsSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { DEMO_VIDEO, PRODUCT } from "../src/config";

// A cache-busting "?v=3" is part of the URL, not of the file name.
const pub = (url: string) => join(__dirname, "..", "public", url.replace(/\?.*$/, ""));

describe("product links", () => {
  it("points the community link at a Discord invite over https", () => {
    expect(PRODUCT.discordUrl).toMatch(/^https:\/\/discord\.gg\/[A-Za-z0-9]+$/);
  });
});

describe("demo video", () => {
  it("uses same-origin files that exist", () => {
    for (const url of [DEMO_VIDEO.src, DEMO_VIDEO.poster]) {
      expect(url.startsWith("/")).toBe(true);
      expect(existsSync(pub(url))).toBe(true);
    }
  });

  it("stays light enough to serve from a small host", () => {
    expect(statSync(pub(DEMO_VIDEO.src)).size).toBeLessThan(6 * 1024 * 1024);
    expect(statSync(pub(DEMO_VIDEO.poster)).size).toBeLessThan(200 * 1024);
  });
});
