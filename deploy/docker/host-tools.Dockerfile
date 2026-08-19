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
CMD ["/out/argusctl", "version"]
