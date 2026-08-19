CREATE OR REPLACE FUNCTION argus_create_default_content_project_sync_schedule()
RETURNS TRIGGER AS $$
BEGIN
  INSERT INTO job_schedules(
    id, organization_id, project_id, job_kind, resource_key, payload,
    interval_seconds, max_attempts, enabled, next_run_at, created_at, updated_at
  ) VALUES (
    gen_random_uuid(), NEW.id, NULL, 'content.projects.sync', 'default', '{}'::jsonb,
    300, 5, TRUE, NOW(), NOW(), NOW()
  )
  ON CONFLICT(organization_id, job_kind, resource_key) DO NOTHING;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS organizations_default_content_project_sync_schedule ON organizations;
CREATE TRIGGER organizations_default_content_project_sync_schedule
AFTER INSERT ON organizations
FOR EACH ROW EXECUTE FUNCTION argus_create_default_content_project_sync_schedule();

INSERT INTO job_schedules(
  id, organization_id, project_id, job_kind, resource_key, payload,
  interval_seconds, max_attempts, enabled, next_run_at, created_at, updated_at
)
SELECT
  gen_random_uuid(), o.id, NULL, 'content.projects.sync', 'default', '{}'::jsonb,
  300, 5, TRUE, NOW(), NOW(), NOW()
FROM organizations o
ON CONFLICT(organization_id, job_kind, resource_key) DO NOTHING;
