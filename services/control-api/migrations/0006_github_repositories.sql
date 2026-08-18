CREATE TABLE IF NOT EXISTS project_repositories (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  provider TEXT NOT NULL CHECK (provider IN ('github')),
  owner TEXT NOT NULL,
  name TEXT NOT NULL,
  html_url TEXT NOT NULL,
  default_branch TEXT NOT NULL,
  visibility TEXT NOT NULL,
  snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
  sync_status TEXT NOT NULL DEFAULT 'PENDING' CHECK (sync_status IN ('PENDING','SYNCED','ERROR')),
  sync_error TEXT NULL,
  last_synced_at TIMESTAMPTZ NULL,
  created_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(project_id, provider, owner, name)
);

CREATE INDEX IF NOT EXISTS project_repositories_project_idx
  ON project_repositories(project_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS project_repositories_org_idx
  ON project_repositories(organization_id, provider, owner, name);
