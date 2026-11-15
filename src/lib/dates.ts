import type { Note } from "./types";

const DAY_MS = 86_400_000;

/** Start of the local calendar day containing `t`, as UTC ms. */
export function startOfDay(t: number): number {
  const d = new Date(t);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** `[from, to)` bounds of the local day containing `t`. Handles DST days. */
export function dayRange(t: number): { from: number; to: number } {
  const from = startOfDay(t);
  const next = new Date(from);
  next.setDate(next.getDate() + 1);
  return { from, to: next.getTime() };
}

/** Bounds for a `YYYY-MM-DD` value from a date input, or null if invalid. */
export function dayRangeFromInput(value: string): { from: number; to: number } | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return Number.isNaN(d.getTime()) ? null : dayRange(d.getTime());
}

export function dayLabel(t: number, now: number, locale?: string): string {
  const diff = Math.round((startOfDay(now) - startOfDay(t)) / DAY_MS);
  if (diff === 0) return "Today";
  if (diff === 1) return "Yesterday";
  const sameYear = new Date(t).getFullYear() === new Date(now).getFullYear();
  return new Intl.DateTimeFormat(locale, {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: sameYear ? undefined : "numeric",
  }).format(t);
}

export function timeLabel(t: number, locale?: string): string {
  return new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "2-digit" }).format(t);
}

export interface DayGroup {
  key: number;
  label: string;
  notes: Note[];
}

/** Groups notes (already newest first) by the local day they were captured. */
export function groupByDay(notes: Note[], now: number, locale?: string): DayGroup[] {
  const groups: DayGroup[] = [];
  for (const note of notes) {
    const key = startOfDay(note.created_at);
    const last = groups[groups.length - 1];
    if (last && last.key === key) last.notes.push(note);
    else groups.push({ key, label: dayLabel(key, now, locale), notes: [note] });
  }
  return groups;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** The local calendar day containing `t`, written `YYYY-MM-DD`. */
export function dayKey(t: number): string {
  const d = new Date(t);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Whole local calendar days from `a` to `b` (negative when `b` is earlier). */
export function daysBetween(a: number, b: number): number {
  return Math.round((startOfDay(b) - startOfDay(a)) / DAY_MS);
}

/** First day of the week for a locale: 0 Sunday, 1 Monday, 6 Saturday.
 * Falls back to Monday when the runtime cannot tell. */
export function weekStart(locale?: string): 0 | 1 | 6 {
  try {
    const loc = new Intl.Locale(locale ?? navigator.language) as Intl.Locale & {
      getWeekInfo?: () => { firstDay: number };
      weekInfo?: { firstDay: number };
    };
    const first = (loc.getWeekInfo?.() ?? loc.weekInfo)?.firstDay;
    if (first === 7) return 0;
    if (first === 6) return 6;
    return 1;
  } catch {
    return 1;
  }
}

export interface MonthCell {
  key: string;
  /** Local midnight that starts the day. */
  t: number;
  inMonth: boolean;
}

/** Six weeks covering `month` (0-based), starting on `weekStartsOn`. */
export function monthGrid(year: number, month: number, weekStartsOn: 0 | 1 | 6): MonthCell[] {
  const first = new Date(year, month, 1);
  const lead = (first.getDay() - weekStartsOn + 7) % 7;
  const cells: MonthCell[] = [];
  for (let i = 0; i < 42; i++) {
    // Built from calendar fields, so DST days still land on midnight.
    const d = new Date(year, month, 1 - lead + i);
    cells.push({ key: dayKey(d.getTime()), t: d.getTime(), inMonth: d.getMonth() === month });
  }
  return cells;
}

/** Day starts of a grid plus the end of its last day: n cells give n+1 edges. */
export function monthEdges(cells: MonthCell[]): number[] {
  if (cells.length === 0) return [];
  return [...cells.map((c) => c.t), dayRange(cells[cells.length - 1]!.t).to];
}

/** Where a carried note came from, as shown beside it in Today. Null when it
 * was captured on the day being viewed. "Days ago" only makes sense when
 * that day is today, so a past day gets the bare date. */
export function originLabel(
  createdAt: number,
  viewedDay: number,
  now: number,
  locale?: string,
): string | null {
  const before = daysBetween(createdAt, viewedDay);
  if (before <= 0) return null;
  const viewingToday = daysBetween(viewedDay, now) === 0;
  if (viewingToday && before === 1) return "Yesterday";
  const sameYear = new Date(createdAt).getFullYear() === new Date(now).getFullYear();
  const date = new Intl.DateTimeFormat(locale, {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: sameYear ? undefined : "numeric",
  })
    .format(createdAt)
    .replace(/,/g, "");
  return viewingToday ? `${date} · ${before} days ago` : date;
}

/** Groups trashed items (already newest first) by the local day they were
 * deleted. */
export function groupByDeletedDay<T extends { deleted_at: number }>(
  items: T[],
  now: number,
  locale?: string,
): { key: number; label: string; items: T[] }[] {
  const groups: { key: number; label: string; items: T[] }[] = [];
  for (const item of items) {
    const key = startOfDay(item.deleted_at);
    const last = groups[groups.length - 1];
    if (last && last.key === key) last.items.push(item);
    else groups.push({ key, label: dayLabel(key, now, locale), items: [item] });
  }
  return groups;
}

/** When a note deleted at `deletedAt` is erased for good, e.g. "Erased on 15 Dec". */
export function erasedOn(deletedAt: number, retentionMs: number, locale?: string): string {
  const at = deletedAt + retentionMs;
  const sameYear = new Date(at).getFullYear() === new Date(deletedAt).getFullYear();
  const date = new Intl.DateTimeFormat(locale, {
    day: "numeric",
    month: "short",
    year: sameYear ? undefined : "numeric",
  })
    .format(at)
    .replace(/,/g, "");
  return `Erased on ${date}`;
}
