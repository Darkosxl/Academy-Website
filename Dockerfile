# Backward-compatible Academy image. Build from the repository root.
FROM rust:1.95.0-slim-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY services ./services
RUN cargo build --locked --release -p academy

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 app
WORKDIR /app
COPY --from=builder /app/target/release/academy ./academy
# CARGO_MANIFEST_DIR is embedded in the binary, so preserve its build-time asset path.
COPY services/academy/static ./services/academy/static

USER app
ENV BIND=0.0.0.0:3000
EXPOSE 3000
CMD ["./academy"]
