FROM rust:1.97-bookworm AS build

RUN apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add x86_64-unknown-linux-musl

WORKDIR /src

# Compile third-party dependencies in a layer that changes only when Cargo
# manifests change. The final build then recompiles only local Argus crates.
COPY Cargo.toml Cargo.lock LICENSE ./
COPY crates/agent/Cargo.toml crates/agent/Cargo.toml
COPY crates/cli/Cargo.toml crates/cli/Cargo.toml
COPY crates/common/Cargo.toml crates/common/Cargo.toml
COPY crates/helper/Cargo.toml crates/helper/Cargo.toml
COPY crates/protocol/Cargo.toml crates/protocol/Cargo.toml
COPY crates/system/Cargo.toml crates/system/Cargo.toml
COPY services/control-api/Cargo.toml services/control-api/Cargo.toml
COPY services/worker/Cargo.toml services/worker/Cargo.toml
RUN for package in crates/agent crates/cli crates/common crates/helper crates/protocol crates/system services/control-api services/worker; do \
      mkdir -p "$package/src"; \
      printf 'fn main() {}\n' >"$package/src/main.rs"; \
      printf '\n' >"$package/src/lib.rs"; \
    done \
    && printf 'fn main() {}\n' >crates/cli/src/installer.rs \
    && cargo build --locked --release --target x86_64-unknown-linux-musl -p agent -p helper -p cli

COPY crates crates
COPY scripts scripts
RUN find crates -type f -name '*.rs' -exec touch {} +
RUN cargo build --locked --release --target x86_64-unknown-linux-musl -p agent -p helper -p cli

FROM scratch AS artifact
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/argus-agent /out/argus-agent
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/argus-helper /out/argus-helper
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/argusctl /out/argusctl
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/argus-installer /out/argus-installer
COPY deploy/compose/compose.yaml /deploy/compose.yaml
COPY deploy/compose/Caddyfile.template /deploy/Caddyfile.template
COPY deploy/systemd/argus-agent.service /deploy/systemd/argus-agent.service
COPY deploy/systemd/argus-helper.service /deploy/systemd/argus-helper.service
CMD ["/out/argusctl", "version"]
