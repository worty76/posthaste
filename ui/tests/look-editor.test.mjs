import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { eventually, open, start, stop, text } from "./harness.mjs";

before(start);
after(stop);

/** One user look in the gallery, selected, with an editable scene. */
async function editor(sceneEdit, extra = {}, settings = {}) {
  const store = {
    looks: [
      {
        id: "user:1",
        name: "Mine",
        author: null,
        builtin: false,
        base: "light",
        preview: { bg: "#101010", surface: "#202020", text: "#f0f0f0", accent: "#ff8800" },
      },
    ],
    sceneEdits: sceneEdit ? { "user:1": sceneEdit } : {},
    ...extra,
  };
  const w = await open({
    label: "main",
    store,
    settings: { look_id: "user:1", ...settings },
  });
  await w.page.getByRole("button", { name: "Settings", exact: true }).click();
  await w.page.getByRole("button", { name: "Edit" }).click();
  const dialog = w.page.getByRole("dialog", { name: "Edit look" });
  await eventually(
    () => dialog.count(),
    (n) => n === 1,
    "the editor to open",
  );
  return { ...w, dialog };
}

/** The scene the editor last sent to `look_save`. */
async function savedScene(w) {
  return (await w.mock.calls("look_save")).at(-1)?.args.scene;
}

describe("look editor scene", () => {
  it("adds a sticker and saves it in the chosen window", async () => {
    const w = await editor(null);
    await w.dialog.getByRole("button", { name: "Add sticker" }).click();
    await w.dialog.getByRole("button", { name: "Save" }).click();
    await text(w.toast, 'Saved "Mine"');
    const scene = await savedScene(w);
    assert.equal(scene.scene.all.margin.length, 1);
    assert.equal(scene.scene.all.margin[0].config.char, "🌸");
    assert.equal(scene.scene.all.margin[0].kind, "emoji");
    await w.close();
  });

  it("sends an unknown layer back untouched", async () => {
    const sceneEdit = {
      scene: {
        capture: {
          margin: [{ kind: "shader", v: 9, config: { source: "spiral" }, custom: "keep" }],
        },
      },
      assets: [],
    };
    const w = await editor(sceneEdit);
    await w.dialog.getByRole("button", { name: "Save" }).click();
    await text(w.toast, 'Saved "Mine"');
    const scene = await savedScene(w);
    const layer = scene.scene.capture.margin[0];
    assert.equal(layer.kind, "shader");
    assert.equal(layer.config.source, "spiral");
    assert.equal(layer.custom, "keep");
    await w.close();
  });

  it("removing the backdrop drops its asset when nothing mentions it", async () => {
    const id = `sha256:${"a".repeat(64)}`;
    const sceneEdit = {
      scene: { all: { backdrop: [{ kind: "image", v: 1, config: { asset: id } }] } },
      assets: [{ id, type: "image/png", bytes: 33, width: 2, height: 3 }],
    };
    const w = await editor(sceneEdit);
    await w.dialog.getByRole("button", { name: "Remove image" }).click();
    await w.dialog.getByRole("button", { name: "Save" }).click();
    await text(w.toast, 'Saved "Mine"');
    const scene = await savedScene(w);
    assert.equal(scene.assets.length, 0);
    assert.equal(scene.scene.all.backdrop?.length ?? 0, 0);
    await w.close();
  });

  it("keeps an asset an unknown layer still mentions", async () => {
    const id = `sha256:${"a".repeat(64)}`;
    const sceneEdit = {
      scene: {
        all: { backdrop: [{ kind: "image", v: 1, config: { asset: id } }] },
        capture: { margin: [{ kind: "shader", v: 9, config: { asset: id } }] },
      },
      assets: [{ id, type: "image/png", bytes: 33, width: 2, height: 3 }],
    };
    const w = await editor(sceneEdit);
    await w.dialog.getByRole("button", { name: "Remove image" }).click();
    await w.dialog.getByRole("button", { name: "Save" }).click();
    await text(w.toast, 'Saved "Mine"');
    const scene = await savedScene(w);
    assert.equal(scene.assets.length, 1, "the unknown layer keeps the asset listed");
    await w.close();
  });

  it("does not save a scene it has not loaded yet", async () => {
    const id = `sha256:${"a".repeat(64)}`;
    const loaded = {
      scene: {
        all: { margin: [{ kind: "emoji", v: 1, config: { char: "🌸", anchor: "top-left" } }] },
      },
      assets: [],
    };
    const store = {
      looks: [
        {
          id: "user:1",
          name: "Mine",
          author: null,
          builtin: false,
          base: "light",
          preview: { bg: "#101010", surface: "#202020", text: "#f0f0f0", accent: "#ff8800" },
        },
      ],
    };
    const w = await open({ label: "main", store, settings: { look_id: "user:1" } });
    await w.page.getByRole("button", { name: "Settings", exact: true }).click();
    // The scene load is held, so the editor cannot know the real scene.
    await w.mock.hold("look_scene");
    await w.page.getByRole("button", { name: "Edit" }).click();
    const dialog = w.page.getByRole("dialog", { name: "Edit look" });
    await eventually(
      () => dialog.count(),
      (n) => n === 1,
      "the editor to open",
    );

    // Save is off while the load is in flight, and nothing was sent.
    assert.equal(await dialog.getByRole("button", { name: "Save" }).isDisabled(), true);
    assert.equal((await w.mock.calls("look_save")).length, 0);

    // The load lands, Save comes on, and the save carries the loaded scene.
    await w.mock.release("look_scene", loaded);
    await eventually(
      () => dialog.getByRole("button", { name: "Save" }).isDisabled(),
      (disabled) => disabled === false,
      "Save to come on after the scene loads",
    );
    await dialog.getByRole("button", { name: "Save" }).click();
    await text(w.toast, 'Saved "Mine"');
    const scene = (await w.mock.calls("look_save")).at(-1)?.args.scene;
    assert.equal(scene.scene.all.margin[0].config.char, "🌸");
    await w.close();
  });

  it("notes when Simple looks hides the scene in the windows", async () => {
    const w = await editor(null, {}, { simple_looks: true });
    await text(
      w.dialog.getByRole("status"),
      "Simple looks is hiding stickers and images in the windows.",
    );
    await w.close();
  });

  it("says when part of the look needs a newer Posthaste", async () => {
    const previewScene = { main: {}, capture: {}, today: {}, details: {}, skipped: 2 };
    const w = await editor(null, { previewScene });
    await text(w.dialog.getByRole("status"), "Part of this look needs a newer Posthaste.");
    await w.close();
  });
});
