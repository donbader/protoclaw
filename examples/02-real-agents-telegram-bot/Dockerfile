# Stage 1: Chef base
FROM lukemathwalker/cargo-chef:latest-rust-1.91-bookworm AS chef
WORKDIR /build

# Stage 2: Planner
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder — cook deps then build protoclaw + channel binaries
FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release \
    --bin protoclaw \
    --bin debug-http \
    --bin telegram-channel \
    --bin system-info

# Stage 4a: OpenCode agent target (D-02)
FROM node:20-slim AS opencode
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g opencode-ai@latest
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /build/target/release/debug-http /usr/local/bin/debug-http
COPY --from=builder /build/target/release/telegram-channel /usr/local/bin/telegram-channel
COPY --from=builder /build/target/release/system-info /usr/local/bin/system-info
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Stage 4b: Claude Code agent target (D-02)
FROM node:20-slim AS claude-code
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g @anthropic-ai/claude-code
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /build/target/release/debug-http /usr/local/bin/debug-http
COPY --from=builder /build/target/release/telegram-channel /usr/local/bin/telegram-channel
COPY --from=builder /build/target/release/system-info /usr/local/bin/system-info
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]
