# Jobs / Worker Foundation V1

Argus background work uses PostgreSQL as the first queue and scheduler. The worker is a separate long-running Rust process; Next.js does not own recurring work and Agents do not act as SaaS schedulers.

## Components

```text
PostgreSQL
  ├── job_schedules
  └── background_jobs
          ↓ claim with SKIP LOCKED
     argus-worker
          ↓ authenticated internal request
     Rust Control API
          ↓ typed domain operation
     Notifications / later Monitoring / other modules
```

## Why PostgreSQL first

V1 does not need Redis, Kafka, RabbitMQ or a dedicated workflow engine. PostgreSQL already provides durable storage, transactions, row locks and `FOR UPDATE SKIP LOCKED`, which are sufficient for Argus's initial operational workload.

A separate queue technology should only be introduced when measured throughput or latency requires it.

## Schedules

`job_schedules` stores:

- Organization and optional Project scope;
- typed job kind;
- resource key;
- JSON payload;
- interval in seconds;
- maximum attempts;
- enabled state;
- next and last enqueue timestamps.

V1 intervals are bounded to 60 seconds through 24 hours.

Missed schedule ticks are not replayed in a burst after downtime. After enqueueing, the next run moves to at least one full interval after the current time.

## Queue

`background_jobs` stores each concrete execution attempt with:

- job kind and payload;
- schedule provenance;
- QUEUED / RUNNING / SUCCEEDED / DEAD state;
- run time;
- attempt count and maximum attempts;
- lease owner and expiry;
- bounded error code/message;
- completion timestamps.

A schedule tick receives a deterministic dedupe key, so concurrent workers cannot enqueue the same scheduled execution twice.

## Claiming and leases

Workers claim one due job through `FOR UPDATE SKIP LOCKED` and atomically set:

- RUNNING;
- incremented attempt count;
- worker-specific lease owner;
- lease expiration.

Multiple worker processes can therefore compete safely without a leader election service.

If a worker dies while executing a job, another worker may reclaim it after the lease expires. This means job handlers must remain idempotent.

## Retries and dead jobs

Failed jobs return to QUEUED with exponential backoff, capped at five minutes. When the configured maximum attempt count is reached, the job becomes DEAD and is no longer claimed automatically.

V1 does not automatically discard dead jobs or loop forever.

## Execution boundary

The worker does not duplicate domain logic. It sends a typed job request to:

`POST /internal/jobs/execute`

The endpoint requires `ARGUS_WORKER_TOKEN`. If the Control API has no worker token configured, the internal worker API is effectively disabled and returns a service-unavailable response.

The worker itself requires the same token plus `DATABASE_URL` and `ARGUS_CONTROL_API_URL`.

## First job kind

### `notifications.materialize`

Every Organization receives one default 60-second schedule. The Control API invokes the existing Notification materializer for that Organization using a system/worker audit actor.

Notification deduplication remains `(rule_id, source_event_id)`, so retries and overlapping workers cannot create duplicate inbox items.

The existing manual `Refresh from events` action remains useful for debugging and immediate operator-triggered refreshes.

## Security

- internal execution is bearer-token authenticated separately from browser/backend credentials;
- the worker does not receive arbitrary shell commands;
- job kind is allowlisted by the Control API;
- payloads are typed/validated by the owning handler;
- the worker has no Agent credential and cannot issue privileged host operations directly;
- raw logs, secrets and arbitrary command output are not copied into queue records.

## Configuration

Worker:

- `DATABASE_URL` — required PostgreSQL database;
- `ARGUS_CONTROL_API_URL` — defaults to `http://127.0.0.1:8080`;
- `ARGUS_WORKER_TOKEN` — required, minimum 32 characters;
- `ARGUS_WORKER_POLL_SECONDS` — 1-60, default 2;
- `ARGUS_WORKER_LEASE_SECONDS` — 15-900, default 60.

Control API:

- `ARGUS_WORKER_TOKEN` — optional; internal job execution is disabled when absent.

## Non-goals

V1 does not implement:

- Redis/Kafka/RabbitMQ;
- arbitrary user-defined code execution;
- cron expressions;
- workflow DAGs;
- parallel execution inside one worker process;
- a browser job administration console;
- automatic dead-letter replay;
- Site Monitoring schedules yet.

## Next slice

Site Monitoring scheduling should reuse `job_schedules` with typed `site_monitor.check` jobs. Monitor config can then expose enabled + interval controls, while the existing SSRF-safe probe remains in the Control API rather than moving into the worker.

## Validation gate

Merge only after Rust workspace tests, `cargo fmt --all -- --check`, and the web TypeScript check pass, with no temporary workflow files left on the branch.
