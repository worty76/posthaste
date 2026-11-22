CREATE TABLE projects_v11 (
    name        TEXT PRIMARY KEY NOT NULL CHECK (length(trim(name)) > 0 AND length(name) <= 40),
    archived_at INTEGER,
    created_at  INTEGER NOT NULL,
    color       TEXT CHECK (color IS NULL OR (length(color) = 7 AND substr(color, 1, 1) = '#'))
) WITHOUT ROWID;

INSERT INTO projects_v11 (name, archived_at, created_at, color)
SELECT
    name,
    archived_at,
    created_at,
    CASE color
        WHEN '1' THEN '#e57373'
        WHEN '2' THEN '#e6b05c'
        WHEN '3' THEN '#74c28e'
        WHEN '4' THEN '#64b5f6'
        WHEN '5' THEN '#9a8cf2'
        WHEN '6' THEN '#d484b8'
        ELSE NULL
    END
FROM projects;

DROP TABLE projects;
ALTER TABLE projects_v11 RENAME TO projects;
