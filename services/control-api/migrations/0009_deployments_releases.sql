CREATE TABLE IF NOT EXISTS deployments (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  service_id UUID NOT NULL REFERENCES services(id) ON DELETE RESTRICT,
  environment_id UUID NOT NULL REFERENCES environments(id) ON DELETE RESTRICT,
  repository_id UUID NULL REFERENCES project_repositories(id) ON DELETE SET NULL,
  source_commit_sha TEXT NULL,
  source_version TEXT NULL,
  provider TEXT NOT NULL DEFAULT 'manual',
  status TEXT NOT NULL DEFAULT 'QUEUED' CHECK (status IN ('QUEUED','RUNNING','SUCCEEDED','FAILED','CANCELLED','ROLLED_BACK')),
  deployment_url TEXT NULL,
  error_summary TEXT NULL,
  notes TEXT NOT NULL DEFAULT '',
  previous_deployment_id UUID NULL REFERENCES deployments(id) ON DELETE SET NULL,
  rollback_of_deployment_id UUID NULL REFERENCES deployments(id) ON DELETE SET NULL,
  triggered_by UUID NOT NULL REFERENCES users(id),
  started_at TIMESTAMPTZ NULL,
  finished_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS releases (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  version TEXT NOT NULL,
  name TEXT NOT NULL,
  notes TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'DRAFT' CHECK (status IN ('DRAFT','READY','RELEASED','FAILED','ROLLED_BACK')),
  created_by UUID NOT NULL REFERENCES users(id),
  released_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(project_id, version)
);

CREATE TABLE IF NOT EXISTS release_components (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  release_id UUID NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
  service_id UUID NOT NULL REFERENCES services(id) ON DELETE RESTRICT,
  deployment_id UUID NULL REFERENCES deployments(id) ON DELETE SET NULL,
  version TEXT NULL,
  commit_sha TEXT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(release_id, service_id)
);

CREATE INDEX IF NOT EXISTS deployments_project_created_idx ON deployments(project_id, created_at DESC);
CREATE INDEX IF NOT EXISTS deployments_service_environment_idx ON deployments(service_id, environment_id, created_at DESC);
CREATE INDEX IF NOT EXISTS deployments_status_idx ON deployments(project_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS releases_project_created_idx ON releases(project_id, created_at DESC);
CREATE INDEX IF NOT EXISTS release_components_release_idx ON release_components(release_id, created_at);
