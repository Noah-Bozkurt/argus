CREATE TABLE maintenance_windows (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    server_id UUID NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ NOT NULL,
    reason TEXT NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMPTZ NULL,
    CHECK (ends_at > starts_at),
    CHECK (char_length(reason) BETWEEN 1 AND 500)
);

CREATE INDEX maintenance_windows_server_time_idx
    ON maintenance_windows(server_id, starts_at DESC);

CREATE INDEX maintenance_windows_active_idx
    ON maintenance_windows(server_id, ends_at)
    WHERE ended_at IS NULL;
