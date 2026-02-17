FROM rust:1.88-alpine3.22 AS base-builder
RUN apk add --no-cache \
    alpine-sdk \
    pkgconfig \
    cmake \
    musl-dev

FROM base-builder AS planner
WORKDIR /app
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM base-builder AS cacher
WORKDIR /app
RUN cargo install cargo-chef
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin astionicbot --target x86_64-unknown-linux-musl

FROM base-builder AS builder
WORKDIR /app
COPY . .
COPY --from=cacher /app/target /app/target
RUN cargo build --release --bin astionicbot --target x86_64-unknown-linux-musl


FROM alpine:3.22 AS runtime
LABEL type="AstionicBotRs"

# Install yt-dlp from PyPI rather than apk -- the apk package lags months behind
# and won't have the latest YouTube signature/n-challenge fixes.
RUN apk add --no-cache \
    python3 \
    py3-pip \
    nodejs \
    ffmpeg \
    ca-certificates \
    tzdata && \
    pip3 install --no-cache-dir --break-system-packages yt-dlp bgutil-ytdlp-pot-provider

RUN adduser -D -s /bin/sh appuser

RUN mkdir -p /app/data && \
    chown -R appuser:appuser /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/astionicbot /usr/local/bin/astionicbot
COPY grrr.mp3 /app/grrr.mp3

WORKDIR /app
USER appuser

ENV RUST_LOG=info

CMD ["astionicbot"]
