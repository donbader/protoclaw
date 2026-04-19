ARG CORE_IMAGE=ghcr.io/donbader/anyclaw:latest
ARG EXT_IMAGE=ghcr.io/donbader/anyclaw-ext:latest
FROM ${CORE_IMAGE} AS core
FROM ${EXT_IMAGE} AS ext

# Deps stage: claude-agent-acp (ACP adapter wrapping Claude Agent SDK)
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS claude-deps
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates sudo && rm -rf /var/lib/apt/lists/*
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
COPY --from=core /usr/local/bin/anyclaw /usr/local/bin/anyclaw
COPY --from=ext /usr/local/bin/channels/debug-http /usr/local/bin/channels/debug-http
COPY --from=ext /usr/local/bin/channels/telegram /usr/local/bin/channels/telegram
COPY --from=ext /usr/local/bin/tools/system-info /usr/local/bin/tools/system-info
RUN mkdir -p /workspace && chown node:node /workspace
WORKDIR /workspace
COPY --chown=node:node anyclaw.yaml /anyclaw/anyclaw.yaml
USER node
ENTRYPOINT ["anyclaw", "--config", "/anyclaw/anyclaw.yaml"]

# Claude Code agent image (for Docker workspace mode)
# Rebased on claude-deps to avoid redundant apt-get update on every build.
FROM claude-deps AS claude-code-agent
RUN usermod -l agent-claude-code -d /home/agent-claude-code -m node && \
    groupmod -n agent-claude-code node && \
    echo "agent-claude-code ALL=(ALL) NOPASSWD: /usr/bin/apt-get" >> /etc/sudoers.d/agent-claude-code
RUN mkdir -p /home/agent-claude-code/.claude /home/agent-claude-code/.local/share /home/agent-claude-code/.local/state /home/agent-claude-code/workspace && \
    chown -R agent-claude-code:agent-claude-code /home/agent-claude-code
COPY --chown=agent-claude-code:agent-claude-code .claude/settings.json /home/agent-claude-code/.claude/settings.json
COPY --chown=agent-claude-code:agent-claude-code .claude.json /home/agent-claude-code/.claude.json
USER agent-claude-code
WORKDIR /home/agent-claude-code
ENTRYPOINT ["claude-agent-acp"]
