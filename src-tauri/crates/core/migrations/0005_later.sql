-- Later: a note parked out of Today and the inbox without being finished.
-- Only meaningful while status = 'inbox'; completing or archiving clears it.
-- There is no date to come back on: the person brings it back themselves.
ALTER TABLE notes ADD COLUMN later_at INTEGER;

CREATE INDEX notes_later ON notes (later_at) WHERE later_at IS NOT NULL AND deleted_at IS NULL;
