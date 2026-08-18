CREATE TABLE IF NOT EXISTS status_pages (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  visibility TEXT NOT NULL DEFAULT 'INTERNAL' CHECK (visibility IN ('INTERNAL','PUBLIC')),
  created_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS status_page_components (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  status_page_id UUID NOT NULL REFERENCES status_pages(id) ON DELETE CASCADE,
  site_id UUID NULL REFERENCES sites(id) ON DELETE CASCADE,
  service_id UUID NULL REFERENCES services(id) ON DELETE CASCADE,
  display_name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 100,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK ((site_id IS NOT NULL AND service_id IS NULL) OR (site_id IS NULL AND service_id IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS status_page_component_site_unique
  ON status_page_components(status_page_id, site_id)
  WHERE site_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS status_page_component_service_unique
  ON status_page_components(status_page_id, service_id)
  WHERE service_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS status_page_incidents (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  status_page_id UUID NOT NULL REFERENCES status_pages(id) ON DELETE CASCADE,
  incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
  public_title TEXT NOT NULL,
  public_message TEXT NOT NULL,
  is_published BOOLEAN NOT NULL DEFAULT TRUE,
  published_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(status_page_id, incident_id)
);

CREATE INDEX IF NOT EXISTS status_pages_project_idx ON status_pages(project_id, created_at);
CREATE INDEX IF NOT EXISTS status_page_components_page_idx ON status_page_components(status_page_id, sort_order, display_name);
CREATE INDEX IF NOT EXISTS status_page_incidents_page_idx ON status_page_incidents(status_page_id, is_published, published_at DESC);
