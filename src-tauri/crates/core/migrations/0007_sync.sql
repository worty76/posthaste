-- Device sync: per-field logical clocks, paired devices and local clock state.
CREATE TABLE sync_fields (
    note_id TEXT    NOT NULL,
    field   TEXT    NOT NULL,
    hlc     TEXT    NOT NULL,
    seq     INTEGER NOT NULL,
    PRIMARY KEY (note_id, field)
) WITHOUT ROWID;

CREATE INDEX sync_fields_by_seq ON sync_fields (seq);

CREATE TABLE sync_peers (
    device_id     TEXT PRIMARY KEY NOT NULL,
    name          TEXT NOT NULL,
    addrs         TEXT NOT NULL DEFAULT '[]',
    paired_at     INTEGER NOT NULL,
    last_sync_at  INTEGER,
    sent_up_to    INTEGER NOT NULL DEFAULT 0,
    received_hlc  TEXT
) WITHOUT ROWID;

CREATE TABLE sync_state (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) WITHOUT ROWID;
