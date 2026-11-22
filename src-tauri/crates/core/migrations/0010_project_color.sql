ALTER TABLE projects
    ADD COLUMN color TEXT CHECK (color IS NULL OR color IN ('1', '2', '3', '4', '5', '6'));
