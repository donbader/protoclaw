FROM rust:1.91-bookworm AS builder

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY crates/protoclaw/Cargo.toml crates/protoclaw/Cargo.toml
COPY crates/protoclaw-core/Cargo.toml crates/protoclaw-core/Cargo.toml
COPY crates/protoclaw-config/Cargo.toml crates/protoclaw-config/Cargo.toml
COPY crates/protoclaw-jsonrpc/Cargo.toml crates/protoclaw-jsonrpc/Cargo.toml
COPY crates/protoclaw-agents/Cargo.toml crates/protoclaw-agents/Cargo.toml
COPY crates/protoclaw-channels/Cargo.toml crates/protoclaw-channels/Cargo.toml
COPY crates/protoclaw-tools/Cargo.toml crates/protoclaw-tools/Cargo.toml
COPY crates/protoclaw-sdk-types/Cargo.toml crates/protoclaw-sdk-types/Cargo.toml
COPY crates/protoclaw-sdk-channel/Cargo.toml crates/protoclaw-sdk-channel/Cargo.toml
COPY crates/protoclaw-sdk-tool/Cargo.toml crates/protoclaw-sdk-tool/Cargo.toml
COPY crates/protoclaw-sdk-agent/Cargo.toml crates/protoclaw-sdk-agent/Cargo.toml
COPY ext/channels/debug-http/Cargo.toml ext/channels/debug-http/Cargo.toml
COPY ext/channels/telegram/Cargo.toml ext/channels/telegram/Cargo.toml
COPY tests/mock-agent/Cargo.toml tests/mock-agent/Cargo.toml
COPY tests/integration/Cargo.toml tests/integration/Cargo.toml

RUN mkdir -p crates/protoclaw/src && echo "fn main() {}" > crates/protoclaw/src/main.rs \
    && mkdir -p crates/protoclaw-core/src && echo "" > crates/protoclaw-core/src/lib.rs \
    && mkdir -p crates/protoclaw-config/src && echo "" > crates/protoclaw-config/src/lib.rs \
    && mkdir -p crates/protoclaw-jsonrpc/src && echo "" > crates/protoclaw-jsonrpc/src/lib.rs \
    && mkdir -p crates/protoclaw-agents/src && echo "" > crates/protoclaw-agents/src/lib.rs \
    && mkdir -p crates/protoclaw-channels/src && echo "" > crates/protoclaw-channels/src/lib.rs \
    && mkdir -p crates/protoclaw-tools/src && echo "" > crates/protoclaw-tools/src/lib.rs \
    && mkdir -p crates/protoclaw-sdk-types/src && echo "" > crates/protoclaw-sdk-types/src/lib.rs \
    && mkdir -p crates/protoclaw-sdk-channel/src && echo "" > crates/protoclaw-sdk-channel/src/lib.rs \
    && mkdir -p crates/protoclaw-sdk-tool/src && echo "" > crates/protoclaw-sdk-tool/src/lib.rs \
    && mkdir -p crates/protoclaw-sdk-agent/src && echo "" > crates/protoclaw-sdk-agent/src/lib.rs \
    && mkdir -p ext/channels/debug-http/src && echo "fn main() {}" > ext/channels/debug-http/src/main.rs \
    && mkdir -p ext/channels/telegram/src && echo "fn main() {}" > ext/channels/telegram/src/main.rs \
    && mkdir -p tests/mock-agent/src && echo "fn main() {}" > tests/mock-agent/src/main.rs \
    && mkdir -p tests/integration/src && echo "" > tests/integration/src/lib.rs

RUN cargo build --release --bin protoclaw --bin telegram-channel 2>/dev/null || true

COPY crates/ crates/
COPY ext/ ext/
COPY tests/ tests/

RUN cargo build --release --bin protoclaw --bin telegram-channel

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /build/target/release/telegram-channel /usr/local/bin/telegram-channel

WORKDIR /workspace

ENTRYPOINT ["protoclaw"]
