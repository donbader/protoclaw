# Stage 1: Chef base — Alpine for native musl toolchain
FROM lukemathwalker/cargo-chef:latest-rust-1.94-alpine AS chef
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

# Stage 4: Core runtime — protoclaw only (static, no OS packages)
FROM gcr.io/distroless/static-debian12:nonroot AS core
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Stage 5: Builder export — protoclaw + all ext/ binaries in categorized paths
FROM gcr.io/distroless/static-debian12:nonroot AS builder-export
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /build/target/release/mock-agent /usr/local/bin/agents/mock-agent
COPY --from=builder /build/target/release/opencode-wrapper /usr/local/bin/agents/opencode-wrapper
COPY --from=builder /build/target/release/telegram-channel /usr/local/bin/channels/telegram
COPY --from=builder /build/target/release/debug-http /usr/local/bin/channels/debug-http
COPY --from=builder /build/target/release/system-info /usr/local/bin/tools/system-info
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]
