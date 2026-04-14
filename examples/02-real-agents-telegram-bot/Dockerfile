FROM ghcr.io/donbader/anyclaw-builder:latest AS builder

# Deps stage: opencode-ai with pruned node_modules
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS opencode-deps
RUN npm install -g opencode-ai@latest --omit=dev && \
    find /usr/local/lib/node_modules \
      -name "*.md" -o -name "*.map" -o -name "*.ts" -o -name "LICENSE*" \
      -o -name "CHANGELOG*" -o -name "*.d.ts" | xargs rm -f 2>/dev/null && \
    find /usr/local/lib/node_modules \
      -type d \( -name "test" -o -name "tests" -o -name "__tests__" \
      -o -name "docs" -o -name ".github" \) | xargs rm -rf 2>/dev/null; \
    true

# OpenCode target — anyclaw + ext/ binaries + Node.js + opencode-ai
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS example-02-opencode
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=opencode-deps /usr/local/lib/node_modules /usr/local/lib/node_modules
COPY --from=opencode-deps /usr/local/bin /usr/local/bin
COPY --from=builder /usr/local/bin/anyclaw /usr/local/bin/anyclaw
COPY --from=builder /usr/local/bin/channels/debug-http /usr/local/bin/channels/debug-http
COPY --from=builder /usr/local/bin/channels/telegram /usr/local/bin/channels/telegram
COPY --from=builder /usr/local/bin/tools/system-info /usr/local/bin/tools/system-info
WORKDIR /workspace
ENTRYPOINT ["anyclaw"]

# OpenCode agent image (for Docker workspace mode)
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS opencode-agent
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=opencode-deps /usr/local/lib/node_modules /usr/local/lib/node_modules
COPY --from=opencode-deps /usr/local/bin /usr/local/bin
# .opencode/ is gitignored — users provide their own config or run without it.
# Glob trick: README.md always exists so COPY never fails; .opencode is optional.
RUN mkdir -p /home/node/.config/opencode
COPY README.md .opencode* /tmp/build-staging/
RUN if [ -f /tmp/build-staging/opencode.json ]; then \
      cp /tmp/build-staging/opencode.json /home/node/.config/opencode/ && \
      cp /tmp/build-staging/package*.json /home/node/.config/opencode/ 2>/dev/null || true; \
    fi && rm -rf /tmp/build-staging
RUN if [ -f /home/node/.config/opencode/package.json ]; then \
      cd /home/node/.config/opencode && npm install --omit=dev; \
    fi
RUN mkdir -p /home/node/.local/share /home/node/.local/state && chown -R node:node /home/node/.local
USER node
WORKDIR /home/node
ENTRYPOINT ["opencode", "acp"]
