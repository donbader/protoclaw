# Use pre-built protoclaw image as source for binaries
FROM ghcr.io/donbader/protoclaw:latest AS core

# OpenCode target — core binaries + Node.js + opencode-ai
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS example-02-opencode
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g opencode-ai@latest
COPY --from=core /usr/local/bin/protoclaw /usr/local/bin/protoclaw
COPY --from=core /usr/local/bin/debug-http /usr/local/bin/debug-http
COPY --from=core /usr/local/bin/telegram-channel /usr/local/bin/telegram-channel
COPY --from=core /usr/local/bin/system-info /usr/local/bin/system-info
RUN mkdir -p /usr/local/bin/agents
COPY --from=core /usr/local/bin/opencode-wrapper /usr/local/bin/agents/opencode
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Claude Code target — core binaries + Node.js + claude-code
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS example-02-claude-code
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g @anthropic-ai/claude-code
COPY --from=core /usr/local/bin/protoclaw /usr/local/bin/protoclaw
COPY --from=core /usr/local/bin/debug-http /usr/local/bin/debug-http
COPY --from=core /usr/local/bin/telegram-channel /usr/local/bin/telegram-channel
COPY --from=core /usr/local/bin/system-info /usr/local/bin/system-info
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# OpenCode agent image (for Docker workspace mode)
# node:20-slim
FROM node:20-slim@sha256:f93745c153377ee2fbbdd6e24efcd03cd2e86d6ab1d8aa9916a3790c40313a55 AS opencode-agent
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g opencode-ai@latest
COPY --from=core /usr/local/bin/opencode-wrapper /usr/local/bin/opencode-wrapper
COPY .opencode /home/node/.config/opencode
USER node
WORKDIR /home/node
ENTRYPOINT ["opencode-wrapper"]
