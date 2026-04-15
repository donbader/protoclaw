# Stage 1: Chef base — Alpine for native musl toolchain
FROM lukemathwalker/cargo-chef:latest-rust-1.94-alpine AS chef
WORKDIR /build

# mold linker + clang driver for faster link times
RUN apk add --no-cache clang mold

# Stage 2: Planner — generate recipe.json from workspace manifests
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder — cook deps from recipe, then build all workspace binaries
# PROFILE: "release" (default) or "debug" (for dev builds)
FROM chef AS builder
ARG PROFILE=release
COPY --from=planner /build/recipe.json recipe.json
COPY .cargo .cargo

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo chef cook $(if [ "$PROFILE" = "release" ]; then echo "--release"; fi) --locked --recipe-path recipe.json

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo build $(if [ "$PROFILE" = "release" ]; then echo "--release"; fi) --locked \
    --bin anyclaw \
    --bin telegram-channel \
    --bin debug-http \
    --bin mock-agent \
    --bin system-info \
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
