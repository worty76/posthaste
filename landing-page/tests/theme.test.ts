import { describe, expect, it } from "vitest";
import { LOOKS } from "@/lib/looks";
import { lookVariables, themeCss } from "@/lib/theme";
import { contrast, parseHex } from "@/lib/contrast";

const ratio = (a: string, b: string) => contrast(parseHex(a)!, parseHex(b)!);

describe("look switching", () => {
  it("emits one block per look, keyed by data-look", () => {
    const css = themeCss();
    for (const look of LOOKS) expect(css).toContain(`[data-look="${look.id}"]{`);
  });

  it("defaults to Strawberry Milk, and to Midnight Grape when the system is dark", () => {
    const css = themeCss();
    expect(css.startsWith(":root{--bg:#fff5f7")).toBe(true);
    expect(css).toContain(
      "@media (prefers-color-scheme: dark){:root:not([data-look]){--bg:#1a1423",
    );
  });

  it("sets every variable the stylesheet uses, for every look", () => {
    const names = Object.keys(lookVariables(LOOKS[0]!)).sort();
    for (const look of LOOKS) expect(Object.keys(lookVariables(look)).sort()).toEqual(names);
  });

  it("keeps text, secondary text and accent text readable in all six looks", () => {
    for (const look of LOOKS) {
      const v = lookVariables(look);
      for (const fg of [v["--text"]!, v["--muted"]!, v["--accent-ink"]!]) {
        expect(ratio(fg, v["--bg"]!)).toBeGreaterThanOrEqual(4.5);
        expect(ratio(fg, v["--surface"]!)).toBeGreaterThanOrEqual(4.5);
      }
      expect(ratio(v["--on-accent"]!, v["--accent"]!)).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("maps the app's font modes to Recursive's axes", () => {
    const byId = Object.fromEntries(LOOKS.map((l) => [l.id, lookVariables(l)]));
    expect(byId["strawberry-milk"]!["--casl"]).toBe("1");
    expect(byId["ink"]!["--mono"]).toBe("1");
    expect(byId["blueberry"]!["--casl"]).toBe("0");
  });
});
