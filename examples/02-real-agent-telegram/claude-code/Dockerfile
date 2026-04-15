ARG BUILDER_IMAGE=ghcr.io/donbader/anyclaw-builder:latest
FROM ${BUILDER_IMAGE} AS builder

# Deps stage: claude-agent-acp (ACP adapter wrapping Claude Agent SDK)
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS claude-deps
RUN npm install -g @agentclientprotocol/claude-agent-acp@latest --omit=dev && \
    find /usr/local/lib/node_modules \
      -name "*.md" -o -name "*.map" -o -name "*.ts" -o -name "LICENSE*" \
      -o -name "CHANGELOG*" -o -name "*.d.ts" | xargs rm -f 2>/dev/null && \
    find /usr/local/lib/node_modules \
      -type d \( -name "test" -o -name "tests" -o -name "__tests__" \
      -o -name "docs" -o -name ".github" \) | xargs rm -rf 2>/dev/null; \
    true

# Claude Code target — anyclaw + ext/ binaries + Node.js + claude-agent-acp
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS example-02-claude-code
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=claude-deps /usr/local/lib/node_modules /usr/local/lib/node_modules
COPY --from=claude-deps /usr/local/bin /usr/local/bin
COPY --from=builder /usr/local/bin/anyclaw /usr/local/bin/anyclaw
COPY --from=builder /usr/local/bin/channels/debug-http /usr/local/bin/channels/debug-http
COPY --from=builder /usr/local/bin/channels/telegram /usr/local/bin/channels/telegram
COPY --from=builder /usr/local/bin/tools/system-info /usr/local/bin/tools/system-info
RUN mkdir -p /workspace && chown node:node /workspace
WORKDIR /workspace
COPY --chown=node:node anyclaw.yaml /workspace/anyclaw.yaml
USER node
ENTRYPOINT ["anyclaw"]

# Claude Code agent image (for Docker workspace mode)
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS claude-code-agent
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates sudo && rm -rf /var/lib/apt/lists/* && \
    echo "node ALL=(ALL) NOPASSWD: /usr/bin/apt-get" >> /etc/sudoers.d/node
COPY --from=claude-deps /usr/local/lib/node_modules /usr/local/lib/node_modules
COPY --from=claude-deps /usr/local/bin /usr/local/bin
RUN mkdir -p /home/node/.claude /home/node/.local/share /home/node/.local/state && \
    chown -R node:node /home/node/.claude /home/node/.local
COPY --chown=node:node .claude/settings.json /home/node/.claude/settings.json
COPY --chown=node:node .claude.json /home/node/.claude.json
USER node
WORKDIR /home/node
ENTRYPOINT ["claude-agent-acp"]
