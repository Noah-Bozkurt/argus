CREATE TABLE IF NOT EXISTS server_policies (
    server_id UUID PRIMARY KEY REFERENCES servers(id) ON DELETE CASCADE,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    mode TEXT NOT NULL DEFAULT 'MONITOR' CHECK (mode IN ('MONITOR','ENFORCE')),
    firewall_enabled BOOLEAN,
    ssh_password_auth BOOLEAN,
    ssh_root_login TEXT,
    automatic_security_updates BOOLEAN,
    updated_by UUID REFERENCES users(id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_server_policies_org ON server_policies(organization_id);
