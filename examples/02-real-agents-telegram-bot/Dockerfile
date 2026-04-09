# Stage 1: Chef base
FROM lukemathwalker/cargo-chef:latest-rust-1.91-bookworm AS chef
WORKDIR /build

# Stage 2: Planner
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder
FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release \
    --bin protoclaw \
    --bin telegram-channel \
    --bin debug-http \
    --bin system-info \
    --bin opencode-wrapper

# Stage 4: Core runtime
FROM debian:bookworm-slim AS core
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /build/target/release/debug-http /usr/local/bin/debug-http
COPY --from=builder /build/target/release/telegram-channel /usr/local/bin/telegram-channel
COPY --from=builder /build/target/release/system-info /usr/local/bin/system-info
COPY --from=builder /build/target/release/opencode-wrapper /usr/local/bin/opencode-wrapper
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Stage 5: OpenCode target — core binaries + Node.js + opencode-ai
FROM node:20-slim AS example-02-opencode
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g opencode-ai@latest
COPY --from=core /usr/local/bin/protoclaw /usr/local/bin/protoclaw
COPY --from=core /usr/local/bin/debug-http /usr/local/bin/debug-http
COPY --from=core /usr/local/bin/telegram-channel /usr/local/bin/telegram-channel
COPY --from=core /usr/local/bin/system-info /usr/local/bin/system-info
RUN mkdir -p /usr/local/bin/agents
COPY --from=core /usr/local/bin/opencode-wrapper /usr/local/bin/agents/opencode
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Stage 6: Claude Code target — core binaries + Node.js + claude-code
FROM node:20-slim AS example-02-claude-code
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g @anthropic-ai/claude-code
COPY --from=core /usr/local/bin/protoclaw /usr/local/bin/protoclaw
COPY --from=core /usr/local/bin/debug-http /usr/local/bin/debug-http
COPY --from=core /usr/local/bin/telegram-channel /usr/local/bin/telegram-channel
COPY --from=core /usr/local/bin/system-info /usr/local/bin/system-info
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Stage 7: OpenCode agent image (for Docker workspace mode)
FROM node:20-slim AS opencode-agent
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g opencode-ai@latest
COPY --from=core /usr/local/bin/opencode-wrapper /usr/local/bin/opencode-wrapper
COPY examples/02-real-agents-telegram-bot/.opencode /home/node/.config/opencode
USER node
WORKDIR /home/node
ENTRYPOINT ["opencode-wrapper"]
