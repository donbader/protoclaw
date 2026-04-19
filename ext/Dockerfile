# Builds all ext/ workspace binaries into a minimal image for COPY --from= usage.
# Separate from the root Dockerfile so ext/ concerns don't leak into core.
#
# Usage:
#   docker build -f ext/Dockerfile --target ext .
#   COPY --from=ghcr.io/donbader/anyclaw-ext:latest /usr/local/bin/channels/telegram /usr/local/bin/channels/telegram

FROM lukemathwalker/cargo-chef:latest-rust-1.94-alpine AS chef
WORKDIR /build

RUN apk add --no-cache clang mold jq

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

FROM chef AS planner
COPY . .
RUN cd ext && cargo chef prepare --recipe-path /build/recipe-ext.json

FROM chef AS builder
ARG PROFILE=release

ARG SCCACHE_GHA_ENABLED
ENV RUSTC_WRAPPER=/usr/local/bin/sccache

# ext/ path-depends on SDK crates which use workspace inheritance from root
COPY . .
COPY --from=planner /build/recipe-ext.json ext/recipe-ext.json

RUN --mount=type=secret,id=actions_results_url,env=ACTIONS_RESULTS_URL \
    --mount=type=secret,id=actions_runtime_token,env=ACTIONS_RUNTIME_TOKEN \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/ext/target \
    --mount=type=cache,target=/root/.cache/sccache \
    cd ext && cargo chef cook $(if [ "$PROFILE" = "release" ]; then echo "--release"; fi) --locked --recipe-path recipe-ext.json \
    && sccache --show-stats

# Restore real source files over the stubs that cargo-chef cook created.
# Without this, cargo build sees the stub sources and skips recompilation.
COPY . .

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
    && mkdir -p /out/agents /out/channels /out/tools \
    && for bin in $EXT_BINS; do cp /build/ext/target/$PROFILE/$bin /out/ 2>/dev/null || true; done \
    && mv /out/mock-agent /out/agents/ 2>/dev/null || true \
    && mv /out/telegram-channel /out/channels/telegram 2>/dev/null || true \
    && mv /out/debug-http /out/channels/ 2>/dev/null || true \
    && mv /out/system-info /out/tools/ 2>/dev/null || true

FROM gcr.io/distroless/static-debian12:nonroot AS ext
COPY --from=builder /out/ /usr/local/bin/
