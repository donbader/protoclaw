FROM ghcr.io/donbader/protoclaw-builder:latest AS builder

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

# Deps stage: claude-code with pruned node_modules
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS claude-code-deps
RUN npm install -g @anthropic-ai/claude-code --omit=dev && \
    find /usr/local/lib/node_modules \
      -name "*.md" -o -name "*.map" -o -name "*.ts" -o -name "LICENSE*" \
      -o -name "CHANGELOG*" -o -name "*.d.ts" | xargs rm -f 2>/dev/null && \
    find /usr/local/lib/node_modules \
      -type d \( -name "test" -o -name "tests" -o -name "__tests__" \
      -o -name "docs" -o -name ".github" \) | xargs rm -rf 2>/dev/null; \
    true

# OpenCode target — protoclaw + ext/ binaries + Node.js + opencode-ai
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS example-02-opencode
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=opencode-deps /usr/local/lib/node_modules /usr/local/lib/node_modules
COPY --from=opencode-deps /usr/local/bin /usr/local/bin
COPY --from=builder /usr/local/bin/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /usr/local/bin/channels/debug-http /usr/local/bin/channels/debug-http
COPY --from=builder /usr/local/bin/channels/telegram /usr/local/bin/channels/telegram
COPY --from=builder /usr/local/bin/tools/system-info /usr/local/bin/tools/system-info
COPY --from=builder /usr/local/bin/agents/opencode-wrapper /usr/local/bin/agents/opencode-wrapper
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Claude Code target — protoclaw + ext/ binaries + Node.js + claude-code
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS example-02-claude-code
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=claude-code-deps /usr/local/lib/node_modules /usr/local/lib/node_modules
COPY --from=claude-code-deps /usr/local/bin /usr/local/bin
COPY --from=builder /usr/local/bin/protoclaw /usr/local/bin/protoclaw
COPY --from=builder /usr/local/bin/channels/debug-http /usr/local/bin/channels/debug-http
COPY --from=builder /usr/local/bin/channels/telegram /usr/local/bin/channels/telegram
COPY --from=builder /usr/local/bin/tools/system-info /usr/local/bin/tools/system-info
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# OpenCode agent image (for Docker workspace mode)
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS opencode-agent
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=opencode-deps /usr/local/lib/node_modules /usr/local/lib/node_modules
COPY --from=opencode-deps /usr/local/bin /usr/local/bin
COPY --from=builder /usr/local/bin/agents/opencode-wrapper /usr/local/bin/agents/opencode-wrapper
# .opencode/ is gitignored — users provide their own config or run without it.
# Glob trick: README.md always exists so COPY never fails; .opencode is optional.
RUN mkdir -p /home/node/.config/opencode
COPY README.md .opencode* /tmp/build-staging/
RUN if [ -d /tmp/build-staging/.opencode ]; then \
      cp -r /tmp/build-staging/.opencode/* /home/node/.config/opencode/ 2>/dev/null || true; \
    fi && rm -rf /tmp/build-staging
USER node
WORKDIR /home/node
ENTRYPOINT ["/usr/local/bin/agents/opencode-wrapper"]
