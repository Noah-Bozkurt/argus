# Logs & Diagnostics

This phase adds bounded read-only diagnostics without introducing a browser terminal or arbitrary shell execution.

## Diagnostics snapshot

The agent refreshes diagnostics every 60 seconds and sends them through the existing authenticated heartbeat. The snapshot contains:

- failed systemd service units;
- listening TCP port numbers;
- recent journald tails for explicitly managed/allowlisted services.

The normal 5-second heartbeat reuses the cached diagnostics so systemctl, ss and journalctl are not run on every heartbeat.

## Journald safety

Journal reads travel through the privileged helper Unix socket. The helper:

- only accepts service names that pass the existing service-name validation;
- only reads services in the configured Argus allowlist;
- invokes `journalctl` directly without a shell;
- allows at most 500 requested lines;
- truncates captured output at 64 KiB.

The periodic diagnostic snapshot requests 50 lines per managed service.

## Protocol

Protocol version is 1.2 and advertises `logs.journal.v1`.

A typed `logs.journal` command exists for future on-demand refreshes, but the current UI uses the periodic persisted diagnostic snapshot rather than creating a terminal-like log stream.

## Reboot result

`system.reboot` now returns `UNKNOWN` after systemd accepts the request instead of claiming immediate success. This accurately represents that the control plane still needs to observe the server disappear and reconnect. A subsequent reliability slice will persist the pre-reboot boot identity/uptime and automatically verify the reboot after reconnect.

## Non-goals

This phase does not add:

- arbitrary process control;
- an interactive shell;
- unlimited log streaming;
- full observability/time-series storage;
- arbitrary journald units outside the managed service allowlist.
