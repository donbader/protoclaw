# Stage 1: Chef base
FROM lukemathwalker/cargo-chef:latest-rust-1.91-bookworm AS chef
WORKDIR /build

# Stage 2: Planner
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder — cook deps then build all needed binaries
FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release \
    --bin protoclaw \
    --bin mock-agent \
    --bin debug-http \
    --bin telegram-channel \
    --bin system-info

# Stage 4: Runtime — single container with all binaries
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /build/target/release/mock-agent /usr/local/bin/mock-agent
COPY --from=builder /build/target/release/debug-http /usr/local/bin/debug-http
COPY --from=builder /build/target/release/telegram-channel /usr/local/bin/telegram-channel
COPY --from=builder /build/target/release/system-info /usr/local/bin/system-info
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]
