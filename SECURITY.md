# Security Policy

## Supported versions

Argus is under active development and has not reached a stable production release. Security fixes are applied to the current `main` line and the most recent published image set. Older revisions are not guaranteed to receive backports.

## Reporting a vulnerability

Do not open a public issue, discussion, or pull request for a suspected vulnerability.

Use GitHub's private vulnerability reporting for this repository:

**https://github.com/Noah-Bozkurt/argus/security/advisories/new**

Include, when available:

- affected revision, component, and deployment mode;
- impact and required attacker access;
- reproducible steps or a minimal proof of concept;
- relevant logs with secrets, domains, tokens, IDs, and host data redacted;
- any suggested mitigation or evidence of active exploitation.

You should receive an acknowledgement through the advisory within 7 days. Triage, remediation, disclosure timing, and credit will be coordinated in the private advisory. Please allow a reasonable remediation window before disclosure.

## Security expectations

- Never submit real credentials or production data.
- Do not test against infrastructure you do not own or have explicit permission to assess.
- Avoid destructive testing and privacy-impacting data access.
- Preserve confidentiality until a coordinated disclosure is published.

For the current trust model and known limitations, see [`docs/security-and-recovery.md`](docs/security-and-recovery.md) and [`docs/authentication.md`](docs/authentication.md).
