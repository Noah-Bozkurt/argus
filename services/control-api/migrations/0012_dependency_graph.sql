CREATE TABLE IF NOT EXISTS resource_dependencies (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  source_type TEXT NOT NULL CHECK (source_type IN ('SERVICE','SITE','DOMAIN','SERVER','ENVIRONMENT','REPOSITORY')),
  source_id UUID NOT NULL,
  target_type TEXT NOT NULL CHECK (target_type IN ('SERVICE','SITE','DOMAIN','SERVER','ENVIRONMENT','REPOSITORY')),
  target_id UUID NOT NULL,
  relationship TEXT NOT NULL CHECK (relationship IN ('DEPENDS_ON','USES')),
  created_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (NOT (source_type = target_type AND source_id = target_id)),
  UNIQUE(project_id, source_type, source_id, target_type, target_id, relationship)
);

CREATE INDEX IF NOT EXISTS resource_dependencies_project_idx ON resource_dependencies(project_id, created_at);
CREATE INDEX IF NOT EXISTS resource_dependencies_target_idx ON resource_dependencies(project_id, target_type, target_id);

CREATE OR REPLACE FUNCTION argus_cleanup_resource_dependencies()
RETURNS TRIGGER AS $$
BEGIN
  DELETE FROM resource_dependencies
  WHERE organization_id = OLD.organization_id
    AND project_id = OLD.project_id
    AND ((source_type = TG_ARGV[0] AND source_id = OLD.id)
      OR (target_type = TG_ARGV[0] AND target_id = OLD.id));
  RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS argus_cleanup_service_dependencies ON services;
CREATE TRIGGER argus_cleanup_service_dependencies
AFTER DELETE ON services
FOR EACH ROW EXECUTE FUNCTION argus_cleanup_resource_dependencies('SERVICE');

DROP TRIGGER IF EXISTS argus_cleanup_site_dependencies ON sites;
CREATE TRIGGER argus_cleanup_site_dependencies
AFTER DELETE ON sites
FOR EACH ROW EXECUTE FUNCTION argus_cleanup_resource_dependencies('SITE');

DROP TRIGGER IF EXISTS argus_cleanup_domain_dependencies ON domains;
CREATE TRIGGER argus_cleanup_domain_dependencies
AFTER DELETE ON domains
FOR EACH ROW EXECUTE FUNCTION argus_cleanup_resource_dependencies('DOMAIN');

DROP TRIGGER IF EXISTS argus_cleanup_server_dependencies ON servers;
CREATE TRIGGER argus_cleanup_server_dependencies
AFTER DELETE ON servers
FOR EACH ROW EXECUTE FUNCTION argus_cleanup_resource_dependencies('SERVER');

DROP TRIGGER IF EXISTS argus_cleanup_environment_dependencies ON environments;
CREATE TRIGGER argus_cleanup_environment_dependencies
AFTER DELETE ON environments
FOR EACH ROW EXECUTE FUNCTION argus_cleanup_resource_dependencies('ENVIRONMENT');

DROP TRIGGER IF EXISTS argus_cleanup_repository_dependencies ON project_repositories;
CREATE TRIGGER argus_cleanup_repository_dependencies
AFTER DELETE ON project_repositories
FOR EACH ROW EXECUTE FUNCTION argus_cleanup_resource_dependencies('REPOSITORY');
