CREATE TABLE IF NOT EXISTS domain_lifecycle_states (
  domain_id UUID PRIMARY KEY REFERENCES domains(id) ON DELETE CASCADE,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  expiration_status TEXT NOT NULL CHECK (expiration_status IN ('UNKNOWN','OK','WARNING','CRITICAL','EXPIRED')),
  tls_status TEXT NOT NULL CHECK (tls_status IN ('UNKNOWN','VALID','FAILED','STALE')),
  overall_status TEXT NOT NULL CHECK (overall_status IN ('OK','ATTENTION','CRITICAL','UNKNOWN')),
  days_until_expiry INTEGER NULL,
  last_evaluated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(organization_id, project_id, domain_id)
);

CREATE INDEX IF NOT EXISTS domain_lifecycle_states_project_idx
  ON domain_lifecycle_states(organization_id, project_id, overall_status, changed_at DESC);

CREATE OR REPLACE FUNCTION argus_create_default_domain_lifecycle_schedule()
RETURNS TRIGGER AS $$
BEGIN
  INSERT INTO job_schedules(
    id,organization_id,project_id,job_kind,resource_key,payload,
    interval_seconds,max_attempts,enabled,next_run_at,created_at,updated_at
  ) VALUES (
    gen_random_uuid(),NEW.id,NULL,'domains.lifecycle_evaluate','default','{}'::jsonb,
    21600,5,TRUE,NOW(),NOW(),NOW()
  )
  ON CONFLICT(organization_id,job_kind,resource_key) DO NOTHING;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS organizations_default_domain_lifecycle_schedule ON organizations;
CREATE TRIGGER organizations_default_domain_lifecycle_schedule
AFTER INSERT ON organizations
FOR EACH ROW EXECUTE FUNCTION argus_create_default_domain_lifecycle_schedule();

INSERT INTO job_schedules(
  id,organization_id,project_id,job_kind,resource_key,payload,
  interval_seconds,max_attempts,enabled,next_run_at,created_at,updated_at
)
SELECT
  gen_random_uuid(),o.id,NULL,'domains.lifecycle_evaluate','default','{}'::jsonb,
  21600,5,TRUE,NOW(),NOW(),NOW()
FROM organizations o
ON CONFLICT(organization_id,job_kind,resource_key) DO NOTHING;
