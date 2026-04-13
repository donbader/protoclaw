#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILDER_IMAGE="lukemathwalker/cargo-chef:latest-rust-1.94-alpine"
BUILDER_CONTAINER="protoclaw-dev-builder"
COMPOSE_PROJECT="02-real-agents-telegram-bot"

usage() {
    cat <<EOF
Usage: ./dev.sh [command]

Commands:
  up        Build (if needed) and start containers
  rebuild   Incremental rebuild + restart protoclaw container
  logs      Follow protoclaw logs (filtered)
  down      Stop and remove containers
  shell     Shell into the protoclaw container

First run uses docker compose build. Subsequent rebuilds use a persistent
builder container with cached cargo registry and target directory.
EOF
    exit 0
}

ensure_builder() {
    if ! docker inspect "$BUILDER_CONTAINER" &>/dev/null; then
        echo "Creating persistent builder container..."
        docker create \
            --name "$BUILDER_CONTAINER" \
            -v "protoclaw-cargo-registry:/usr/local/cargo/registry" \
            -v "protoclaw-cargo-git:/usr/local/cargo/git" \
            -v "protoclaw-target:/build/target" \
            -v "$WORKSPACE_ROOT:/build/src:ro" \
            -w /build/src \
            "$BUILDER_IMAGE" \
            sleep infinity
    fi

    if [ "$(docker inspect -f '{{.State.Running}}' "$BUILDER_CONTAINER" 2>/dev/null)" != "true" ]; then
        docker start "$BUILDER_CONTAINER"
    fi
}

cmd_up() {
    cd "$SCRIPT_DIR"
    docker compose build
    docker compose up -d
    echo "Up. Use './dev.sh logs' to follow output."
}

cmd_rebuild() {
    ensure_builder

    echo "Building protoclaw binaries (incremental)..."
    local start=$SECONDS
    docker exec "$BUILDER_CONTAINER" \
        cargo build --release --locked \
        --bin protoclaw \
        --bin telegram-channel \
        --bin debug-http \
        --bin system-info

    local container
    container=$(cd "$SCRIPT_DIR" && docker compose ps -q protoclaw 2>/dev/null || true)
    if [ -z "$container" ]; then
        echo "Protoclaw container not running. Use './dev.sh up' first."
        exit 1
    fi

    echo "Copying binaries into container..."
    docker cp "$BUILDER_CONTAINER:/build/src/target/release/protoclaw" "$container:/usr/local/bin/protoclaw"
    docker cp "$BUILDER_CONTAINER:/build/src/target/release/telegram-channel" "$container:/usr/local/bin/channels/telegram"
    docker cp "$BUILDER_CONTAINER:/build/src/target/release/debug-http" "$container:/usr/local/bin/channels/debug-http"
    docker cp "$BUILDER_CONTAINER:/build/src/target/release/system-info" "$container:/usr/local/bin/tools/system-info"

    echo "Restarting protoclaw..."
    cd "$SCRIPT_DIR"
    docker compose restart protoclaw

    local elapsed=$((SECONDS - start))
    echo "Done in ${elapsed}s."
}

cmd_logs() {
    cd "$SCRIPT_DIR"
    docker compose logs protoclaw -f --since 0s 2>&1 | grep -viE "pool.*idle|reuse idle|hyper_util.*pool|hyper_util.*connect"
}

cmd_down() {
    cd "$SCRIPT_DIR"
    docker compose down
    echo "Stopped. Builder container preserved (run 'docker rm -f $BUILDER_CONTAINER' to remove)."
}

cmd_shell() {
    cd "$SCRIPT_DIR"
    docker compose exec protoclaw /bin/sh
}

case "${1:-help}" in
    up)      cmd_up ;;
    rebuild) cmd_rebuild ;;
    logs)    cmd_logs ;;
    down)    cmd_down ;;
    shell)   cmd_shell ;;
    *)       usage ;;
esac
