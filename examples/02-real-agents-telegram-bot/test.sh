#!/usr/bin/env bash
set -euo pipefail

# Local-only E2E test — ANTHROPIC_API_KEY optional (agent uses baked-in config)
# CI skips this script (no secrets available in CI runner)
# Usage: ./test.sh [--local] [base_url]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

LOCAL_MODE=false
if [[ "${1:-}" == "--local" ]]; then
  LOCAL_MODE=true
  shift
fi

BASE_URL="${1:-http://localhost:8080}"
PASS=0
FAIL=0

pass() { ((PASS++)); printf "  ✓ %s\n" "$1"; }
fail() { ((FAIL++)); printf "  ✗ %s\n" "$1"; }

SSE_PID=""
cleanup() {
  [ -n "$SSE_PID" ] && kill "$SSE_PID" 2>/dev/null || true
  printf "\nTearing down...\n"
  docker compose down --timeout 10 2>/dev/null || true
  [ -f protoclaw.yaml.bak ] && mv protoclaw.yaml.bak protoclaw.yaml
  [ -f docker-compose.override.yml ] && rm -f docker-compose.override.yml
}
trap cleanup EXIT

# --- .env validation ---
if [ ! -f .env ]; then
  printf "WARN: .env file not found — creating from .env.example\n"
  cp .env.example .env
fi

source .env
if [ -z "${ANTHROPIC_API_KEY:-}" ] || [ "$ANTHROPIC_API_KEY" = "your-api-key-here" ]; then
  printf "NOTE: ANTHROPIC_API_KEY not set — agent uses baked-in config\n"
fi

# --- Local mode setup ---
if [ "$LOCAL_MODE" = true ]; then
  printf "Running in local workspace mode (--local)\n\n"
  cp protoclaw.yaml protoclaw.yaml.bak
  sed -i.tmp 's/^\(\s*\)enabled: true/\1enabled: false/' protoclaw.yaml
  sed -i.tmp '/opencode-local:/,/tools:/{s/enabled: false/enabled: true/}' protoclaw.yaml
  sed -i.tmp 's/agent: "opencode"/agent: "opencode-local"/' protoclaw.yaml
  rm -f protoclaw.yaml.tmp
  cat > docker-compose.override.yml <<'OVERRIDE'
services:
  protoclaw:
    volumes:
      - ./protoclaw.yaml:/workspace/protoclaw.yaml:ro
      - ./.opencode:/home/protoclaw/.config/opencode:ro
OVERRIDE
fi

# --- Build agent image + start ---
if [ "$LOCAL_MODE" = false ]; then
  printf "Building agent Docker image...\n"
  docker compose --profile build-only build || { printf "FAIL: agent image build failed\n"; exit 1; }
fi

printf "Building and starting containers...\n"
docker compose up --build -d || { printf "FAIL: docker compose up failed\n"; exit 1; }

# --- Wait for readiness ---
printf "Waiting for health endpoint"
for i in $(seq 1 60); do
  if curl -sf "$BASE_URL/health" >/dev/null 2>&1; then
    printf " ready\n\n"
    break
  fi
  printf "."
  sleep 1
  if [ "$i" -eq 60 ]; then
    printf " timeout\n"
    echo "FAIL: service not ready after 60s"
    exit 1
  fi
done

printf "Testing %s\n\n" "$BASE_URL"

# --- Test 1: Health ---
printf "Health\n"
STATUS=$(curl -sf -o /dev/null -w "%{http_code}" "$BASE_URL/health")
if [ "$STATUS" = "200" ]; then pass "GET /health → 200"; else fail "GET /health → $STATUS"; fi

printf "Send message\n"
RESP=$(curl -sf -X POST "$BASE_URL/message" \
  -H "Content-Type: application/json" \
  -d '{"message": "hello"}')
if echo "$RESP" | grep -q '"queued"\|"sent"'; then pass "POST /message → accepted"; else fail "POST /message → $RESP"; fi

sleep 3

SSE_FILE=$(mktemp)
curl -sN "$BASE_URL/events" > "$SSE_FILE" 2>/dev/null &
SSE_PID=$!
sleep 2

printf "SSE response\n"
curl -sf -X POST "$BASE_URL/message" \
  -H "Content-Type: application/json" \
  -d '{"message": "Reply with exactly: PROTOCLAW_TEST_OK"}' >/dev/null

sleep 45

if grep -q "PROTOCLAW_TEST_OK\|agent_message_chunk\|data:" "$SSE_FILE"; then
  pass "SSE stream contains agent response"
else
  fail "SSE stream empty (got: $(head -20 "$SSE_FILE"))"
fi

printf "Streaming\n"
if grep -q "agent_message_chunk\|^data: " "$SSE_FILE"; then
  pass "SSE stream contains streaming chunks"
else
  fail "SSE stream missing streaming chunks"
fi

if grep -q "result\|^data: .*PROTOCLAW_TEST_OK" "$SSE_FILE"; then
  pass "SSE stream contains result"
else
  fail "SSE stream missing result"
fi

printf "Message merging\n"
BEFORE_BATCH=$(wc -l < "$SSE_FILE")

for i in $(seq 1 5); do
  curl -sf -X POST "$BASE_URL/message" \
    -H "Content-Type: application/json" \
    -d "{\"message\": \"PROTO_BATCH_$i\"}" >/dev/null
  sleep 0.03
done

sleep 60

BATCH_OUTPUT=$(tail -n +"$((BEFORE_BATCH + 1))" "$SSE_FILE")

if [ -n "$BATCH_OUTPUT" ]; then
  pass "Agent responded to batch messages"
else
  fail "No response to batch messages"
fi

RESULT_COUNT=$(echo "$BATCH_OUTPUT" | grep -c '"result"\|^event: result' || true)
if [ "$RESULT_COUNT" -ge 1 ] && [ "$RESULT_COUNT" -lt 5 ]; then
  pass "Messages merged: 5 sent, $RESULT_COUNT agent turn(s)"
else
  fail "Expected 1-4 agent turns (merging), got $RESULT_COUNT"
fi

CONTAINER_LOGS=$(docker compose logs protoclaw --no-color 2>/dev/null || true)
CONTENT_FOUND=0
for i in $(seq 1 5); do
  if printf '%s' "$CONTAINER_LOGS" | grep -q "PROTO_BATCH_$i" 2>/dev/null; then
    CONTENT_FOUND=$((CONTENT_FOUND + 1))
  fi
done
if [ "$CONTENT_FOUND" -ge 3 ]; then
  pass "Merged prompt contains batch content ($CONTENT_FOUND/5 in server logs)"
else
  fail "Merged prompt missing batch content ($CONTENT_FOUND/5 in server logs)"
fi

kill "$SSE_PID" 2>/dev/null || true
wait "$SSE_PID" 2>/dev/null || true
SSE_PID=""
rm -f "$SSE_FILE"

printf "\n%d passed, %d failed\n" "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ] || exit 1
