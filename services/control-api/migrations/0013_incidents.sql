CREATE TABLE IF NOT EXISTS incidents (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  summary TEXT NOT NULL DEFAULT '',
  severity TEXT NOT NULL CHECK (severity IN ('MINOR','MAJOR','CRITICAL')),
  status TEXT NOT NULL DEFAULT 'INVESTIGATING' CHECK (status IN ('INVESTIGATING','IDENTIFIED','MONITORING','RESOLVED')),
  source_type TEXT NOT NULL CHECK (source_type IN ('SERVICE','SITE','DOMAIN','SERVER','ENVIRONMENT','REPOSITORY')),
  source_id UUID NOT NULL,
  source_name TEXT NOT NULL,
  created_by UUID NOT NULL REFERENCES users(id),
  started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  resolved_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS incident_affected_resources (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
  resource_type TEXT NOT NULL,
  resource_id UUID NOT NULL,
  resource_name TEXT NOT NULL,
  distance INTEGER NOT NULL CHECK (distance > 0),
  impact_path JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(incident_id, resource_type, resource_id)
);

CREATE TABLE IF NOT EXISTS incident_timeline (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
  event_type TEXT NOT NULL CHECK (event_type IN ('CREATED','STATUS_CHANGED','NOTE')),
  message TEXT NOT NULL,
  data JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS incidents_project_status_idx ON incidents(project_id, status, started_at DESC);
CREATE INDEX IF NOT EXISTS incident_affected_incident_idx ON incident_affected_resources(incident_id, distance, resource_type);
CREATE INDEX IF NOT EXISTS incident_timeline_incident_idx ON incident_timeline(incident_id, created_at);
