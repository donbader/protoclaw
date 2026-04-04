#!/usr/bin/env bash
set -euo pipefail

# Local-only E2E test — requires ANTHROPIC_API_KEY in .env
# CI skips this script (no secrets available in CI runner)
# Usage: ./test.sh [base_url]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

BASE_URL="${1:-http://localhost:8080}"
PASS=0
FAIL=0

pass() { ((PASS++)); printf "  ✓ %s\n" "$1"; }
fail() { ((FAIL++)); printf "  ✗ %s\n" "$1"; }

cleanup() {
  printf "\nTearing down...\n"
  docker compose down --timeout 10 2>/dev/null || true
}
trap cleanup EXIT

# --- .env validation ---
if [ ! -f .env ]; then
  printf "ERROR: .env file not found.\n"
  printf "Copy .env.example to .env and set ANTHROPIC_API_KEY:\n\n"
  printf "  cp .env.example .env\n"
  printf "  # Edit .env — set your Anthropic API key\n\n"
  exit 1
fi

source .env
if [ -z "${ANTHROPIC_API_KEY:-}" ] || [ "$ANTHROPIC_API_KEY" = "your-api-key-here" ]; then
  printf "ERROR: ANTHROPIC_API_KEY not set in .env\n"
  printf "Get your key from https://console.anthropic.com\n"
  exit 1
fi

# --- Build + start ---
printf "Building and starting containers...\n"
docker compose up --build -d

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

# --- Test 2: Send message + verify agent responds ---
printf "Agent response\n"
SSE_FILE=$(mktemp)
curl -sf -N "$BASE_URL/events" > "$SSE_FILE" 2>/dev/null &
SSE_PID=$!
sleep 1

curl -sf -X POST "$BASE_URL/message" \
  -H "Content-Type: application/json" \
  -d '{"message": "Say hello in exactly 3 words"}' >/dev/null

sleep 30
kill "$SSE_PID" 2>/dev/null || true
wait "$SSE_PID" 2>/dev/null || true

if grep -q '"result"' "$SSE_FILE"; then
  pass "SSE stream contains result event (agent responded)"
else
  fail "SSE stream missing result event (got: $(head -20 "$SSE_FILE"))"
fi
rm -f "$SSE_FILE"

printf "\n%d passed, %d failed\n" "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ] || exit 1
