# ---- builder ----
FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# cache deps
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# real source
COPY src ./src
COPY migrations ./migrations
COPY videos.dat ./
RUN touch src/main.rs && cargo build --release

# ---- runtime ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 app
WORKDIR /app
COPY --from=builder /app/target/release/academy ./academy
COPY static ./static

USER app
ENV BIND=0.0.0.0:3000
EXPOSE 3000
CMD ["./academy"]
