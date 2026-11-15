-- Note details: a description per note and files attached to it.
-- The note's one-line text stays in `content`; the longer description goes in
-- `details`. Attachments live in this same file so a backup, a clear or the
-- trash purge covers them with no separate cleanup.
ALTER TABLE notes ADD COLUMN details TEXT NOT NULL DEFAULT '';

CREATE TABLE attachments (
    id         TEXT    PRIMARY KEY NOT NULL,
    note_id    TEXT    NOT NULL REFERENCES notes (id) ON DELETE CASCADE,
    name       TEXT    NOT NULL CHECK (length(name) > 0),
    mime       TEXT    NOT NULL,
    size       INTEGER NOT NULL CHECK (size > 0),
    data       BLOB    NOT NULL,
    created_at INTEGER NOT NULL
) WITHOUT ROWID;

CREATE INDEX attachments_by_note ON attachments (note_id, created_at);

-- Search covers the description too: rebuild the index with a third column.
DROP TRIGGER notes_fts_insert;
DROP TRIGGER notes_fts_delete;
DROP TRIGGER notes_fts_update;
DROP TABLE notes_fts;

CREATE VIRTUAL TABLE notes_fts USING fts5(
    content,
    category,
    details,
    content = 'notes',
    content_rowid = 'seq',
    tokenize = 'unicode61 remove_diacritics 2',
    prefix = '2 3'
);

INSERT INTO notes_fts (rowid, content, category, details)
SELECT seq, content, category, details FROM notes;

CREATE TRIGGER notes_fts_insert AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts (rowid, content, category, details)
    VALUES (new.seq, new.content, new.category, new.details);
END;

CREATE TRIGGER notes_fts_delete AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts (notes_fts, rowid, content, category, details)
    VALUES ('delete', old.seq, old.content, old.category, old.details);
END;

CREATE TRIGGER notes_fts_update AFTER UPDATE OF content, category, details ON notes BEGIN
    INSERT INTO notes_fts (notes_fts, rowid, content, category, details)
    VALUES ('delete', old.seq, old.content, old.category, old.details);
    INSERT INTO notes_fts (rowid, content, category, details)
    VALUES (new.seq, new.content, new.category, new.details);
END;
