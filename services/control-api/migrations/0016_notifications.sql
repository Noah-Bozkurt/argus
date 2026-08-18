CREATE TABLE IF NOT EXISTS notification_rules (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NULL REFERENCES projects(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  event_pattern TEXT NOT NULL,
  data_field TEXT NULL,
  data_value TEXT NULL,
  severity TEXT NOT NULL DEFAULT 'INFO' CHECK (severity IN ('INFO','WARNING','CRITICAL')),
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  created_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS notifications (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  rule_id UUID NOT NULL REFERENCES notification_rules(id) ON DELETE RESTRICT,
  source_event_id UUID NOT NULL,
  source_event_type TEXT NOT NULL,
  title TEXT NOT NULL,
  message TEXT NOT NULL,
  severity TEXT NOT NULL CHECK (severity IN ('INFO','WARNING','CRITICAL')),
  source_occurred_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(rule_id, source_event_id)
);

CREATE TABLE IF NOT EXISTS notification_user_state (
  notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  read_at TIMESTAMPTZ NULL,
  acknowledged_at TIMESTAMPTZ NULL,
  PRIMARY KEY(notification_id, user_id)
);

CREATE INDEX IF NOT EXISTS notification_rules_org_enabled_idx
  ON notification_rules(organization_id, enabled, updated_at DESC);
CREATE INDEX IF NOT EXISTS notifications_org_time_idx
  ON notifications(organization_id, source_occurred_at DESC);
CREATE INDEX IF NOT EXISTS notifications_project_time_idx
  ON notifications(project_id, source_occurred_at DESC);
CREATE INDEX IF NOT EXISTS notification_user_state_user_idx
  ON notification_user_state(user_id, read_at, acknowledged_at);
