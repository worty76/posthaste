import { useCallback, useEffect, useRef, useState } from "react";
import { IconButton } from "../components/Button";
import { Icon } from "../components/Icon";
import { Kbd } from "../components/Kbd";
import { useAppearance } from "../hooks/useAppearance";
import { useSettings } from "../hooks/useSettings";
import {
  attachmentKind,
  batchRefusal,
  formatBytes,
  pastedImageName,
  toDataUrl,
  typeLabel,
} from "../lib/attachments";
import { dayLabel, daysBetween, timeLabel } from "../lib/dates";
import { DraftSync } from "../lib/draftSync";
import { api, on, onFileDrop, toAppError } from "../lib/ipc";
import { hasMod } from "../lib/shortcut";
import {
  MAX_ATTACHMENT_BYTES,
  type AttachReport,
  type Attachment,
  type DetailsTarget,
  type NoteDetail,
  type Os,
} from "../lib/types";

/** How long typing pauses before the description is written. */
const SAVE_DELAY_MS = 500;

type SaveState = "idle" | "saving" | "saved";

export function DetailsWindow({ os }: { os: Os }) {
  const { settings } = useSettings();
  useAppearance(settings);

  const [detail, setDetail] = useState<NoteDetail | null>(null);
  const [title, setTitle] = useState("");
  const [text, setText] = useState("");
  const [save, setSave] = useState<SaveState>("idle");
  const [notice, setNotice] = useState<string | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [previews, setPreviews] = useState<Record<string, string | null>>({});
  const [dropping, setDropping] = useState(false);

  const idRef = useRef<string | null>(null);
  const detailRef = useRef<NoteDetail | null>(null);
  const savedText = useRef("");
  const savedTitle = useRef("");
  const textSync = useRef<DraftSync | null>(null);
  const titleSync = useRef<DraftSync | null>(null);
  const loadSeq = useRef(0);
  const textRef = useRef<HTMLTextAreaElement>(null);
  const addRef = useRef<HTMLButtonElement>(null);
  const closing = useRef(false);

  const fail = useCallback((e: unknown) => setErrors([toAppError(e).message]), []);

  /** Writes anything typed but not yet stored. */
  const flush = useCallback(async () => {
    await Promise.all([textSync.current?.flush(), titleSync.current?.flush()]);
  }, []);

  const close = useCallback(async () => {
    if (closing.current) return;
    closing.current = true;
    try {
      await flush();
      await api.detailsDismiss();
    } finally {
      closing.current = false;
    }
  }, [flush]);

  /** Fetches the note. `fresh` replaces the text fields; otherwise they are
   * only replaced when the user has nothing unsaved in them. */
  const load = useCallback(
    async (id: string, fresh: boolean) => {
      const seq = ++loadSeq.current;
      try {
        const d = await api.noteDetailGet(id);
        if (seq !== loadSeq.current || idRef.current !== id) return;
        detailRef.current = d;
        setDetail(d);
        setErrors((current) => (fresh ? [] : current));
        // Compare with what was saved before this load: the updaters run later.
        const prevText = savedText.current;
        const prevTitle = savedTitle.current;
        setText((current) => (fresh || current === prevText ? d.details : current));
        setTitle((current) => (fresh || current === prevTitle ? d.note.content : current));
        savedText.current = d.details;
        savedTitle.current = d.note.content;
        if (fresh) requestAnimationFrame(() => focusEnd(textRef.current));
      } catch (e) {
        if (seq !== loadSeq.current || idRef.current !== id) return;
        if (toAppError(e).code === "not_found") {
          // Deleted elsewhere: there is nothing left to show or save into.
          textSync.current = null;
          titleSync.current = null;
          void api.detailsDismiss();
        } else {
          fail(e);
        }
      }
    },
    [fail],
  );

  /** Shows another note, saving the current one first. */
  const open = useCallback(
    async (target: DetailsTarget) => {
      if (idRef.current === target.id) {
        await load(target.id, false);
        requestAnimationFrame(() => focusEnd(textRef.current));
        return;
      }
      await flush();
      const id = target.id;
      idRef.current = id;
      setDetail(null);
      setPreviews({});
      setNotice(null);
      setErrors([]);
      setSave("idle");
      textSync.current = new DraftSync(
        async (value) => {
          await api.noteDetailsSet(id, value);
          if (idRef.current === id) {
            savedText.current = value;
            setSave("saved");
          }
        },
        fail,
        SAVE_DELAY_MS,
      );
      titleSync.current = new DraftSync(
        async (value) => {
          await api.noteUpdate(id, value, detailRef.current?.note.category ?? null);
          if (idRef.current === id) {
            savedTitle.current = value;
            setSave("saved");
          }
        },
        fail,
        SAVE_DELAY_MS,
      );
      await load(id, true);
    },
    [flush, load, fail],
  );

  useEffect(() => {
    void api.detailsCurrent().then((t) => t && void open(t), fail);
  }, [open, fail]);
  useEffect(() => on("details-open", (t) => void open(t)), [open]);
  useEffect(() => on("details-close-requested", () => void close()), [close]);
  useEffect(
    () =>
      on("notes-changed", () => {
        if (idRef.current) void load(idRef.current, false);
      }),
    [load],
  );

  const report = useCallback((r: AttachReport | null) => {
    if (!r) return;
    if (r.added.length > 0) {
      setNotice(
        r.added.length === 1 ? `Attached ${r.added[0]!.name}` : `Attached ${r.added.length} files`,
      );
    }
    setErrors(r.failed.map((f) => `${f.name}: ${f.message}`));
  }, []);

  const count = detail?.attachments.length ?? 0;

  const addPaths = useCallback(
    (paths: string[]) => {
      const id = idRef.current;
      if (!id) return;
      const refusal = batchRefusal(paths.length, detailRef.current?.attachments.length ?? 0);
      if (refusal) return setErrors([refusal]);
      void api.attachmentAddPaths(id, paths).then(report, fail);
    },
    [report, fail],
  );

  useEffect(() => onFileDrop(addPaths, setDropping), [addPaths]);

  const addFiles = () => {
    const id = idRef.current;
    if (!id) return;
    const refusal = batchRefusal(1, count);
    if (refusal) return setErrors([refusal]);
    void api.attachmentAddFiles(id).then(report, fail);
  };

  const onPaste = (e: ClipboardEvent) => {
    const id = idRef.current;
    const images = Array.from(e.clipboardData?.files ?? []).filter((f) =>
      f.type.startsWith("image/"),
    );
    if (!id || images.length === 0) return;
    e.preventDefault();
    const refusal = batchRefusal(images.length, count);
    if (refusal) return setErrors([refusal]);
    void (async () => {
      const failed: string[] = [];
      let added = 0;
      for (const file of images) {
        const name =
          file.name && file.name !== "image.png"
            ? file.name
            : pastedImageName(Date.now(), file.type);
        if (file.size > MAX_ATTACHMENT_BYTES) {
          failed.push(`${name}: This file is over 15 MB. Posthaste attaches files up to 15 MB.`);
          continue;
        }
        try {
          const bytes = new Uint8Array(await file.arrayBuffer());
          await api.attachmentAddBytes(id, name, file.type, bytes);
          added += 1;
        } catch (err) {
          failed.push(`${name}: ${toAppError(err).message}`);
        }
      }
      if (added > 0)
        setNotice(added === 1 ? "Attached the pasted image" : `Attached ${added} images`);
      setErrors(failed);
    })();
  };

  const remove = (a: Attachment) => {
    void api.attachmentRemove(a.id).then(() => {
      setNotice(`Removed ${a.name}`);
      addRef.current?.focus();
    }, fail);
  };

  const saveCopy = (a: Attachment) => {
    void api.attachmentSaveCopy(a.id).then((path) => {
      if (path) setNotice(`Saved a copy of ${a.name}`);
    }, fail);
  };

  const openFile = (a: Attachment) => void api.attachmentOpen(a.id).catch(fail);

  const toggleDone = () => {
    const note = detailRef.current?.note;
    if (!note) return;
    void api
      .noteSetStatus(note.id, note.status === "completed" ? "inbox" : "completed")
      .catch(fail);
  };

  // Image previews load lazily, once per attachment.
  useEffect(() => {
    for (const a of detail?.attachments ?? []) {
      if (attachmentKind(a) !== "image" || a.id in previews) continue;
      setPreviews((p) => ({ ...p, [a.id]: null }));
      api.attachmentPreview(a.id).then(
        (bytes) => setPreviews((p) => ({ ...p, [a.id]: toDataUrl(bytes, a.mime) })),
        () => undefined,
      );
    }
  }, [detail, previews]);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.isComposing || e.defaultPrevented) return;
    if (e.key === "Escape") {
      e.preventDefault();
      void close();
    } else if (hasMod(e, os) && e.key === "Enter") {
      e.preventDefault();
      toggleDone();
    } else if (hasMod(e, os) && e.key.toLowerCase() === "o" && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      addFiles();
    }
  };

  // Window-level, so keys and paste work wherever focus sits in the page.
  const keyRef = useRef(onKeyDown);
  const pasteRef = useRef(onPaste);
  keyRef.current = onKeyDown;
  pasteRef.current = onPaste;
  useEffect(() => {
    const key = (e: KeyboardEvent) => keyRef.current(e);
    const paste = (e: ClipboardEvent) => pasteRef.current(e);
    // The OS may hand the window focus after the page asked for it; put the
    // caret back in the description when nothing else holds focus.
    const focus = () => {
      if (!document.activeElement || document.activeElement === document.body) {
        focusEnd(textRef.current);
      }
    };
    window.addEventListener("keydown", key);
    window.addEventListener("paste", paste);
    window.addEventListener("focus", focus);
    return () => {
      window.removeEventListener("keydown", key);
      window.removeEventListener("paste", paste);
      window.removeEventListener("focus", focus);
    };
  }, []);

  const note = detail?.note;
  const done = note?.status === "completed";
  const mod = os === "macos" ? "⌘" : "Ctrl";
  const now = Date.now();
  // Open and not parked: how many days it has been carried to the next one.
  const carriedDays =
    note && note.status === "inbox" && note.later_at === null
      ? daysBetween(note.created_at, now)
      : 0;

  return (
    <div className="details">
      <section className="details__panel" aria-label="Note details" data-dropping={dropping}>
        <header className="details__header" data-tauri-drag-region>
          <input
            className="details__title"
            aria-label="Title"
            value={title}
            disabled={!note}
            onChange={(e) => {
              setTitle(e.target.value);
              const trimmed = e.target.value.trim();
              if (trimmed) {
                setSave("saving");
                titleSync.current?.schedule(e.target.value, null);
              }
            }}
            onBlur={() => {
              // An emptied title would make the note empty: put it back.
              if (!title.trim()) setTitle(savedTitle.current);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.nativeEvent.isComposing && !hasMod(e, os)) {
                e.preventDefault();
                textRef.current?.focus();
              }
            }}
          />
          {note && (
            <button
              type="button"
              className="details__status"
              data-done={done}
              aria-pressed={done}
              title={`Mark as ${done ? "not completed" : "completed"} (${mod}+Enter)`}
              onClick={toggleDone}
            >
              {done && <Icon name="check" size={12} />}
              {done
                ? "Done"
                : note.status === "archived"
                  ? "Archived"
                  : note.later_at !== null
                    ? "Later"
                    : "Inbox"}
            </button>
          )}
          <IconButton icon="close" label="Close (Esc)" onClick={() => void close()} />
        </header>

        {note && (
          <p className="details__meta">
            Created {dayLabel(note.created_at, now).toLowerCase()}, {timeLabel(note.created_at)}
            {note.source && <> · from {note.source}</>}
            {carriedDays > 0 && (
              <>
                {" "}
                · carried over {carriedDays} {carriedDays === 1 ? "day" : "days"}
              </>
            )}
            {note.updated_at - note.created_at > 60_000 && (
              <>
                {" "}
                · edited {dayLabel(note.updated_at, now).toLowerCase()},{" "}
                {timeLabel(note.updated_at)}
              </>
            )}
          </p>
        )}

        <div className="details__body">
          <textarea
            ref={textRef}
            className="details__text selectable"
            aria-label="Details"
            placeholder="Add details…"
            value={text}
            disabled={!note}
            onChange={(e) => {
              setText(e.target.value);
              setSave("saving");
              textSync.current?.schedule(e.target.value, null);
            }}
          />

          <section className="details__files" aria-label="Attachments">
            <h2 className="details__files-title">
              Attachments{count > 0 && <span className="details__count">{count}</span>}
            </h2>
            {count > 0 && (
              <ul className="details__grid">
                {detail!.attachments.map((a) => (
                  <Tile
                    key={a.id}
                    attachment={a}
                    preview={previews[a.id] ?? null}
                    onOpen={openFile}
                    onSave={saveCopy}
                    onRemove={remove}
                  />
                ))}
              </ul>
            )}
            <div className="details__drop" data-empty={count === 0}>
              <button
                ref={addRef}
                type="button"
                className="btn"
                onClick={addFiles}
                disabled={!note}
              >
                <Icon name="paperclip" size={14} /> Add files
              </button>
              <span className="details__drop-hint">
                {dropping ? "Drop to attach" : `Drop files here, paste an image, or press ${mod}+O`}
              </span>
            </div>
          </section>

          {errors.length > 0 && (
            <ul className="details__errors" role="alert">
              {errors.map((m, i) => (
                <li key={i}>{m}</li>
              ))}
            </ul>
          )}
        </div>

        <footer className="details__footer">
          <p className="details__saved" role="status">
            {notice ?? (save === "saving" ? "Saving…" : save === "saved" ? "Saved" : "")}
          </p>
          <span className="details__hints" aria-hidden="true">
            <span className="details__hint">
              <Kbd keys={["esc"]} /> close
            </span>
            <span className="details__hint">
              <Kbd keys={[mod, "↵"]} /> done
            </span>
            <span className="details__hint">
              <Kbd keys={[mod, "O"]} /> add file
            </span>
          </span>
        </footer>
      </section>
    </div>
  );
}

