ARG BUILDER_IMAGE=ghcr.io/donbader/anyclaw-builder:latest
FROM ${BUILDER_IMAGE} AS builder

# Deps stage: opencode-ai with pruned node_modules
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS opencode-deps
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates sudo && rm -rf /var/lib/apt/lists/*
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
RUN mkdir -p /workspace && chown node:node /workspace
WORKDIR /workspace
COPY --chown=node:node anyclaw.yaml /anyclaw/anyclaw.yaml
USER node
ENTRYPOINT ["anyclaw", "--config", "/anyclaw/anyclaw.yaml"]

# OpenCode agent image (for Docker workspace mode)
# Rebased on opencode-deps to avoid redundant apt-get update on every build.
FROM opencode-deps AS opencode-agent
RUN usermod -l agent-opencode -d /home/agent-opencode -m node && \
    groupmod -n agent-opencode node && \
    echo "agent-opencode ALL=(ALL) NOPASSWD: /usr/bin/apt-get" >> /etc/sudoers.d/agent-opencode
# opencode-config/ contains minimal committed config — customize as needed.
# Glob trick: README.md always exists so COPY never fails; opencode-config is optional.
RUN mkdir -p /home/agent-opencode/.config/opencode
COPY README.md opencode-config* /tmp/build-staging/
RUN if [ -f /tmp/build-staging/opencode.json ]; then \
      cp /tmp/build-staging/opencode.json /home/agent-opencode/.config/opencode/ && \
      cp /tmp/build-staging/package*.json /home/agent-opencode/.config/opencode/ 2>/dev/null || true; \
    fi && rm -rf /tmp/build-staging
RUN if [ -f /home/agent-opencode/.config/opencode/package.json ]; then \
      cd /home/agent-opencode/.config/opencode && npm install --omit=dev; \
    fi
RUN mkdir -p /home/agent-opencode/.local/share /home/agent-opencode/.local/state /home/agent-opencode/workspace && \
    chown -R agent-opencode:agent-opencode /home/agent-opencode
USER agent-opencode
WORKDIR /home/agent-opencode
ENTRYPOINT ["opencode", "acp"]
