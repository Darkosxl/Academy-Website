# Exposure Academy Verified. Build context must be the REPO ROOT (this is a
# workspace member, same as the root Dockerfile does for academy) — see
# compose.dokploy.yml's `context: ../..`.
FROM rust:1.95.0-slim-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY services ./services
RUN cargo build --locked --release -p verified

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 app
WORKDIR /app
COPY --from=builder /app/target/release/verified ./verified

USER app
ENV BIND=0.0.0.0:3100
EXPOSE 3100
CMD ["./verified"]
