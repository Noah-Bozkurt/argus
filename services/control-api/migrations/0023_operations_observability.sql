ALTER TABLE commands ADD COLUMN IF NOT EXISTS phase TEXT NULL;
ALTER TABLE commands ADD COLUMN IF NOT EXISTS output TEXT NULL;
ALTER TABLE commands ADD COLUMN IF NOT EXISTS output_truncated BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE IF NOT EXISTS server_metric_samples (
  server_id UUID NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  captured_at TIMESTAMPTZ NOT NULL,
  cpu_percent REAL NOT NULL,
  ram_percent REAL NOT NULL,
  disk_percent REAL NOT NULL,
  load DOUBLE PRECISION NOT NULL,
  PRIMARY KEY (server_id, captured_at)
);

CREATE INDEX IF NOT EXISTS server_metric_samples_org_server_time_idx
  ON server_metric_samples (organization_id, server_id, captured_at DESC);

CREATE OR REPLACE FUNCTION argus_prune_server_metrics() RETURNS trigger AS $$
BEGIN
  IF MOD(EXTRACT(EPOCH FROM NEW.captured_at)::BIGINT, 3600) < 5 THEN
    DELETE FROM server_metric_samples
      WHERE server_id = NEW.server_id AND captured_at < NEW.captured_at - INTERVAL '30 days';
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS argus_prune_server_metrics_trigger ON server_metric_samples;
CREATE TRIGGER argus_prune_server_metrics_trigger
AFTER INSERT ON server_metric_samples
FOR EACH ROW EXECUTE FUNCTION argus_prune_server_metrics();
