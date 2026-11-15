# The prebuilt cargo-chef image is rust:1.98-alpine3.21 with cargo-chef and
# musl-dev already installed; `cargo install` costs ~90 s on every cold build.
FROM lukemathwalker/cargo-chef:0.1.78-rust-1.98-alpine3.21 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

# Compile-time feature set; an empty default keeps the shipped image unseeded.
ARG CARGO_FEATURES=""

COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json ${CARGO_FEATURES:+--features "$CARGO_FEATURES"}
# Build application
COPY . .
RUN cargo build --release --bin server ${CARGO_FEATURES:+--features "$CARGO_FEATURES"}


FROM alpine:3.24 AS runtime

RUN apk add --no-cache ffmpeg=8.1.3-r0 \
    && addgroup -S -g 10001 mnemorium \
    && adduser -S -D -H -G mnemorium -u 10001 mnemorium \
    && mkdir -p /app \
    && chown mnemorium:mnemorium /app

WORKDIR /app
COPY --from=builder /app/target/release/server /usr/local/bin/server
COPY LICENSE THIRD_PARTY_NOTICES.txt /licenses/

EXPOSE 4080

# Exec form, with the shell kept for the explicit failure path.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD ["sh", "-c", "wget -qO- http://127.0.0.1:4080/health >/dev/null || exit 1"]

# Numeric ids, so no name lookup is needed at runtime.
USER 10001:10001

CMD ["/usr/local/bin/server"]
