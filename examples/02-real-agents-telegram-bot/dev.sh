#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILDER_IMAGE="lukemathwalker/cargo-chef:latest-rust-1.94-alpine"
BUILDER_CONTAINER="protoclaw-dev-builder"
COMPOSE_DEV="-f docker-compose.yml -f docker-compose.dev.yml"

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
            -e "CARGO_TARGET_DIR=/build/target" \
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
    docker compose $COMPOSE_DEV build
    docker compose $COMPOSE_DEV up -d
    echo "Up. Use './dev.sh logs' to follow output."
}

cmd_rebuild() {
    ensure_builder

    echo "Building protoclaw binaries (incremental)..."
    local start=$SECONDS
    docker exec "$BUILDER_CONTAINER" \
        cargo build --locked \
        --bin protoclaw \
        --bin telegram-channel \
        --bin debug-http \
        --bin system-info

    local container
    container=$(cd "$SCRIPT_DIR" && docker compose $COMPOSE_DEV ps -q protoclaw 2>/dev/null || true)
    if [ -z "$container" ]; then
        echo "Protoclaw container not running. Use './dev.sh up' first."
        exit 1
    fi

    echo "Copying binaries into container..."
    local tmpdir
    tmpdir=$(mktemp -d)
    docker cp "$BUILDER_CONTAINER:/build/target/debug/protoclaw" "$tmpdir/protoclaw"
    docker cp "$BUILDER_CONTAINER:/build/target/debug/telegram-channel" "$tmpdir/telegram-channel"
    docker cp "$BUILDER_CONTAINER:/build/target/debug/debug-http" "$tmpdir/debug-http"
    docker cp "$BUILDER_CONTAINER:/build/target/debug/system-info" "$tmpdir/system-info"
    docker cp "$tmpdir/protoclaw" "$container:/usr/local/bin/protoclaw"
    docker cp "$tmpdir/telegram-channel" "$container:/usr/local/bin/channels/telegram"
    docker cp "$tmpdir/debug-http" "$container:/usr/local/bin/channels/debug-http"
    docker cp "$tmpdir/system-info" "$container:/usr/local/bin/tools/system-info"
    rm -rf "$tmpdir"

    echo "Restarting protoclaw..."
    cd "$SCRIPT_DIR"
    docker compose $COMPOSE_DEV restart protoclaw

    local elapsed=$((SECONDS - start))
    echo "Done in ${elapsed}s."
}

cmd_logs() {
    cd "$SCRIPT_DIR"
    docker compose $COMPOSE_DEV logs protoclaw -f --since 0s
}

cmd_down() {
    cd "$SCRIPT_DIR"
    docker compose $COMPOSE_DEV down
    echo "Stopped. Builder container preserved (run 'docker rm -f $BUILDER_CONTAINER' to remove)."
}

cmd_shell() {
    cd "$SCRIPT_DIR"
    docker compose $COMPOSE_DEV exec protoclaw /bin/sh
}

case "${1:-help}" in
    up)      cmd_up ;;
    rebuild) cmd_rebuild ;;
    logs)    cmd_logs ;;
    down)    cmd_down ;;
    shell)   cmd_shell ;;
    *)       usage ;;
esac
