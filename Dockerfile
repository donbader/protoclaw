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
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Stage 5: Example 01 — core + mock-agent + system-info
FROM core AS example-01
COPY --from=builder /build/target/release/mock-agent /usr/local/bin/mock-agent
COPY --from=builder /build/target/release/system-info /usr/local/bin/system-info

# Stage 6: Example 01 mock-agent image (for Docker workspace mode)
FROM debian:bookworm-slim AS mock-agent
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/mock-agent /usr/local/bin/mock-agent
ENTRYPOINT ["mock-agent"]

# Stage 7: Example 02 OpenCode target — core + system-info + opencode-wrapper + node
FROM node:20-slim AS example-02-opencode
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g opencode-ai@latest
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /build/target/release/debug-http /usr/local/bin/debug-http
COPY --from=builder /build/target/release/telegram-channel /usr/local/bin/telegram-channel
COPY --from=builder /build/target/release/system-info /usr/local/bin/system-info
RUN mkdir -p /usr/local/bin/agents
COPY --from=builder /build/target/release/opencode-wrapper /usr/local/bin/agents/opencode
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Stage 8: Example 02 Claude Code target
FROM node:20-slim AS example-02-claude-code
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g @anthropic-ai/claude-code
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /build/target/release/debug-http /usr/local/bin/debug-http
COPY --from=builder /build/target/release/telegram-channel /usr/local/bin/telegram-channel
COPY --from=builder /build/target/release/system-info /usr/local/bin/system-info
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Stage 9: Example 02 opencode-agent image (for Docker workspace mode)
FROM node:20-slim AS opencode-agent
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g opencode-ai@latest
COPY --from=builder /build/target/release/opencode-wrapper /usr/local/bin/opencode-wrapper
COPY examples/02-real-agents-telegram-bot/.opencode /home/node/.config/opencode
USER node
WORKDIR /home/node
ENTRYPOINT ["opencode-wrapper"]
