CREATE TABLE projects (
    name        TEXT PRIMARY KEY NOT NULL CHECK (length(trim(name)) > 0 AND length(name) <= 40),
    archived_at INTEGER,
    created_at  INTEGER NOT NULL
) WITHOUT ROWID;

INSERT OR IGNORE INTO projects (name, created_at)
    SELECT DISTINCT category, MIN(created_at) FROM notes
    WHERE category IS NOT NULL AND deleted_at IS NULL GROUP BY category;
