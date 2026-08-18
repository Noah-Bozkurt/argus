ALTER TABLE environments ADD COLUMN IF NOT EXISTS description TEXT NOT NULL DEFAULT '';
ALTER TABLE environments ADD COLUMN IF NOT EXISTS is_protected BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE environments ADD COLUMN IF NOT EXISTS sort_order INTEGER NOT NULL DEFAULT 100;
ALTER TABLE environments ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

UPDATE environments
SET is_protected = TRUE
WHERE LOWER(type) = 'production';

CREATE INDEX IF NOT EXISTS environments_project_sort_idx
  ON environments(project_id, sort_order, created_at);
