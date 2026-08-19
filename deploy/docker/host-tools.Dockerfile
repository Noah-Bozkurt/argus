FROM rust:1.89-bookworm AS build

RUN apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add x86_64-unknown-linux-musl

WORKDIR /src
COPY . .
RUN cargo build --locked --release --target x86_64-unknown-linux-musl -p agent -p helper -p cli

FROM scratch AS artifact
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/argus-agent /out/argus-agent
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/argus-helper /out/argus-helper
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/argusctl /out/argusctl
COPY deploy/compose/compose.yaml /deploy/compose.yaml
COPY deploy/compose/Caddyfile.template /deploy/Caddyfile.template
COPY deploy/systemd/argus-agent.service /deploy/systemd/argus-agent.service
COPY deploy/systemd/argus-helper.service /deploy/systemd/argus-helper.service
CMD ["/out/argusctl", "version"]
