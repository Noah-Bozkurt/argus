FROM rust:1.97-bookworm AS build

WORKDIR /src

# Compile third-party dependencies in a layer that changes only when Cargo
# manifests change. Real workspace sources are copied afterward.
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
    && cargo build --locked --release -p control-api -p argus-worker

COPY crates crates
COPY services/control-api services/control-api
RUN find crates services/control-api -type f -name '*.rs' -exec touch {} +
RUN cargo build --locked --release -p control-api

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 argus \
    && useradd --system --uid 10001 --gid argus --home-dir /nonexistent argus

COPY --from=build /src/target/release/control-api /usr/local/bin/argus-control-api

USER argus
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/argus-control-api"]
