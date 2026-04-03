# Stage 1: Chef base — install cargo-chef, cached across all builds
FROM lukemathwalker/cargo-chef:latest-rust-1.91-bookworm AS chef
WORKDIR /build

# Stage 2: Planner — generate recipe.json from workspace manifests
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder — cook deps from recipe, then build all binaries
FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release \
    --bin protoclaw \
    --bin telegram-channel \
    --bin debug-http \
    --bin mock-agent

# Stage 4: Core runtime — protoclaw binary only (D-01)
FROM debian:bookworm-slim AS core
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Stage 5: Telegram — core + telegram-channel binary
FROM core AS telegram
COPY --from=builder /build/target/release/telegram-channel /usr/local/bin/telegram-channel

# Stage 6: debug-http — core + debug-http binary
FROM core AS debug-http
COPY --from=builder /build/target/release/debug-http /usr/local/bin/debug-http

# Stage 7: mock-agent — core + mock-agent binary
FROM core AS mock-agent
COPY --from=builder /build/target/release/mock-agent /usr/local/bin/mock-agent
