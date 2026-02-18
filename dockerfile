FROM rust:1.88-alpine3.22 AS base-builder
RUN apk add --no-cache \
    alpine-sdk \
    pkgconfig \
    cmake \
    musl-dev

RUN cargo install cargo-chef


FROM base-builder AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM base-builder AS cacher
WORKDIR /app
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
    tzdata \
    bash

RUN adduser -D -s /bin/bash appuser

RUN mkdir -p /app/data && \
    chown -R appuser:appuser /app

COPY --from=builder --chown=appuser:appuser /app/target/x86_64-unknown-linux-musl/release/astionicbot /usr/local/bin/astionicbot
COPY grrr.mp3 /app/grrr.mp3

WORKDIR /app
USER appuser

COPY --chown=appuser:appuser entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

ENV RUST_LOG=info

ENTRYPOINT ["/app/entrypoint.sh"]
