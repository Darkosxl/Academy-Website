# syntax=docker/dockerfile:1.7
# Artifact builder only. Production runs the outputs host-native under systemd; this image
# is never used as an outer container around rootless Podman.
FROM rust:1.95.0-slim-bookworm AS rust-builder
WORKDIR /source
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY services/academy/Cargo.toml ./services/academy/Cargo.toml
COPY services/academy/src ./services/academy/src
COPY services/academy/migrations ./services/academy/migrations
COPY services/academy/videos.dat ./services/academy/videos.dat
COPY services/benchmark-node/Cargo.toml ./services/benchmark-node/Cargo.toml
COPY services/benchmark-node/src ./services/benchmark-node/src
RUN cargo build --locked --release -p benchmark-node --bins

FROM python:3.12.11-slim-bookworm AS python-builder
WORKDIR /bundle
COPY services/benchmark-node/adapters/requirements.lock ./requirements.lock
RUN python -m pip wheel --require-hashes --no-deps \
      --wheel-dir /bundle/wheels -r /bundle/requirements.lock
COPY services/benchmark-node/adapters ./adapters
RUN python -m compileall -q /bundle/adapters

FROM debian:bookworm-slim AS packager
WORKDIR /artifacts
COPY --from=rust-builder /source/target/release/benchmark-controller ./bin/benchmark-controller
COPY --from=rust-builder /source/target/release/benchmark-executor ./bin/benchmark-executor
COPY --from=python-builder /bundle/adapters ./adapters
COPY --from=python-builder /bundle/requirements.lock ./python/requirements.lock
COPY --from=python-builder /bundle/wheels ./python/wheels
COPY services/benchmark-node/sandbox ./sandbox
COPY services/benchmark-node/systemd ./systemd
COPY infra/ec2/install-artifacts.sh infra/ec2/verify-isolation.sh ./infra/
RUN find . -type f ! -name SHA256SUMS -print0 \
      | sort -z | xargs -0 sha256sum > /artifacts/SHA256SUMS

FROM scratch AS artifacts
COPY --from=packager /artifacts /
LABEL org.opencontainers.image.title="Exposure benchmark-node deterministic artifacts"
