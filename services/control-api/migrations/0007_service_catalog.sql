ALTER TABLE services ALTER COLUMN environment_id DROP NOT NULL;
ALTER TABLE services ALTER COLUMN server_id DROP NOT NULL;
ALTER TABLE services ADD COLUMN IF NOT EXISTS repository_id UUID NULL REFERENCES project_repositories(id) ON DELETE SET NULL;
ALTER TABLE services ADD COLUMN IF NOT EXISTS description TEXT NOT NULL DEFAULT '';
ALTER TABLE services ADD COLUMN IF NOT EXISTS runtime TEXT NULL;
ALTER TABLE services ADD COLUMN IF NOT EXISTS owner_user_id UUID NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE services ADD COLUMN IF NOT EXISTS endpoint_url TEXT NULL;
ALTER TABLE services ADD COLUMN IF NOT EXISTS lifecycle_status TEXT NOT NULL DEFAULT 'ACTIVE';
ALTER TABLE services ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

CREATE INDEX IF NOT EXISTS services_project_updated_idx ON services(project_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS services_repository_idx ON services(repository_id) WHERE repository_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS services_environment_idx ON services(environment_id) WHERE environment_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS services_server_idx ON services(server_id) WHERE server_id IS NOT NULL;
