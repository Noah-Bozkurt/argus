# Security Model

## Trust boundaries

- **Web** is unprivileged and never executes privileged system commands.
- **Control API** enforces authorization and command policy.
- **Agent** is authenticated and executes typed operations only.
- **Helper** is privileged but local-only and allowlisted.

## Compromise impact

- If web is compromised: attacker still cannot directly run root commands.
- If control API is compromised: attacker can queue commands, but helper allowlist still limits operation class.
- If agent is compromised: helper boundary still prevents arbitrary command execution by API design.
- If helper is compromised: host privilege is at risk; helper surface is intentionally minimal and non-networked.

## Implemented controls

- Enrollment tokens are short-lived and one-time.
- Permanent identity is distinct from enrollment token.
- Server-side authorization for command requests.
- Command TTL and expiry enforcement.
- Idempotency keys to prevent duplicate execution.
- Conflict checks for mutually exclusive operations.
- Structured audit/event records.
- No secrets in API responses by default.
