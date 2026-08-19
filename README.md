# Argus

Argus is a private platform for keeping projects, infrastructure and the software running on it in one place.

It is designed around **projects**, not customers. A project can be personal, experimental, open-source, infrastructure-only or connected to a client. Client features are an optional layer rather than a requirement for the rest of the platform.

Argus brings together the things that otherwise tend to end up spread across hosting dashboards, SSH sessions, deployment tools, monitoring services, status pages and content systems: projects, servers, services, releases, websites, domains, monitoring, incidents, recovery and application/content data.

## Why “Argus”?

The name is inspired by **Argus Panoptes**, the many-eyed watchman from Greek mythology. The idea fits the project: Argus keeps watch over many different parts of a project and gives them one shared view instead of treating each system as a separate island.

## Status

Argus is under active development and is not yet considered production-ready. A substantial control-plane, operations and Payload-based content foundation is implemented.

The repository now contains the first reproducible single-server deployment path for Ubuntu/Debian amd64: a Compose-based control plane, native Agent/Helper services, a rerunnable installer, a disposable-host reset path, and CI readiness checks for clean database startup and production application builds. Custom Argus images are designed to publish from `main` only after the normal CI workflow succeeds.

The next milestone is the first real installation on a clean test server. Every manual workaround found during that test should be treated as an installer or product bug rather than becoming undocumented setup knowledge. See the [Roadmap](docs/roadmap.md) for the exact test sequence and current limitations.

## Documentation

The README intentionally stays high-level. Implementation details, operating rules and current limitations live in the documentation:

- [Documentation index](docs/README.md)
- [Architecture](docs/architecture.md)
- [Projects & delivery](docs/projects-and-delivery.md)
- [Operations](docs/operations.md)
- [Security & recovery](docs/security-and-recovery.md)
- [Content platform](docs/content-platform.md)
- [Development](docs/development.md)
- [Roadmap](docs/roadmap.md)

## License

Argus is **proprietary, closed-source software**. The repository is private and the project is not licensed for public use, copying, modification or redistribution. See [LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
