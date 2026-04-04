#!/usr/bin/env bash
set -euo pipefail

# Self-contained E2E test for the fake-agent example.
# Builds, starts, tests, and tears down Docker containers automatically.
# Usage: ./test.sh [base_url]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

BASE_URL="${1:-http://localhost:8080}"
PASS=0
FAIL=0

pass() { ((PASS++)); printf "  ✓ %s\n" "$1"; }
fail() { ((FAIL++)); printf "  ✗ %s\n" "$1"; }

# --- Trap-based cleanup (runs on success, failure, or Ctrl+C) ---
SSE_PID=""
cleanup() {
  [ -n "$SSE_PID" ] && kill "$SSE_PID" 2>/dev/null || true
  printf "\nTearing down...\n"
  docker compose down --timeout 10 2>/dev/null || true
}
trap cleanup EXIT

# --- Ensure .env exists ---
if [ ! -f .env ]; then
  cp .env.example .env
  printf "Created .env from .env.example\n"
fi

# --- Build + start ---
printf "Building and starting containers...\n"
docker compose up --build -d

# --- Wait for readiness ---
printf "Waiting for health endpoint"
for i in $(seq 1 30); do
  if curl -sf "$BASE_URL/health" >/dev/null 2>&1; then
    printf " ready\n\n"
    break
  fi
  printf "."
  sleep 1
  if [ "$i" -eq 30 ]; then
    printf " timeout\n"
    echo "FAIL: service not ready after 30s"
    exit 1
  fi
done

printf "Testing %s\n\n" "$BASE_URL"

# --- Test 1: Health ---
printf "Health\n"
STATUS=$(curl -sf -o /dev/null -w "%{http_code}" "$BASE_URL/health")
if [ "$STATUS" = "200" ]; then pass "GET /health → 200"; else fail "GET /health → $STATUS"; fi

# --- Test 2: Send message ---
printf "Send message\n"
RESP=$(curl -sf -X POST "$BASE_URL/message" \
  -H "Content-Type: application/json" \
  -d '{"message": "hello"}')
if echo "$RESP" | grep -q '"queued"\|"sent"'; then pass "POST /message → accepted"; else fail "POST /message → $RESP"; fi

sleep 3

# --- Open single SSE connection for all streaming tests ---
SSE_FILE=$(mktemp)
curl -sN "$BASE_URL/events" > "$SSE_FILE" 2>/dev/null &
SSE_PID=$!
sleep 2

# --- Test 3: SSE stream receives echo ---
printf "SSE response\n"
curl -sf -X POST "$BASE_URL/message" \
  -H "Content-Type: application/json" \
  -d '{"message": "ping"}' >/dev/null

sleep 5

if grep -q "Echo: ping" "$SSE_FILE"; then
  pass "SSE result contains 'Echo: ping'"
else
  fail "SSE result missing 'Echo: ping' (got: $(cat "$SSE_FILE"))"
fi

if grep -q '"agent_message_chunk"' "$SSE_FILE"; then
  pass "SSE stream contains streaming chunks"
else
  fail "SSE stream missing streaming chunks"
fi

if grep -q '"result"' "$SSE_FILE"; then
  pass "SSE stream contains result event"
else
  fail "SSE stream missing result event"
fi

# --- Test 4: Thinking pipeline (mock agent sends thought events) ---
printf "Thinking pipeline\n"
if grep -q "thought" "$SSE_FILE"; then
  pass "SSE stream contains thought events"
else
  fail "SSE stream missing thought events"
fi

sleep 2

# --- Test 5: Batch debounce (3 rapid messages → single merged response) ---
printf "Batch debounce\n"
BEFORE_BATCH=$(wc -l < "$SSE_FILE")

curl -sf -X POST "$BASE_URL/message" \
  -H "Content-Type: application/json" \
  -d '{"message": "batch1"}' >/dev/null
sleep 0.1
curl -sf -X POST "$BASE_URL/message" \
  -H "Content-Type: application/json" \
  -d '{"message": "batch2"}' >/dev/null
sleep 0.1
curl -sf -X POST "$BASE_URL/message" \
  -H "Content-Type: application/json" \
  -d '{"message": "batch3"}' >/dev/null

sleep 10

BATCH_OUTPUT=$(tail -n +"$((BEFORE_BATCH + 1))" "$SSE_FILE")
BATCH_RESULTS=$(echo "$BATCH_OUTPUT" | grep -c '"result"' || true)
HAS_BATCH1=$(echo "$BATCH_OUTPUT" | grep -c "batch1" || true)
HAS_BATCH2=$(echo "$BATCH_OUTPUT" | grep -c "batch2" || true)
HAS_BATCH3=$(echo "$BATCH_OUTPUT" | grep -c "batch3" || true)

if [ "$HAS_BATCH1" -gt 0 ] && [ "$HAS_BATCH2" -gt 0 ] && [ "$HAS_BATCH3" -gt 0 ]; then
  pass "All 3 batch messages appear in SSE stream"
else
  fail "Missing batch messages (batch1=$HAS_BATCH1 batch2=$HAS_BATCH2 batch3=$HAS_BATCH3)"
fi

if [ "$BATCH_RESULTS" -eq 1 ]; then
  pass "Debounce merged 3 messages into 1 result"
else
  fail "Expected 1 merged result, got $BATCH_RESULTS results"
fi

kill "$SSE_PID" 2>/dev/null || true
wait "$SSE_PID" 2>/dev/null || true
SSE_PID=""
rm -f "$SSE_FILE"

printf "\n%d passed, %d failed\n" "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ] || exit 1
