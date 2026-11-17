import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { eventually, note, open, start, stop } from "./harness.mjs";

before(start);
after(stop);

/** The window sizes the three settings produce, roughly, in CSS pixels. */
const SIZES = [
  ["compact", 360, 260],
  ["standard", 440, 320],
  ["roomy", 560, 400],
];

const ANCHORS = [
  "top-left",
  "top-right",
  "bottom-left",
  "bottom-right",
  "left-middle",
  "right-middle",
];

/** A resolved scene with one backdrop and a sticker at every anchor, as
    large as stickers get: if placement ever covers a control, this shows
    it. */
function sceneFor(window) {
  return {
    main: {},
    capture: {},
    today: {},
    details: {},
    [window]: {
      backdrop: [{ kind: "image", asset: "a1", fit: "cover", dim_tenths: 4 }],
      margin: ANCHORS.map((anchor) => ({
        kind: "emoji",
        char: "🌸",
        anchor,
        size: "xl",
        tilt: 0,
        faded: false,
      })),
    },
    skipped: 0,
  };
}

const overlap = (a, b) =>
  a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;

describe("scene", () => {
  for (const label of ["capture", "main", "today", "details"]) {
    it(`${label}: draws the backdrop and the stickers, and Simple looks clears both live`, async () => {
      const store = { scene: sceneFor(label) };
      if (label === "main" || label === "today") store.notes = [note("hello world")];
      const w = await open({ label, store });

      await eventually(
        () => w.page.locator(".scene-backdrop img").count(),
        (n) => n === 1,
        "the backdrop image to be drawn",
      );
      await eventually(
        () => w.page.locator(".scene-sticker").count(),
        (n) => n === ANCHORS.length,
        "a sticker at every anchor to be drawn",
      );

      // Every scene element is inert.
      const pointerEvents = await w.page.evaluate(() =>
        [...document.querySelectorAll(".scene-backdrop, .scene-sticker")].map(
          (el) => getComputedStyle(el).pointerEvents,
        ),
      );
      assert.ok(
        pointerEvents.every((p) => p === "none"),
        JSON.stringify(pointerEvents),
      );

      // The backdrop image comes from the shell as a data: URL.
      const src = await w.page.locator(".scene-backdrop img").getAttribute("src");
      assert.ok(src.startsWith("data:image/png;base64,"), `src: ${src?.slice(0, 40)}`);

      // Simple looks clears both, live, without a reload.
      await w.mock.changeSettings({ simple_looks: true });
      await eventually(
        () => w.page.locator(".scene-backdrop, .scene-sticker").count(),
        (n) => n === 0,
        "Simple looks to clear the scene",
      );
      assert.deepEqual(w.errors, []);
      await w.close();
    });

    it(`${label}: no sticker ever covers a control at any window size`, async () => {
      const store = { scene: sceneFor(label) };
      if (label === "main" || label === "today") store.notes = [note("hello world")];
      const w = await open({ label, store });
      await eventually(
        () => w.page.locator(".scene-sticker").count(),
        (n) => n === ANCHORS.length,
        "the stickers to be drawn",
      );

      for (const [name, width, height] of SIZES) {
        await w.page.setViewportSize({ width, height });
        const clash = await w.page.evaluate(() => {
          const boxes = [...document.querySelectorAll(".scene-sticker")].map((el) => {
            const r = el.getBoundingClientRect();
            return { x: r.x, y: r.y, width: r.width, height: r.height };
          });
          const overlaps = (a, b) =>
            a.x < b.x + b.width &&
            b.x < a.x + a.width &&
            a.y < b.y + b.height &&
            b.y < a.y + a.height;
          // Anything a person could click, focus or type into.
          const controls = [...document.querySelectorAll("button, a, input, textarea, [role]")].map(
            (el) => {
              const r = el.getBoundingClientRect();
              return {
                name: `${el.tagName.toLowerCase()}${el.getAttribute("aria-label") ? `(${el.getAttribute("aria-label")})` : ""}`,
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
              };
            },
          );
          for (const sticker of boxes) {
            for (const control of controls) {
              if (
                control.width === 0 ||
                control.height === 0 ||
                sticker.width === 0 ||
                sticker.height === 0
              ) {
                continue;
              }
              if (overlaps(sticker, control)) {
                return `${JSON.stringify(sticker)} covers ${control.name} ${JSON.stringify(control)}`;
              }
            }
          }
          return null;
        });
        assert.equal(clash, null, `${name}: ${clash}`);
      }
      assert.deepEqual(w.errors, []);
      await w.close();
    });
  }
});
