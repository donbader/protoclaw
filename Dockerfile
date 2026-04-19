# Stage 1: Chef base — Alpine for native musl toolchain
FROM lukemathwalker/cargo-chef:latest-rust-1.94-alpine AS chef
WORKDIR /build

# mold linker + clang driver for faster link times
RUN apk add --no-cache clang mold jq

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

# Stage 2a: Planner for root workspace
FROM chef AS planner-core
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2b: Planner for ext/ workspace
FROM chef AS planner-ext
COPY . .
RUN cd ext && cargo chef prepare --recipe-path /build/recipe-ext.json

# Stage 3a: Build anyclaw binary (runs in parallel with 3b)
FROM chef AS builder-core
ARG PROFILE=release
ARG ANYCLAW_VERSION=unknown
ENV ANYCLAW_VERSION=${ANYCLAW_VERSION}

ARG SCCACHE_GHA_ENABLED
ENV RUSTC_WRAPPER=/usr/local/bin/sccache

COPY --from=planner-core /build/recipe.json recipe.json
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
    cargo build $(if [ "$PROFILE" = "release" ]; then echo "--release"; fi) --locked --bin anyclaw \
    && cp target/$PROFILE/anyclaw /tmp/anyclaw \
    && sccache --show-stats

# Stage 3b: Build ext/ binaries (runs in parallel with 3a)
FROM chef AS builder-ext
ARG PROFILE=release

ARG SCCACHE_GHA_ENABLED
ENV RUSTC_WRAPPER=/usr/local/bin/sccache

# ext/ path-depends on SDK crates which use workspace inheritance from root —
# copy full tree so the root workspace resolves all members during dep cooking
COPY . .
COPY --from=planner-ext /build/recipe-ext.json ext/recipe-ext.json

RUN --mount=type=secret,id=actions_results_url,env=ACTIONS_RESULTS_URL \
    --mount=type=secret,id=actions_runtime_token,env=ACTIONS_RUNTIME_TOKEN \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/ext/target \
    --mount=type=cache,target=/root/.cache/sccache \
    cd ext && cargo chef cook $(if [ "$PROFILE" = "release" ]; then echo "--release"; fi) --locked --recipe-path recipe-ext.json \
    && sccache --show-stats

RUN --mount=type=secret,id=actions_results_url,env=ACTIONS_RESULTS_URL \
    --mount=type=secret,id=actions_runtime_token,env=ACTIONS_RUNTIME_TOKEN \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/ext/target \
    --mount=type=cache,target=/root/.cache/sccache \
    cd ext \
    && cargo build $(if [ "$PROFILE" = "release" ]; then echo "--release"; fi) --locked --workspace \
    && sccache --show-stats \
    && EXT_BINS=$(cargo metadata --format-version 1 --no-deps \
         | jq -r '.packages[].targets[] | select(.kind[] == "bin") | .name') \
    && for bin in $EXT_BINS; do cp /build/ext/target/$PROFILE/$bin /tmp/ 2>/dev/null || true; done

# Stage 4: Core runtime — anyclaw only (static, no OS packages)
FROM gcr.io/distroless/static-debian12:nonroot AS core
COPY --from=builder-core /tmp/anyclaw /usr/local/bin/anyclaw
WORKDIR /workspace
ENTRYPOINT ["anyclaw"]

# Stage 5: Builder export — anyclaw + all ext/ binaries in categorized paths
FROM gcr.io/distroless/static-debian12:nonroot AS builder-export
COPY --from=builder-core /tmp/anyclaw /usr/local/bin/anyclaw
COPY --from=builder-ext /tmp/mock-agent /usr/local/bin/agents/mock-agent
COPY --from=builder-ext /tmp/telegram-channel /usr/local/bin/channels/telegram
COPY --from=builder-ext /tmp/debug-http /usr/local/bin/channels/debug-http
COPY --from=builder-ext /tmp/system-info /usr/local/bin/tools/system-info
WORKDIR /workspace
ENTRYPOINT ["anyclaw"]
