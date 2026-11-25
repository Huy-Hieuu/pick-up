# syntax=docker/dockerfile:1
FROM rust:1.98.1-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends cmake libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY server/Cargo.toml server/Cargo.lock ./
COPY server/src ./src
COPY server/migrations ./migrations
# Runtime SQLx queries need neither a build-time DB nor the sqlx CLI.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked && cp target/release/pickup-server /usr/local/bin/pickup-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home appuser
COPY --from=builder /usr/local/bin/pickup-server /usr/local/bin/pickup-server
USER appuser
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=30s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:8080/health/ready || exit 1
ENTRYPOINT ["pickup-server"]
