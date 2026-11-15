import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Attachment, NoteDetail } from "../lib/types";
import { api, dropFiles, emit, note, resetIpc } from "../test/mockIpc";
import { DetailsWindow } from "./DetailsWindow";

vi.mock("../lib/ipc", async () => (await import("../test/mockIpc")).ipcMock);

function attachment(name: string, patch: Partial<Attachment> = {}): Attachment {
  return {
    id: `att-${name}`,
    note_id: "n1",
    name,
    mime: "text/plain",
    size: 2048,
    created_at: 1,
    ...patch,
  };
}

function detail(patch: Partial<NoteDetail> = {}, id = "n1"): NoteDetail {
  return {
    note: note("Prepare Q4 review", { id }),
    details: "agenda",
    attachments: [],
    ...patch,
  };
}

beforeEach(() => {
  resetIpc();
  api.noteDetailGet.mockImplementation(async (id: string) => detail({}, id));
  api.noteDetailsSet.mockImplementation(async (id: string) => note("x", { id }));
  api.noteUpdate.mockImplementation(async (id: string) => note("x", { id }));
});

afterEach(() => {
  vi.useRealTimers();
});

async function openOn(id = "n1") {
  render(<DetailsWindow os="linux" />);
  await act(async () => emit("details-open", { id, from: "today" }));
  await screen.findByDisplayValue("Prepare Q4 review");
}

const textarea = () => screen.getByLabelText("Details") as HTMLTextAreaElement;
const panel = () => document.querySelector(".details")!;

