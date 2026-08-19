FROM rust:1.89-bookworm AS build

WORKDIR /src
COPY . .
RUN cargo build --locked --release -p argus-worker

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 argus \
    && useradd --system --uid 10001 --gid argus --home-dir /nonexistent argus

COPY --from=build /src/target/release/argus-worker /usr/local/bin/argus-worker

USER argus
ENTRYPOINT ["/usr/local/bin/argus-worker"]
