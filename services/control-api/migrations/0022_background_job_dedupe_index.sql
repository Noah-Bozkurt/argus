-- PostgreSQL cannot infer a partial unique index for
-- ON CONFLICT(dedupe_key) unless the query repeats the index predicate.
-- A normal unique index still permits multiple NULL values and can therefore
-- enforce the intended non-NULL deduplication without special query syntax.
DROP INDEX IF EXISTS background_jobs_dedupe_idx;

CREATE UNIQUE INDEX background_jobs_dedupe_idx
  ON background_jobs(dedupe_key);
