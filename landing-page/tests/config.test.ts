import { describe, expect, it } from "vitest";
import { PRODUCT } from "../src/config";

describe("product links", () => {
  it("points the community link at a Discord invite over https", () => {
    expect(PRODUCT.discordUrl).toMatch(/^https:\/\/discord\.gg\/[A-Za-z0-9]+$/);
  });
});
