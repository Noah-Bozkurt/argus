CREATE TABLE compose_stacks (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    environment_id UUID NOT NULL REFERENCES environments(id),
    server_id UUID NOT NULL REFERENCES servers(id),
    name TEXT NOT NULL,
    compose_project_name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    lifecycle_status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (lifecycle_status IN ('ACTIVE','PAUSED','ARCHIVED')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX compose_stacks_server_project_name_unique
    ON compose_stacks (organization_id, server_id, LOWER(compose_project_name));

CREATE INDEX compose_stacks_project_idx
    ON compose_stacks (organization_id, project_id, created_at);

CREATE INDEX compose_stacks_server_idx
    ON compose_stacks (organization_id, server_id);
