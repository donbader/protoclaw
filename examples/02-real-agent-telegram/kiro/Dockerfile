# syntax=docker/dockerfile:1
ARG BUILDER_IMAGE=ghcr.io/donbader/anyclaw-builder:latest
FROM ${BUILDER_IMAGE} AS builder

# Deps stage: download kiro-cli binary
# kiro-cli is a native binary distributed as a zip, not an npm package.
FROM debian:bookworm-slim@sha256:4724b8cc51e33e398f0e2e15e18d5ec2851ff0c2280647e1310bc1642182655d AS kiro-deps
ARG TARGETARCH
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl sudo unzip
RUN ARCH=$(case "${TARGETARCH}" in \
      amd64) echo "x86_64" ;; \
      arm64) echo "aarch64" ;; \
      *) echo "x86_64" ;; \
    esac) && \
    curl --proto '=https' --tlsv1.2 -sSf \
      "https://desktop-release.q.us-east-1.amazonaws.com/latest/kirocli-${ARCH}-linux.zip" \
      -o /tmp/kirocli.zip && \
    unzip /tmp/kirocli.zip -d /tmp/kirocli && \
    cp /tmp/kirocli/kirocli/bin/* /usr/local/bin/ && \
    chmod +x /usr/local/bin/kiro-cli /usr/local/bin/kiro-cli-chat /usr/local/bin/kiro-cli-term && \
    rm -rf /tmp/kirocli.zip /tmp/kirocli

# Kiro target — anyclaw sidecar + ext/ binaries (no Node.js needed)
FROM debian:bookworm-slim@sha256:4724b8cc51e33e398f0e2e15e18d5ec2851ff0c2280647e1310bc1642182655d AS example-02-kiro
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates
COPY --from=builder /usr/local/bin/anyclaw /usr/local/bin/anyclaw
COPY --from=builder /usr/local/bin/channels/debug-http /usr/local/bin/channels/debug-http
COPY --from=builder /usr/local/bin/channels/telegram /usr/local/bin/channels/telegram
COPY --from=builder /usr/local/bin/tools/system-info /usr/local/bin/tools/system-info
RUN useradd -m -s /bin/bash anyclaw && \
    mkdir -p /workspace && chown anyclaw:anyclaw /workspace
WORKDIR /workspace
COPY --chown=anyclaw:anyclaw anyclaw.yaml /workspace/anyclaw.yaml
USER anyclaw
ENTRYPOINT ["anyclaw"]

# Kiro agent image (for Docker workspace mode)
# Auth: either KIRO_API_KEY env var (Pro+) or pre-authenticated volume at ~/.local/share/kiro-cli.
# See README.md for both auth flows.
# Rebased on kiro-deps to avoid redundant apt-get update on every build.
FROM kiro-deps AS kiro-agent
RUN useradd -m -s /bin/bash kiro && \
    mkdir -p /home/kiro/.kiro /home/kiro/.local/share /home/kiro/.local/state && \
    chown -R kiro:kiro /home/kiro && \
    echo "kiro ALL=(ALL) NOPASSWD: /usr/bin/apt-get" >> /etc/sudoers.d/kiro
USER kiro
WORKDIR /home/kiro
ENTRYPOINT ["kiro-cli", "acp"]
