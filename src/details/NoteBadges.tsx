import { Icon } from "../components/Icon";
import type { Note } from "../lib/types";
/** Small marks for a note that has more inside: a description, files. */
export function NoteBadges({ note }: { note: Note }) {
  if (!note.has_details && note.attachment_count === 0) return null;
  const parts = [
    note.has_details && "has details",
    note.attachment_count > 0 &&
      `${note.attachment_count} ${note.attachment_count === 1 ? "file" : "files"}`,
  ].filter(Boolean);
  return (
    <span className="badges" aria-label={parts.join(", ")} title={parts.join(", ")}>
      {note.has_details && <Icon name="lines" size={12} />}
      {note.attachment_count > 0 && (
        <span className="badges__files">
          <Icon name="paperclip" size={12} />
          {note.attachment_count}
        </span>
      )}
    </span>
  );
}
