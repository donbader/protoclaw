# Stage 1: Chef base — install cargo-chef, cached across all builds
# lukemathwalker/cargo-chef:latest-rust-1.91-bookworm
FROM lukemathwalker/cargo-chef:latest-rust-1.91-bookworm@sha256:beee6a0e6a7fba23540109792737deca7686e3dca811a86ea074b22711cfea83 AS chef
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
RUN cargo build --release --bin protoclaw
RUN cargo build --release \
    --bin telegram-channel \
    --bin debug-http \
    --bin mock-agent \
    --bin system-info \
    --bin opencode-wrapper

# Stage 4: Core runtime — protoclaw only (distroless, no shell)
# TODO: Pin distroless image by digest — Phase 74 will add reproducible pinning
FROM gcr.io/distroless/cc-debian12:nonroot AS core
COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Stage 5: Mock-agent standalone (for Docker workspace mode)
# TODO: Pin distroless image by digest — Phase 74 will add reproducible pinning
FROM gcr.io/distroless/cc-debian12:nonroot AS mock-agent
COPY --from=builder /build/target/release/mock-agent /usr/local/bin/mock-agent
ENTRYPOINT ["mock-agent"]
