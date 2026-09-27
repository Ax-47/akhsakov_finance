# syntax=docker/dockerfile:1.7

FROM rust:1-trixie AS builder

ARG DX_VERSION=0.7.9

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-unknown-unknown
RUN curl --fail --location --silent --show-error \
        "https://github.com/DioxusLabs/dioxus/releases/download/v${DX_VERSION}/dx-x86_64-unknown-linux-gnu.tar.gz" \
        --output /tmp/dx.tar.gz \
    && tar --extract --gzip --file /tmp/dx.tar.gz --directory /usr/local/bin dx \
    && rm /tmp/dx.tar.gz \
    && dx --version

WORKDIR /src
COPY . .

WORKDIR /src/packages/web
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    dx bundle --web --release --out-dir /opt/akhsakov-finance

FROM debian:trixie-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libssl3t64 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /opt/akhsakov-finance/ ./
RUN mkdir -p /data

ENV IP=0.0.0.0 \
    PORT=8080 \
    DIOXUS_PUBLIC_PATH=/app/public \
    AKHSAKOV_DB=/data/akhsakov_finance.db

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl --fail --silent --show-error \
        --request POST \
        --header "Content-Type: application/json" \
        --data '{}' \
        http://127.0.0.1:8080/api/auth/status > /dev/null || exit 1

ENTRYPOINT ["/app/server"]
