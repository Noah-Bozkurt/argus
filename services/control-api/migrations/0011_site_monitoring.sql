CREATE TABLE IF NOT EXISTS site_monitor_configs (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  site_id UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  target_url TEXT NOT NULL,
  check_robots BOOLEAN NOT NULL DEFAULT TRUE,
  check_sitemap BOOLEAN NOT NULL DEFAULT TRUE,
  timeout_seconds INTEGER NOT NULL DEFAULT 10 CHECK (timeout_seconds BETWEEN 2 AND 30),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(site_id)
);

CREATE TABLE IF NOT EXISTS site_monitor_checks (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  site_id UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  config_id UUID NOT NULL REFERENCES site_monitor_configs(id) ON DELETE CASCADE,
  overall_status TEXT NOT NULL CHECK (overall_status IN ('HEALTHY','DEGRADED','DOWN','ERROR')),
  target_url TEXT NOT NULL,
  resolved_ips JSONB NOT NULL DEFAULT '[]'::jsonb,
  dns_ok BOOLEAN NOT NULL,
  http_status INTEGER NULL,
  http_latency_ms BIGINT NULL,
  tls_status TEXT NOT NULL,
  robots_status INTEGER NULL,
  sitemap_status INTEGER NULL,
  error_code TEXT NULL,
  error_message TEXT NULL,
  checked_by UUID NOT NULL REFERENCES users(id),
  checked_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS site_monitor_configs_project_idx ON site_monitor_configs(project_id, site_id);
CREATE INDEX IF NOT EXISTS site_monitor_checks_site_time_idx ON site_monitor_checks(site_id, checked_at DESC);
CREATE INDEX IF NOT EXISTS site_monitor_checks_project_time_idx ON site_monitor_checks(project_id, checked_at DESC);
