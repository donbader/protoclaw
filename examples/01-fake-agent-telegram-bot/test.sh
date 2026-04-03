#!/usr/bin/env bash
set -euo pipefail

# E2E smoke test for the fake-agent example.
# Requires: docker compose up -d (already running)
# Usage: ./test.sh [base_url]

BASE_URL="${1:-http://localhost:8080}"
PASS=0
FAIL=0

pass() { ((PASS++)); printf "  ✓ %s\n" "$1"; }
fail() { ((FAIL++)); printf "  ✗ %s\n" "$1"; }

printf "Testing %s\n\n" "$BASE_URL"

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

# --- Test 1: Health ---
printf "Health\n"
STATUS=$(curl -sf -o /dev/null -w "%{http_code}" "$BASE_URL/health")
if [ "$STATUS" = "200" ]; then pass "GET /health → 200"; else fail "GET /health → $STATUS"; fi

# --- Test 2: Send message ---
printf "Send message\n"
RESP=$(curl -sf -X POST "$BASE_URL/message" \
    -H "Content-Type: application/json" \
    -d '{"message": "hello"}')
if echo "$RESP" | grep -q '"sent"'; then pass "POST /message → sent"; else fail "POST /message → $RESP"; fi

# --- Test 3: SSE stream receives echo ---
printf "SSE response\n"
SSE_FILE=$(mktemp)
curl -sf -N "$BASE_URL/events" > "$SSE_FILE" 2>/dev/null &
SSE_PID=$!
sleep 1

curl -sf -X POST "$BASE_URL/message" \
    -H "Content-Type: application/json" \
    -d '{"message": "ping"}' >/dev/null

sleep 3
kill "$SSE_PID" 2>/dev/null || true
wait "$SSE_PID" 2>/dev/null || true

if grep -q '"Echo: ping"' "$SSE_FILE"; then
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
rm -f "$SSE_FILE"

# --- Summary ---
printf "\n%d passed, %d failed\n" "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ] || exit 1
