# Claude Code auth validation — sourced by ../shared/test.sh
# Requires ANTHROPIC_API_KEY in .env.

if ! grep -q '^ANTHROPIC_API_KEY=.\+' .env 2>/dev/null; then
  printf "ERROR: ANTHROPIC_API_KEY not set in .env\n"
  printf "Get an API key at https://console.anthropic.com/\n"
  exit 1
fi
