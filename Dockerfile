# Stage 1: Chef base — install cargo-chef, cached across all builds
FROM lukemathwalker/cargo-chef:latest-rust-1.91-bookworm AS chef
WORKDIR /build

# Stage 2: Planner — generate recipe.json from workspace manifests
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder — cook deps from recipe, then build all workspace binaries
FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release \
    --bin protoclaw \
    --bin telegram-channel \
    --bin debug-http \
    --bin mock-agent \
    --bin system-info \
    --bin opencode-wrapper

# Stage 4: Core runtime — protoclaw + all channel binaries (base for examples)
FROM debian:bookworm-slim AS core
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /build/target/release/debug-http /usr/local/bin/debug-http
COPY --from=builder /build/target/release/telegram-channel /usr/local/bin/telegram-channel
COPY --from=builder /build/target/release/mock-agent /usr/local/bin/mock-agent
COPY --from=builder /build/target/release/system-info /usr/local/bin/system-info
COPY --from=builder /build/target/release/opencode-wrapper /usr/local/bin/opencode-wrapper
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]
