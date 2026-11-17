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
  for (const label of ["capture", "main", "today", "details"]) {
    it(`${label}: wears a newly chosen look without a reload`, async () => {
      const w = await open({ label, settings: { look_id: "builtin:ink" } });
      await w.page.evaluate((looks) => {
        window.__mock.handlers.look_active = () => looks[window.__mock.settings.look_id] ?? null;
      }, looks);
      // Settings may arrive before the handler above: ask once more.
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
