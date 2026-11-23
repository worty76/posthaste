-- Covering partial indexes for project listing and sidebar counts, so
-- Db::projects() and Db::counts_at() answer from compact index leaves
-- without scanning the notes table heap (content, details).
CREATE INDEX notes_live_by_category
    ON notes (category, status, created_at DESC)
    WHERE deleted_at IS NULL AND category IS NOT NULL;

CREATE INDEX notes_live_counts
    ON notes (status, later_at, created_at)
    WHERE deleted_at IS NULL;
