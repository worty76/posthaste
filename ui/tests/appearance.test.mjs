import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { eventually, open, start, stop } from "./harness.mjs";

before(start);
after(stop);

const resolved = (id, base, bg) => ({
  id,
  name: id,
  author: null,
  base,
  shape: "soft",
  font: "system",
  tokens: { "--bg": bg },
});

/** The fake shell answers `look_active` with the look the settings name. */
const looks = {
  "builtin:ink": resolved("builtin:ink", "dark", "#101010"),
  "builtin:strawberry": resolved("builtin:strawberry", "light", "#fff0f4"),
};

async function root(page) {
  return page.evaluate(() => ({
    theme: document.documentElement.dataset.theme,
    bg: document.documentElement.style.getPropertyValue("--bg"),
  }));
}

describe("appearance", () => {
  it("main: a custom font token is drained when Plain is chosen", async () => {
    const withFont = resolved("builtin:ink", "dark", "#101010");
    withFont.tokens["--font"] = "\"JetBrains Mono\", var(--font-mono-ui)";
    const w = await open({ label: "main", settings: { look_id: "builtin:ink" } });
    await w.page.evaluate((look) => {
      window.__mock.handlers.look_active = () =>
        window.__mock.settings.look_id ? look : null;
    }, withFont);
    await w.mock.changeSettings({ look_id: null });
    await w.mock.changeSettings({ look_id: "builtin:ink" });
    await eventually(
      () => w.page.evaluate(() => document.documentElement.style.getPropertyValue("--font")),
      (v) => v.includes("JetBrains Mono"),
      "the look's font to be worn",
    );

    await w.mock.changeSettings({ look_id: null });
    await eventually(
      () => w.page.evaluate(() => document.documentElement.style.getPropertyValue("--font")),
      (v) => v === "",
      "Plain to drain the look's font",
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
      // the same id is no change at all: drop to Plain, then re-choose.
      await w.mock.changeSettings({ look_id: null });
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

      await w.mock.changeSettings({ look_id: null });
      await eventually(
        () => root(w.page),
        (r) => r.bg === "",
        "Plain to clear the look's colours",
      );
      assert.deepEqual(w.errors, []);
      await w.close();
    });
  }
});
