CREATE TABLE IF NOT EXISTS job_schedules (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NULL REFERENCES projects(id) ON DELETE CASCADE,
  job_kind TEXT NOT NULL,
  resource_key TEXT NOT NULL DEFAULT '',
  payload JSONB NOT NULL DEFAULT '{}'::jsonb,
  interval_seconds INTEGER NOT NULL CHECK (interval_seconds BETWEEN 60 AND 86400),
  max_attempts INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts BETWEEN 1 AND 20),
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  next_run_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  last_enqueued_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(organization_id, job_kind, resource_key)
);

CREATE TABLE IF NOT EXISTS background_jobs (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NULL REFERENCES projects(id) ON DELETE CASCADE,
  schedule_id UUID NULL REFERENCES job_schedules(id) ON DELETE SET NULL,
  job_kind TEXT NOT NULL,
  resource_key TEXT NOT NULL DEFAULT '',
  payload JSONB NOT NULL DEFAULT '{}'::jsonb,
  dedupe_key TEXT NULL,
  status TEXT NOT NULL DEFAULT 'QUEUED' CHECK (status IN ('QUEUED','RUNNING','SUCCEEDED','DEAD')),
  run_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  max_attempts INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts BETWEEN 1 AND 20),
  lease_owner TEXT NULL,
  lease_expires_at TIMESTAMPTZ NULL,
  last_error_code TEXT NULL,
  last_error_message TEXT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  completed_at TIMESTAMPTZ NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS background_jobs_dedupe_idx
  ON background_jobs(dedupe_key)
  WHERE dedupe_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS background_jobs_claim_idx
  ON background_jobs(status, run_at, created_at);
CREATE INDEX IF NOT EXISTS background_jobs_org_time_idx
  ON background_jobs(organization_id, created_at DESC);
CREATE INDEX IF NOT EXISTS job_schedules_due_idx
  ON job_schedules(enabled, next_run_at);

CREATE OR REPLACE FUNCTION argus_create_default_notification_schedule()
RETURNS TRIGGER AS $$
BEGIN
  INSERT INTO job_schedules(
    id, organization_id, project_id, job_kind, resource_key, payload,
    interval_seconds, max_attempts, enabled, next_run_at, created_at, updated_at
  ) VALUES (
    gen_random_uuid(), NEW.id, NULL, 'notifications.materialize', 'default', '{}'::jsonb,
    60, 5, TRUE, NOW(), NOW(), NOW()
  )
  ON CONFLICT(organization_id, job_kind, resource_key) DO NOTHING;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS organizations_default_notification_schedule ON organizations;
CREATE TRIGGER organizations_default_notification_schedule
AFTER INSERT ON organizations
FOR EACH ROW EXECUTE FUNCTION argus_create_default_notification_schedule();

INSERT INTO job_schedules(
  id, organization_id, project_id, job_kind, resource_key, payload,
  interval_seconds, max_attempts, enabled, next_run_at, created_at, updated_at
)
SELECT
  gen_random_uuid(), o.id, NULL, 'notifications.materialize', 'default', '{}'::jsonb,
  60, 5, TRUE, NOW(), NOW(), NOW()
FROM organizations o
ON CONFLICT(organization_id, job_kind, resource_key) DO NOTHING;
