CREATE TABLE IF NOT EXISTS site_incident_policies (
  site_id UUID PRIMARY KEY REFERENCES sites(id) ON DELETE CASCADE,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  enabled BOOLEAN NOT NULL DEFAULT FALSE,
  failure_threshold INTEGER NOT NULL DEFAULT 3 CHECK (failure_threshold BETWEEN 2 AND 10),
  severity TEXT NOT NULL DEFAULT 'MAJOR' CHECK (severity IN ('MINOR','MAJOR','CRITICAL')),
  configured_by UUID NOT NULL,
  active_incident_id UUID NULL REFERENCES incidents(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(organization_id, project_id, site_id)
);

CREATE INDEX IF NOT EXISTS site_incident_policies_project_idx
  ON site_incident_policies(organization_id, project_id, enabled);
