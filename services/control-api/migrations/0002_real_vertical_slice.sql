CREATE TABLE IF NOT EXISTS enrollment_tokens (
  token_hash TEXT PRIMARY KEY,
  server_id UUID NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  expires_at TIMESTAMPTZ NOT NULL,
  created_by UUID NULL REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS agents (
  server_id UUID PRIMARY KEY REFERENCES servers(id) ON DELETE CASCADE,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  agent_id UUID NOT NULL UNIQUE,
  credential_hash TEXT NOT NULL UNIQUE,
  agent_version TEXT NOT NULL,
  protocol_version TEXT NOT NULL,
  capabilities JSONB NOT NULL DEFAULT '[]'::jsonb,
  last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  snapshot JSONB NULL,
  services JSONB NOT NULL DEFAULT '[]'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE commands ADD COLUMN IF NOT EXISTS conflict_group TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE commands ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ NULL;
ALTER TABLE commands ADD COLUMN IF NOT EXISTS finished_at TIMESTAMPTZ NULL;
ALTER TABLE commands ADD COLUMN IF NOT EXISTS error_code TEXT NULL;
ALTER TABLE commands ADD COLUMN IF NOT EXISTS error_message TEXT NULL;
ALTER TABLE commands ADD COLUMN IF NOT EXISTS actor_user_id UUID NULL REFERENCES users(id);
CREATE INDEX IF NOT EXISTS commands_server_status_idx ON commands(server_id, status, created_at);