describe("note details window", () => {
  it("loads the note the window was opened on, also when it mounts late", async () => {
    api.detailsCurrent.mockResolvedValue({ id: "n7", from: "inbox" });
    render(<DetailsWindow os="linux" />);
    await screen.findByDisplayValue("agenda");
    expect(api.noteDetailGet).toHaveBeenCalledWith("n7");
  });

  it("the status chip says Later for a parked note", async () => {
    api.noteDetailGet.mockResolvedValue(
      detail({ note: note("Prepare Q4 review", { id: "n1", later_at: 1 }) }),
    );
    await openOn();
    expect(document.querySelector(".details__status")).toHaveTextContent("Later");
  });

  it("autosaves the description after a pause", async () => {
    await openOn();
    vi.useFakeTimers();
    fireEvent.change(textarea(), { target: { value: "agenda\n- numbers" } });
    expect(api.noteDetailsSet).not.toHaveBeenCalled();
    expect(screen.getByRole("status")).toHaveTextContent("Saving");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    expect(api.noteDetailsSet).toHaveBeenCalledWith("n1", "agenda\n- numbers");
    expect(screen.getByRole("status")).toHaveTextContent("Saved");
  });

  it("Esc saves what was just typed before closing", async () => {
    await openOn();
    const order: string[] = [];
    api.noteDetailsSet.mockImplementation(async () => {
      order.push("save");
      return note("x");
    });
    api.detailsDismiss.mockImplementation(async () => void order.push("dismiss"));
    fireEvent.change(textarea(), { target: { value: "last keystrokes" } });
    await act(async () => {
      fireEvent.keyDown(panel(), { key: "Escape" });
    });
    await waitFor(() => expect(order).toEqual(["save", "dismiss"]));
    expect(api.noteDetailsSet).toHaveBeenCalledWith("n1", "last keystrokes");
  });

  it("the window's close button request also saves first", async () => {
    await openOn();
    fireEvent.change(textarea(), { target: { value: "closing via the OS" } });
    await act(async () => emit("details-close-requested"));
    await waitFor(() => expect(api.detailsDismiss).toHaveBeenCalled());
    expect(api.noteDetailsSet).toHaveBeenCalledWith("n1", "closing via the OS");
  });

  it("switching to another note saves the first one, then shows the second", async () => {
    await openOn("n1");
    fireEvent.change(textarea(), { target: { value: "unsaved on n1" } });
    api.noteDetailGet.mockImplementation(async (id: string) =>
      detail({ details: "second note", note: note("Other", { id }) }, id),
    );
    await act(async () => emit("details-open", { id: "n2", from: "today" }));
    await screen.findByDisplayValue("second note");
    expect(api.noteDetailsSet).toHaveBeenCalledWith("n1", "unsaved on n1");
    expect(api.noteDetailsSet).not.toHaveBeenCalledWith("n2", expect.anything());
  });

  it("a change from elsewhere does not overwrite text the user is typing", async () => {
    await openOn();
    fireEvent.change(textarea(), { target: { value: "typing in progress" } });
    api.noteDetailGet.mockResolvedValue(detail({ details: "changed elsewhere" }));
    await act(async () => emit("notes-changed"));
    expect(textarea().value).toBe("typing in progress");
  });

  it("a change from elsewhere shows when nothing is unsaved", async () => {
    await openOn();
    api.noteDetailGet.mockResolvedValue(
      detail({ details: "changed elsewhere", attachments: [attachment("log.txt")] }),
    );
    await act(async () => emit("notes-changed"));
    await screen.findByDisplayValue("changed elsewhere");
    expect(screen.getByText("log.txt")).toBeInTheDocument();
  });

  it("closes when the note is deleted elsewhere", async () => {
    await openOn();
    api.noteDetailGet.mockRejectedValue({ code: "not_found", message: "gone" });
    await act(async () => emit("notes-changed"));
    expect(api.detailsDismiss).toHaveBeenCalled();
  });

  it("lists attachments with name, size and actions", async () => {
    api.noteDetailGet.mockResolvedValue(
      detail({
        attachments: [attachment("Q3-report.pdf", { mime: "application/pdf", size: 2_516_582 })],
      }),
    );
    await openOn();
    expect(screen.getByText("Q3-report.pdf")).toBeInTheDocument();
    expect(screen.getByText("2.4 MB")).toBeInTheDocument();
    expect(screen.getByText("PDF")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Save a copy of Q3-report.pdf" }));
    expect(api.attachmentSaveCopy).toHaveBeenCalledWith("att-Q3-report.pdf");
    fireEvent.click(screen.getByRole("button", { name: "Open Q3-report.pdf" }));
    expect(api.attachmentOpen).toHaveBeenCalledWith("att-Q3-report.pdf");
  });

  it("Delete on a focused tile removes the file and says so", async () => {
    api.noteDetailGet.mockResolvedValue(detail({ attachments: [attachment("server.log")] }));
    await openOn();
    const tile = screen.getByRole("button", { name: /^server\.log, 2 KB/ });
    tile.focus();
    await act(async () => {
      fireEvent.keyDown(tile, { key: "Delete" });
    });
    expect(api.attachmentRemove).toHaveBeenCalledWith("att-server.log");
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Removed server.log"));
  });

  it("shows image previews from the stored bytes", async () => {
    api.noteDetailGet.mockResolvedValue(
      detail({ attachments: [attachment("chart.png", { mime: "image/png" })] }),
    );
    api.attachmentPreview.mockResolvedValue(new Uint8Array([1, 2, 3]).buffer);
    await openOn();
    await waitFor(() =>
      expect(document.querySelector("img.tile__thumb")).toHaveAttribute(
        "src",
        "data:image/png;base64,AQID",
      ),
    );
  });

  it("Mod+O asks for files and reports what was refused, by name", async () => {
    api.attachmentAddFiles.mockResolvedValue({
      added: [attachment("a.txt")],
      failed: [
        { name: "movie.mov", code: "attachment_too_large", message: "This file is 220.0 MB." },
      ],
    });
    await openOn();
    await act(async () => {
      fireEvent.keyDown(panel(), { key: "o", ctrlKey: true });
    });
    expect(api.attachmentAddFiles).toHaveBeenCalledWith("n1");
    expect(await screen.findByRole("alert")).toHaveTextContent("movie.mov: This file is 220.0 MB.");
    expect(screen.getByRole("status")).toHaveTextContent("Attached a.txt");
  });

  it("dropped files are attached to the open note", async () => {
    api.attachmentAddPaths.mockResolvedValue({ added: [], failed: [] });
    await openOn();
    await act(async () => dropFiles(["/tmp/a.png", "/tmp/b.pdf"]));
    expect(api.attachmentAddPaths).toHaveBeenCalledWith("n1", ["/tmp/a.png", "/tmp/b.pdf"]);
  });

  it("a note that is full refuses more files without asking the native side", async () => {
    api.noteDetailGet.mockResolvedValue(
      detail({ attachments: Array.from({ length: 20 }, (_, i) => attachment(`f${i}.txt`)) }),
    );
    await openOn();
    await act(async () => dropFiles(["/tmp/one-more.txt"]));
    expect(api.attachmentAddPaths).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("already has 20 files");
  });

  it("a pasted image is sent as bytes with a readable name", async () => {
    api.attachmentAddBytes.mockResolvedValue(attachment("x.png"));
    await openOn();
    const file = new File([new Uint8Array([9, 8, 7])], "image.png", { type: "image/png" });
    await act(async () => {
      fireEvent.paste(textarea(), { clipboardData: { files: [file] } });
    });
    await waitFor(() => expect(api.attachmentAddBytes).toHaveBeenCalled());
    const [id, name, mime, bytes] = api.attachmentAddBytes.mock.calls[0]!;
    expect(id).toBe("n1");
    expect(name).toMatch(/^Pasted image \d{4}-\d{2}-\d{2} \d{2}\.\d{2}\.png$/);
    expect(mime).toBe("image/png");
    expect(Array.from(bytes as Uint8Array)).toEqual([9, 8, 7]);
  });

  it("pasting plain text is left to the text field", async () => {
    await openOn();
    await act(async () => {
      fireEvent.paste(textarea(), { clipboardData: { files: [] } });
    });
    expect(api.attachmentAddBytes).not.toHaveBeenCalled();
  });

  it("Mod+Enter toggles done", async () => {
    await openOn();
    fireEvent.keyDown(panel(), { key: "Enter", ctrlKey: true });
    expect(api.noteSetStatus).toHaveBeenCalledWith("n1", "completed");
  });

  it("editing the title saves it, and an emptied title comes back", async () => {
    await openOn();
    vi.useFakeTimers();
    const title = screen.getByLabelText("Title") as HTMLInputElement;
    fireEvent.change(title, { target: { value: "Prepare Q4 planning review" } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    expect(api.noteUpdate).toHaveBeenCalledWith("n1", "Prepare Q4 planning review", null);

    fireEvent.change(title, { target: { value: "  " } });
    fireEvent.blur(title);
    expect(title.value).toBe("Prepare Q4 planning review");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    expect(api.noteUpdate).toHaveBeenCalledTimes(1);
  });

  it("ignores Escape while an IME is composing", async () => {
    await openOn();
    fireEvent.keyDown(panel(), { key: "Escape", isComposing: true });
    await act(async () => {});
    expect(api.detailsDismiss).not.toHaveBeenCalled();
  });
});

describe("where the note came from", () => {
  const DAY = 86_400_000;
  const meta = () => document.querySelector(".details__meta")!;

  it("names the source app and how long an open note has been carried", async () => {
    const created = Date.now() - 6 * DAY;
    api.noteDetailGet.mockResolvedValue(
      detail({
        note: note("Prepare Q4 review", {
          id: "n1",
          created_at: created,
          updated_at: created,
          source: "Safari",
        }),
      }),
    );
    await openOn();
    expect(meta()).toHaveTextContent("from Safari");
    expect(meta()).toHaveTextContent("carried over 6 days");
  });

  it("leaves out the source when unknown and the carry when done or new", async () => {
    const created = Date.now() - 6 * DAY;
    api.noteDetailGet.mockResolvedValue(
      detail({
        note: note("Prepare Q4 review", {
          id: "n1",
          created_at: created,
          updated_at: created,
          source: null,
          status: "completed",
          completed_at: Date.now(),
        }),
      }),
    );
    await openOn();
    expect(meta()).not.toHaveTextContent("from");
    expect(meta()).not.toHaveTextContent("carried");
  });

  it("says one day in the singular and nothing for a note from today", async () => {
    api.noteDetailGet.mockResolvedValue(
      detail({
        note: note("Prepare Q4 review", {
          id: "n1",
          created_at: Date.now() - DAY,
          updated_at: Date.now() - DAY,
        }),
      }),
    );
    await openOn();
    expect(meta()).toHaveTextContent("carried over 1 day");
    expect(meta()).not.toHaveTextContent("1 days");
  });
});