function Tile({
  attachment: a,
  preview,
  onOpen,
  onSave,
  onRemove,
}: {
  attachment: Attachment;
  preview: string | null;
  onOpen: (a: Attachment) => void;
  onSave: (a: Attachment) => void;
  onRemove: (a: Attachment) => void;
}) {
  const kind = attachmentKind(a);
  return (
    <li className="tile" data-kind={kind}>
      <button
        type="button"
        className="tile__main"
        title={`Open ${a.name} (Enter) · Remove (Delete)`}
        aria-label={`${a.name}, ${formatBytes(a.size)}. Enter opens it, Delete removes it.`}
        onClick={() => onOpen(a)}
        onKeyDown={(e) => {
          if (e.key === "Delete" || e.key === "Backspace") {
            e.preventDefault();
            onRemove(a);
          }
        }}
      >
        {preview ? (
          <img className="tile__thumb" src={preview} alt="" />
        ) : (
          <span className="tile__glyph" aria-hidden="true">
            <Icon name={kind === "image" ? "image" : "file"} size={20} />
            <span className="tile__type">{typeLabel(a)}</span>
          </span>
        )}
        <span className="tile__name">{a.name}</span>
        <span className="tile__size">{formatBytes(a.size)}</span>
      </button>
      <span className="tile__actions">
        <IconButton icon="open" label={`Open ${a.name}`} onClick={() => onOpen(a)} />
        <IconButton icon="download" label={`Save a copy of ${a.name}`} onClick={() => onSave(a)} />
        <IconButton icon="trash" label={`Remove ${a.name}`} danger onClick={() => onRemove(a)} />
      </span>
    </li>
  );
}

function focusEnd(el: HTMLTextAreaElement | null) {
  if (!el) return;
  el.focus();
  el.setSelectionRange(el.value.length, el.value.length);
}
