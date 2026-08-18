CREATE TABLE IF NOT EXISTS sites (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  service_id UUID NULL REFERENCES services(id) ON DELETE SET NULL,
  repository_id UUID NULL REFERENCES project_repositories(id) ON DELETE SET NULL,
  environment_id UUID NULL REFERENCES environments(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  framework TEXT NULL,
  canonical_url TEXT NULL,
  lifecycle_status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (lifecycle_status IN ('ACTIVE','PAUSED','ARCHIVED')),
  health_status TEXT NOT NULL DEFAULT 'UNKNOWN',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS domains (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  site_id UUID NULL REFERENCES sites(id) ON DELETE RESTRICT,
  hostname TEXT NOT NULL,
  registrar TEXT NULL,
  dns_provider TEXT NULL,
  routing_mode TEXT NOT NULL DEFAULT 'DIRECT' CHECK (routing_mode IN ('DIRECT','CLOUDFLARE_PROXY','CLOUDFLARE_TUNNEL')),
  is_primary BOOLEAN NOT NULL DEFAULT FALSE,
  expires_at TIMESTAMPTZ NULL,
  tls_status TEXT NOT NULL DEFAULT 'UNKNOWN',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(organization_id, hostname)
);

CREATE UNIQUE INDEX IF NOT EXISTS domains_one_primary_per_site_idx
  ON domains(site_id)
  WHERE is_primary = TRUE AND site_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS sites_project_status_idx ON sites(project_id, lifecycle_status, updated_at DESC);
CREATE INDEX IF NOT EXISTS domains_project_hostname_idx ON domains(project_id, hostname);
CREATE INDEX IF NOT EXISTS domains_site_idx ON domains(site_id) WHERE site_id IS NOT NULL;
