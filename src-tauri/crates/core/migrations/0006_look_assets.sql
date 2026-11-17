-- Look image assets, content-addressed by the SHA-256 of their bytes. One
-- row per stored image, whatever looks reference it; a look's document
-- lists the ids it uses in its own `assets` field.
CREATE TABLE look_assets (
  id TEXT PRIMARY KEY NOT NULL,
  mime TEXT NOT NULL,
  size INTEGER NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  data BLOB NOT NULL,
  created_at INTEGER NOT NULL
) WITHOUT ROWID;
