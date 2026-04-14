# Stage 1: Chef base — Alpine for native musl toolchain
FROM lukemathwalker/cargo-chef:latest-rust-1.94-alpine AS chef
WORKDIR /build

# mold linker + clang driver for faster link times
RUN apk add --no-cache clang mold

# sccache for cross-build compilation caching via GHA cache backend
ARG SCCACHE_VERSION=v0.14.0
ARG TARGETARCH
RUN ARCH=$([ "$TARGETARCH" = "arm64" ] && echo "aarch64" || echo "x86_64") \
    && wget -qO- \
       "https://github.com/mozilla/sccache/releases/download/${SCCACHE_VERSION}/sccache-${SCCACHE_VERSION}-${ARCH}-unknown-linux-musl.tar.gz" \
       | tar -xz --strip-components=1 -C /usr/local/bin \
         "sccache-${SCCACHE_VERSION}-${ARCH}-unknown-linux-musl/sccache" \
    && chmod +x /usr/local/bin/sccache

# Stage 2: Planner — generate recipe.json from workspace manifests
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder — cook deps from recipe, then build all workspace binaries
FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
COPY .cargo .cargo

RUN --mount=type=secret,id=ACTIONS_RUNTIME_TOKEN \
    --mount=type=secret,id=ACTIONS_RESULTS_URL \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    ACTIONS_RUNTIME_TOKEN=$(cat /run/secrets/ACTIONS_RUNTIME_TOKEN 2>/dev/null || true) \
    ACTIONS_RESULTS_URL=$(cat /run/secrets/ACTIONS_RESULTS_URL 2>/dev/null || true) \
    SCCACHE_GHA_ENABLED=true \
    SCCACHE_NO_DAEMON=1 \
    RUSTC_WRAPPER=sccache \
    cargo chef cook --release --locked --recipe-path recipe.json \
    && sccache --show-stats

COPY . .

RUN --mount=type=secret,id=ACTIONS_RUNTIME_TOKEN \
    --mount=type=secret,id=ACTIONS_RESULTS_URL \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    ACTIONS_RUNTIME_TOKEN=$(cat /run/secrets/ACTIONS_RUNTIME_TOKEN 2>/dev/null || true) \
    ACTIONS_RESULTS_URL=$(cat /run/secrets/ACTIONS_RESULTS_URL 2>/dev/null || true) \
    SCCACHE_GHA_ENABLED=true \
    SCCACHE_NO_DAEMON=1 \
    RUSTC_WRAPPER=sccache \
    cargo build --release --locked \
    --bin anyclaw \
    --bin telegram-channel \
    --bin debug-http \
    --bin mock-agent \
    --bin system-info \
    && sccache --show-stats \
    && cp target/release/anyclaw \
        target/release/telegram-channel \
        target/release/debug-http \
        target/release/mock-agent \
        target/release/system-info \
        /tmp/

# Stage 4: Core runtime — anyclaw only (static, no OS packages)
FROM gcr.io/distroless/static-debian12:nonroot AS core
COPY --from=builder /tmp/anyclaw /usr/local/bin/anyclaw
WORKDIR /workspace
ENTRYPOINT ["anyclaw"]

# Stage 5: Builder export — anyclaw + all ext/ binaries in categorized paths
FROM gcr.io/distroless/static-debian12:nonroot AS builder-export
COPY --from=builder /tmp/anyclaw /usr/local/bin/anyclaw
COPY --from=builder /tmp/mock-agent /usr/local/bin/agents/mock-agent
COPY --from=builder /tmp/telegram-channel /usr/local/bin/channels/telegram
COPY --from=builder /tmp/debug-http /usr/local/bin/channels/debug-http
COPY --from=builder /tmp/system-info /usr/local/bin/tools/system-info
WORKDIR /workspace
ENTRYPOINT ["anyclaw"]
