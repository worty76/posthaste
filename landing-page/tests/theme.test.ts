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

  it("defaults to Ink, whatever the system colour scheme", () => {
    const css = themeCss();
    expect(css.startsWith(":root{--bg:#0f0f0f")).toBe(true);
    expect(css.split("\n")[0]).toContain("--mono:1");
    expect(css).not.toContain("prefers-color-scheme");
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
    expect(byId["ink"]!["--mono"]).toBe("1");
    expect(byId["matcha"]!["--casl"]).toBe("0");
  });

  it("wears the bundled fonts the app ships, by name", () => {
    const byId = Object.fromEntries(LOOKS.map((l) => [l.id, l.font]));
    expect(byId["blueberry"]).toBe("be-vietnam-pro");
    expect(byId["strawberry-milk"]).toBe("quicksand");
    const vars = Object.fromEntries(LOOKS.map((l) => [l.id, lookVariables(l)]));
    expect(vars["blueberry"]!["--font"]).toContain('"Be Vietnam Pro"');
    expect(vars["strawberry-milk"]!["--font"]).toContain('"Quicksand"');
    expect(vars["strawberry-milk"]!["--font"]).toContain("sans-serif");
  });
});
