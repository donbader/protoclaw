# Example 02: Real Agent Bot
# Requires: docker build -t protoclaw-core --target core . (from repo root)

# OpenCode target — protoclaw-core binaries + Node.js + opencode-ai
FROM node:20-slim AS example-02-opencode
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g opencode-ai@latest
COPY --from=protoclaw-core /usr/local/bin/protoclaw /usr/local/bin/protoclaw
COPY --from=protoclaw-core /usr/local/bin/debug-http /usr/local/bin/debug-http
COPY --from=protoclaw-core /usr/local/bin/telegram-channel /usr/local/bin/telegram-channel
COPY --from=protoclaw-core /usr/local/bin/system-info /usr/local/bin/system-info
RUN mkdir -p /usr/local/bin/agents
COPY --from=protoclaw-core /usr/local/bin/opencode-wrapper /usr/local/bin/agents/opencode
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# Claude Code target — protoclaw-core binaries + Node.js + claude-code
FROM node:20-slim AS example-02-claude-code
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g @anthropic-ai/claude-code
COPY --from=protoclaw-core /usr/local/bin/protoclaw /usr/local/bin/protoclaw
COPY --from=protoclaw-core /usr/local/bin/debug-http /usr/local/bin/debug-http
COPY --from=protoclaw-core /usr/local/bin/telegram-channel /usr/local/bin/telegram-channel
COPY --from=protoclaw-core /usr/local/bin/system-info /usr/local/bin/system-info
WORKDIR /workspace
ENTRYPOINT ["protoclaw"]

# OpenCode agent image (for Docker workspace mode)
FROM node:20-slim AS opencode-agent
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN npm install -g opencode-ai@latest
COPY --from=protoclaw-core /usr/local/bin/opencode-wrapper /usr/local/bin/opencode-wrapper
COPY .opencode /home/node/.config/opencode
USER node
WORKDIR /home/node
ENTRYPOINT ["opencode-wrapper"]
