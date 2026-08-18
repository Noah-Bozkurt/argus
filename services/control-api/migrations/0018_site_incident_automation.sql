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

CREATE OR REPLACE FUNCTION argus_enqueue_site_incident_evaluation()
RETURNS TRIGGER AS $$
BEGIN
  INSERT INTO background_jobs(
    id,organization_id,project_id,schedule_id,job_kind,resource_key,payload,dedupe_key,
    status,run_at,attempts,max_attempts,created_at,updated_at
  )
  SELECT
    gen_random_uuid(),NEW.organization_id,NEW.project_id,NULL,'site_incident.evaluate',NEW.site_id::text,
    jsonb_build_object('site_id',NEW.site_id,'check_id',NEW.id),
    'site-incident-evaluate:' || NEW.id::text,
    'QUEUED',NOW(),0,5,NOW(),NOW()
  FROM site_incident_policies p
  WHERE p.site_id=NEW.site_id
    AND p.organization_id=NEW.organization_id
    AND p.project_id=NEW.project_id
    AND p.enabled=TRUE
  ON CONFLICT(dedupe_key) DO NOTHING;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS site_monitor_checks_enqueue_incident_evaluation ON site_monitor_checks;
CREATE TRIGGER site_monitor_checks_enqueue_incident_evaluation
AFTER INSERT ON site_monitor_checks
FOR EACH ROW EXECUTE FUNCTION argus_enqueue_site_incident_evaluation();
