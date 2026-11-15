// Pure helpers for attachments: sizes, kinds, and turning bytes into
// something an <img> can show under the app's content security policy.

import { MAX_ATTACHMENTS_PER_NOTE, type Attachment } from "./types";

/** "860 B", "86 KB", "2.4 MB". */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  const mb = n / (1024 * 1024);
  return `${mb < 10 ? mb.toFixed(1) : Math.round(mb)} MB`;
}

export type AttachmentKind = "image" | "pdf" | "text" | "other";

export function attachmentKind(a: Pick<Attachment, "mime">): AttachmentKind {
  if (a.mime.startsWith("image/")) return "image";
  if (a.mime === "application/pdf") return "pdf";
  if (a.mime.startsWith("text/") || a.mime === "application/json") return "text";
  return "other";
}

/** A short type label for a tile: the extension, or the kind. */
export function typeLabel(a: Pick<Attachment, "name" | "mime">): string {
  const dot = a.name.lastIndexOf(".");
  if (dot > 0 && dot < a.name.length - 1 && a.name.length - dot <= 6) {
    return a.name.slice(dot + 1).toUpperCase();
  }
  return attachmentKind(a) === "other" ? "FILE" : attachmentKind(a).toUpperCase();
}

/** Why a batch can't be attached at all, before asking the native side. */
export function batchRefusal(adding: number, existing: number): string | null {
  if (adding <= 0) return null;
  const room = MAX_ATTACHMENTS_PER_NOTE - existing;
  if (room <= 0) return "This note already has 20 files. Remove one to add another.";
  return null;
}

/** `data:` URL for image bytes. `blob:` URLs are not allowed by the CSP. */
export function toDataUrl(bytes: ArrayBuffer | Uint8Array, mime: string): string {
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < view.length; i += chunk) {
    binary += String.fromCharCode(...view.subarray(i, i + chunk));
  }
  return `data:${mime};base64,${btoa(binary)}`;
}

/** Name for an image pasted from the clipboard, e.g. "Pasted image 2026-11-15 15.40.png". */
export function pastedImageName(t: number, mime: string): string {
  const d = new Date(t);
  const pad = (n: number) => String(n).padStart(2, "0");
  const ext = mime === "image/jpeg" ? "jpg" : (mime.split("/")[1] ?? "png").replace("+xml", "");
  return `Pasted image ${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(
    d.getHours(),
  )}.${pad(d.getMinutes())}.${ext}`;
}
