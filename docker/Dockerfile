# syntax=docker/dockerfile:1
# The Zentrale with its web UI and the pinned Syncthing for the hub.

FROM node:22-bookworm-slim AS web
WORKDIR /src
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY web/package.json web/
COPY e2e/package.json e2e/
RUN pnpm install --frozen-lockfile --filter web
COPY web web
RUN pnpm --filter web build

# cargo-chef splits the build: the dependencies are cooked from a recipe of the manifests alone,
# a layer the build cache keeps until Cargo.toml or Cargo.lock change in more than the
# workspace's own version (the recipe leaves that out, so a release does not change it); on a
# code change only the workspace crates compile.
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef --version 0.1.78 --locked
WORKDIR /src

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS zentrale
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --locked -p stellwerk-zentrale --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY --from=web /src/web/dist web/dist
RUN cargo build --release --locked -p stellwerk-zentrale
# Same pin and checksum check as the agents use (crates/engine/src/pin.rs)
RUN ./target/release/stellwerk fetch-syncthing /opt/syncthing

# The data directory, owned by the runtime user (distroless "nonroot", uid 65532).
RUN mkdir -p /data-skeleton

# Distroless: glibc, CA certificates and a non-root user; no shell, no package manager.
# No init needed: the Zentrale handles SIGTERM itself and waits for its Syncthing.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=zentrale /src/target/release/stellwerk /usr/local/bin/stellwerk
COPY --from=zentrale /opt/syncthing /opt/syncthing
COPY --from=zentrale --chown=65532:65532 /data-skeleton /data
COPY LICENSE NOTICE /usr/share/doc/stellwerk/

ENV STELLWERK_DATA=/data \
    STELLWERK_LISTEN=0.0.0.0:8080 \
    STELLWERK_HUB_LISTEN=tcp://0.0.0.0:22001 \
    STELLWERK_SYNCTHING=/opt/syncthing/syncthing
VOLUME /data
EXPOSE 8080 22001
ENTRYPOINT ["/usr/local/bin/stellwerk"]
