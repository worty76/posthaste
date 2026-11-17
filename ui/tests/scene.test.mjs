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

const FIELD = {
  capture: ".capture__input",
  main: ".note__body",
  today: ".note__body",
  details: ".details__title",
};

/** A resolved scene with one backdrop and one sticker in `window`. */
function sceneFor(window) {
  return {
    main: {},
    capture: {},
    today: {},
    details: {},
    [window]: {
      backdrop: [{ kind: "image", asset: "a1", fit: "cover", dim_tenths: 4 }],
      margin: [
        { kind: "emoji", char: "🌸", anchor: "top-right", size: "m", tilt: 0, faded: false },
      ],
    },
    skipped: 0,
  };
}

const overlap = (a, b) =>
  a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;

describe("scene", () => {
  for (const label of ["capture", "main", "today", "details"]) {
    it(`${label}: draws the backdrop and the sticker, and Simple looks clears both live`, async () => {
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
        (n) => n === 1,
        "the sticker to be drawn",
      );
      assert.equal(await w.page.locator(".scene-sticker").textContent(), "🌸");

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

    it(`${label}: the sticker never covers the text at any window size`, async () => {
      const store = { scene: sceneFor(label) };
      if (label === "main" || label === "today") store.notes = [note("hello world")];
      const w = await open({ label, store });
      await eventually(
        () => w.page.locator(".scene-sticker").count(),
        (n) => n === 1,
        "the sticker to be drawn",
      );

      for (const [name, width, height] of SIZES) {
        await w.page.setViewportSize({ width, height });
        const sticker = await w.page.locator(".scene-sticker").boundingBox();
        const field = await w.page.locator(FIELD[label]).first().boundingBox();
        assert.ok(sticker, "sticker has a box");
        assert.ok(field, `${FIELD[label]} is on screen`);
        assert.ok(
          !overlap(sticker, field),
          `${name}: sticker ${JSON.stringify(sticker)} overlaps ${FIELD[label]} ${JSON.stringify(field)}`,
        );
      }
      assert.deepEqual(w.errors, []);
      await w.close();
    });
  }
});
