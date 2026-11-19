import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { eventually, open, start, stop } from "./harness.mjs";

before(start);
after(stop);

const resolved = (id, base, tokens) => ({
  id,
  name: id,
  author: null,
  base,
  shape: "soft",
  font: "system",
  tokens,
});

/** The fake shell answers `look_active` with the look the settings name. */
const looks = {
  "builtin:ink": resolved("builtin:ink", "dark", { "--bg": "#101010" }),
  "builtin:strawberry": resolved("builtin:strawberry", "light", { "--bg": "#fff0f4" }),
  // A look with a token set that carries no custom font: switching to it
  // must leave nothing of the previous look behind.
  "builtin:matcha": resolved("builtin:matcha", "light", {
    "--bg": "#e8f5e9",
    "--accent": "#2e7d32",
  }),
};

async function root(page) {
  return page.evaluate(() => ({
    theme: document.documentElement.dataset.theme,
    bg: document.documentElement.style.getPropertyValue("--bg"),
  }));
}

describe("appearance", () => {
  it("main: a custom font token is drained by a look without one", async () => {
    const withFont = resolved("builtin:ink", "dark", {
      "--bg": "#101010",
      "--font": '"JetBrains Mono", var(--font-mono-ui)',
    });
    const w = await open({ label: "main", settings: { look_id: "builtin:ink" } });
    await w.page.evaluate(
      ({ looks }) => {
        window.__mock.handlers.look_active = () => looks[window.__mock.settings.look_id] ?? null;
      },
      { looks: { ...looks, "builtin:ink": withFont } },
    );
    await w.mock.changeSettings({ look_id: "builtin:strawberry" });
    await w.mock.changeSettings({ look_id: "builtin:ink" });
    await eventually(
      () => w.page.evaluate(() => document.documentElement.style.getPropertyValue("--font")),
      (v) => v.includes("JetBrains Mono"),
      "the look's font to be worn",
    );

    await w.mock.changeSettings({ look_id: "builtin:matcha" });
    await eventually(
      () => w.page.evaluate(() => document.documentElement.style.getPropertyValue("--font")),
      (v) => v === "",
      "a look without a font to drain it",
    );
    await eventually(
      () => w.page.evaluate(() => document.documentElement.style.getPropertyValue("--bg")),
      (v) => v === "#e8f5e9",
      "the next look's colours to be worn",
    );
    assert.deepEqual(w.errors, []);
    await w.close();
  });

  for (const label of ["capture", "main", "today", "details"]) {
    it(`${label}: wears a newly chosen look without a reload`, async () => {
      const w = await open({ label, settings: { look_id: "builtin:ink" } });
      await w.page.evaluate((looks) => {
        window.__mock.handlers.look_active = () => looks[window.__mock.settings.look_id] ?? null;
      }, looks);
      // Settings may arrive before the handler above, and re-asking with
      // the same id is no change at all: step away and back.
      await w.mock.changeSettings({ look_id: "builtin:strawberry" });
      await w.mock.changeSettings({ look_id: "builtin:ink" });
      await eventually(
        () => root(w.page),
        (r) => r.bg === "#101010" && r.theme === "dark",
        "Ink to be worn",
      );

      await w.mock.changeSettings({ look_id: "builtin:strawberry" });
      await eventually(
        () => root(w.page),
        (r) => r.bg === "#fff0f4" && r.theme === "light",
        "Strawberry to be worn",
      );

      await w.mock.changeSettings({ look_id: "builtin:matcha" });
      await eventually(
        () => root(w.page),
        (r) => r.bg === "#e8f5e9" && r.theme === "light",
        "Matcha to be worn",
      );
      assert.deepEqual(w.errors, []);
      await w.close();
    });
  }
});
