# Stage 1: Chef base — Alpine for native musl toolchain
FROM lukemathwalker/cargo-chef:latest-rust-1.94-alpine AS chef
WORKDIR /build

# mold linker + clang driver for faster link times
RUN apk add --no-cache clang mold

# sccache for cross-run compilation caching via GHA cache backend
ARG TARGETARCH
ENV SCCACHE=0.10.0
RUN case "$TARGETARCH" in \
      amd64) ARCH=x86_64 ;; \
      arm64) ARCH=aarch64 ;; \
      *) echo "unsupported arch: $TARGETARCH" && exit 1 ;; \
    esac && \
    wget -qO- "https://github.com/mozilla/sccache/releases/download/v${SCCACHE}/sccache-v${SCCACHE}-${ARCH}-unknown-linux-musl.tar.gz" \
    | tar -xzv --strip-components=1 -C /usr/local/bin "sccache-v${SCCACHE}-${ARCH}-unknown-linux-musl/sccache" && \
    chmod +x /usr/local/bin/sccache

# Stage 2: Planner — generate recipe.json from workspace manifests
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder — cook deps from recipe, then build all workspace binaries
# PROFILE: "release" (default) or "debug" (for dev builds)
FROM chef AS builder
ARG PROFILE=release

# Enable sccache when GHA credentials are available (CI only)
ARG SCCACHE_GHA_ENABLED
ENV RUSTC_WRAPPER=/usr/local/bin/sccache

COPY --from=planner /build/recipe.json recipe.json
COPY .cargo .cargo

RUN --mount=type=secret,id=actions_results_url,env=ACTIONS_RESULTS_URL \
    --mount=type=secret,id=actions_runtime_token,env=ACTIONS_RUNTIME_TOKEN \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    --mount=type=cache,target=/root/.cache/sccache \
    cargo chef cook $(if [ "$PROFILE" = "release" ]; then echo "--release"; fi) --locked --recipe-path recipe.json \
    && sccache --show-stats

COPY . .

RUN --mount=type=secret,id=actions_results_url,env=ACTIONS_RESULTS_URL \
    --mount=type=secret,id=actions_runtime_token,env=ACTIONS_RUNTIME_TOKEN \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    --mount=type=cache,target=/root/.cache/sccache \
    cargo build $(if [ "$PROFILE" = "release" ]; then echo "--release"; fi) --locked \
    --bin anyclaw \
    --bin telegram-channel \
    --bin debug-http \
    --bin mock-agent \
    --bin system-info \
    && sccache --show-stats \
    && cp target/$PROFILE/anyclaw \
        target/$PROFILE/telegram-channel \
        target/$PROFILE/debug-http \
        target/$PROFILE/mock-agent \
        target/$PROFILE/system-info \
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
