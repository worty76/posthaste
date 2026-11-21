-- Steps inside a note: an ordered list of plain one-line items.
-- `seq` mirrors `notes`: an explicit rowid so a full-text index can use it
-- as its content rowid and survive VACUUM. `deleted_at` is a local
-- tombstone; the synced truth for a deletion is its own field clock.
CREATE TABLE note_steps (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    id         TEXT    NOT NULL UNIQUE,
    note_id    TEXT    NOT NULL REFERENCES notes (id) ON DELETE CASCADE,
    text       TEXT    NOT NULL CHECK (length(trim(text)) > 0 AND length(text) <= 500
                                       AND instr(text, char(10)) = 0 AND instr(text, char(13)) = 0),
    done       INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1)),
    rank       TEXT    NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);

CREATE INDEX note_steps_by_note ON note_steps (note_id, rank, id) WHERE deleted_at IS NULL;

-- Step changes for a note this device does not have yet wait here until the
-- note arrives in a later batch, then apply once and are deleted.
CREATE TABLE sync_held_steps (
    note_id TEXT NOT NULL,
    field   TEXT NOT NULL,
    value   TEXT NOT NULL,
    hlc     TEXT NOT NULL,
    PRIMARY KEY (note_id, field)
) WITHOUT ROWID;

-- Lowest seq of step changes withheld from a peer without the steps
-- capability, so they can all be resent once it upgrades.
ALTER TABLE sync_peers ADD COLUMN steps_resend_from INTEGER;
