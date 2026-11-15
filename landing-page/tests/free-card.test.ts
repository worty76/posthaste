import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const src = readFileSync(join(__dirname, "../src/components/FreeCard.tsx"), "utf8");

describe("free card", () => {
  it("states MIT and links the repository, licence and community", () => {
    expect(src).toMatch(/MIT/);
    expect(src).toMatch(/PRODUCT\.repoUrl/);
    expect(src).toMatch(/LICENSE/);
    expect(src).toMatch(/PRODUCT\.discordUrl/);
  });

  it("names no price or currency", () => {
    expect(src).not.toMatch(/[$€£]\s?\d|\d+\s*(usd|eur|per month|\/mo)/i);
  });
});
