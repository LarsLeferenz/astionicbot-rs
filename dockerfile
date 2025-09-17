FROM rust:1.88-alpine3.22 as base-builder
RUN apk add --no-cache \
    alpine-sdk \
    pkgconfig \
    cmake \
    musl-dev \
    libc6-compat

FROM rust as planner
WORKDIR /app
# We only pay the installation cost once,
# it will be cached from the second build onwards
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json


FROM base-builder as cacher

WORKDIR /app
RUN cargo install cargo-chef
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin astionicbot --target x86_64-unknown-linux-musl

# Use Rust Alpine image as the base image
FROM base-builder as builder

WORKDIR /app
COPY . .
COPY --from=cacher /app/target /app/target
# Enable caching for cargo in docker build
RUN cargo build --release --bin astionicbot --target x86_64-unknown-linux-musl


FROM alpine:3.22 as runtime
LABEL type="AstionicBotRs"

# Install required dependencies
RUN apk add --update \
    yt-dlp

COPY .env /.env
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/astionicbot /usr/local/bin/astionicbot
COPY grrr.mp3 /grrr.mp3

CMD ["astionicbot"]
