"use client";

// Small working pieces for the feature blocks. Memory only: nothing typed
// here is stored or sent, and the polish demo uses a ready-made answer.

import { useReducer, useState } from "react";
import { useSite } from "./SiteState";
import { Keys, SHORTCUTS, useBrowserName, useOs } from "./os";
import { searchNotes } from "@/lib/demo";
import { trashReducer, type TrashNote } from "@/lib/trash";

const INBOX_SAMPLES = [
  "Ship v0.2.3 on Friday",
  "Minh owns the release notes",
  "Check the router with Dung before Friday",
  "Buy a USB-C cable",
  "Try the ramen place on Le Loi",
];

export function CaptureMini() {
  const os = useOs() ?? "mac";
  const browser = useBrowserName();
  return (
    <div className="mini mini--capture" aria-hidden="true">
      <Keys keys={SHORTCUTS[os].capture} />
      <div className="mini__window">
        <span className="mini__label">
          <b>Posthaste</b> from {browser}
        </span>
        <span className="mini__text">
          Call Minh about the release notes
          <i className="mini__caret" />
        </span>
      </div>
    </div>
  );
}

export function InboxDemo() {
  const { demo } = useSite();
  const [q, setQ] = useState("");
  const mine = demo.notes.filter((n) => n.savedInMs !== null).map((n) => n.text);
  const all = [...mine, ...INBOX_SAMPLES].map((text, i) => ({ id: i, text }));
  const found = searchNotes(all, q);
  return (
    <div className="mini mini--inbox">
      <label className="visually-hidden" htmlFor="inbox-search">
        Search the demo inbox
      </label>
      <input
        id="inbox-search"
        className="mini__search"
        type="search"
        placeholder="Search all notes"
        value={q}
        onChange={(e) => setQ(e.target.value)}
        autoComplete="off"
      />
      <p className="mini__count" aria-live="polite">
        {found.length} {found.length === 1 ? "note" : "notes"}
      </p>
      {found.length === 0 ? (
        <p className="quiet">No note has every word you typed.</p>
      ) : (
        <ul className="mini__list">
          {found.slice(0, 4).map((n) => (
            <li key={n.id}>{n.text}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

const UNDO_START: TrashNote[] = [
  { id: 1, text: "Renew the domain in December", deletedAt: null },
  { id: 2, text: "Book the dentist for next week", deletedAt: null },
  { id: 3, text: "Reply to Linh about the invoice", deletedAt: null },
];

export function UndoDemo() {
  const [notes, dispatch] = useReducer(trashReducer, UNDO_START);
  return (
    <ul className="mini mini--undo">
      {notes.map((n) =>
        n.deletedAt === null ? (
          <li key={n.id}>
            <span>{n.text}</span>
            <button
              type="button"
              className="link-button"
              aria-label={`Delete "${n.text}"`}
              onClick={() => dispatch({ type: "delete", id: n.id, now: Date.now() })}
            >
              Delete
            </button>
          </li>
        ) : (
          <li key={n.id} className="mini__deleted">
            <span>In the trash for 24 hours.</span>
            <button
              type="button"
              className="link-button"
              onClick={() => dispatch({ type: "undo", id: n.id, now: Date.now() })}
            >
              Undo
            </button>
          </li>
        ),
      )}
    </ul>
  );
}

const ROUGH = "chek router w dung b4 fri";
const POLISHED = "Check the router with Dung before Friday.";

export function AiPolishDemo() {
  const [polished, setPolished] = useState(false);
  const os = useOs();
  return (
    <div className="mini mini--ai">
      <p className="mini__text" aria-live="polite">
        {polished ? POLISHED : ROUGH}
      </p>
      <div className="mini__row">
        <button type="button" className="btn btn--small" onClick={() => setPolished((p) => !p)}>
          {polished ? "Undo polish" : "Polish"}
        </button>
        <span className="quiet">
          <Keys keys={[os === "mac" || os === null ? "⌘" : "Ctrl", "I"]} /> in the app
        </span>
      </div>
      <p className="mini__fine">A demo with a ready-made answer. Nothing leaves this page.</p>
    </div>
  );
}
