-- Existing projects retain their navigation until explicitly configured.
ALTER TABLE projects ADD COLUMN tools JSONB NOT NULL DEFAULT '["content","forms","deployments","domains","infrastructure","monitoring","work"]'::jsonb;
ALTER TABLE projects ADD CONSTRAINT projects_tools_array CHECK (jsonb_typeof(tools) = 'array');
