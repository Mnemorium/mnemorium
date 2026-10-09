FROM rust:1.98-alpine3.21 AS chef

RUN apk add --no-cache musl-dev=1.2.5-r11 \
    && cargo install --locked cargo-chef --version 0.1.78
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin server


FROM alpine:3.21 AS runtime

RUN apk add --no-cache ffmpeg=6.1.2-r1 \
    && addgroup -S mnemorium \
    && adduser -S -D -H -G mnemorium mnemorium \
    && mkdir -p /app \
    && chown mnemorium:mnemorium /app

WORKDIR /app
COPY --from=builder /app/target/release/server /usr/local/bin/server
COPY LICENSE THIRD_PARTY_NOTICES.txt /licenses/

EXPOSE 4080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget -qO- http://127.0.0.1:4080/health >/dev/null || exit 1

USER mnemorium

CMD ["/usr/local/bin/server"]
