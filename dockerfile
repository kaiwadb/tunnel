FROM lukemathwalker/cargo-chef:latest-rust-1.86.0-slim AS chef

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    make \
    gcc \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

FROM chef AS planner
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/kaiwadb-tunnel /usr/local/bin/kaiwadb-tunnel

ENTRYPOINT ["kaiwadb-tunnel"]
