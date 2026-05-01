# ── Stage 1: Build ──────────────────────────────────────────────
FROM rust:1.85-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconf

WORKDIR /app

# Cache dependencies by building a dummy project first.
COPY server/Cargo.toml server/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Now build the real binary with offline SQLx queries.
COPY server/src ./src
COPY server/migrations ./migrations
RUN cargo sqlx prepare --workspace --workspace-root /app
ENV SQLX_OFFLINE=true
RUN touch src/main.rs && cargo build --release

# ── Stage 2: Runtime ───────────────────────────────────────────
FROM alpine:3.21

RUN apk add --no-cache ca-certificates && \
    adduser -D -s /bin/sh appuser

COPY --from=builder /app/target/release/pickup-server /usr/local/bin/pickup-server

USER appuser
EXPOSE 8080
ENTRYPOINT ["pickup-server"]
