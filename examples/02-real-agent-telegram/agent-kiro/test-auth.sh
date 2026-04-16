# Kiro auth validation — sourced by ../shared/test.sh
# Requires either KIRO_API_KEY in .env or a pre-authenticated kiro-auth-data volume.

HAS_API_KEY=false
if grep -q '^KIRO_API_KEY=.\+' .env 2>/dev/null; then
  HAS_API_KEY=true
fi

HAS_AUTH_VOLUME=false
if docker volume inspect kiro-auth-data >/dev/null 2>&1; then
  HAS_AUTH_VOLUME=true
fi

if [ "$HAS_API_KEY" = false ] && [ "$HAS_AUTH_VOLUME" = false ]; then
  printf "ERROR: No Kiro authentication found.\n"
  printf "Either set KIRO_API_KEY in .env, or run the browser login step.\n"
  printf "See README.md for details.\n"
  exit 1
fi
