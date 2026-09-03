FROM rust:1.98-alpine3.21 AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo fetch

COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp /app/target/release/server /app/server

FROM alpine:3.21

WORKDIR /app

COPY --from=builder /app/server /usr/local/bin/server

EXPOSE 4080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget -qO- http://127.0.0.1:4080/health >/dev/null || exit 1

CMD ["server"]