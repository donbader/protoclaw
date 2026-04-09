# Example 01: Fake Agent Bot
# Requires: docker build -t protoclaw-core --target core . (from repo root)

# Main target: protoclaw + mock-agent + system-info + debug-http
FROM protoclaw-core AS example-01
# All binaries already present in core image — no additional COPY needed

# Mock-agent standalone image (for Docker workspace mode)
FROM debian:bookworm-slim AS mock-agent
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=protoclaw-core /usr/local/bin/mock-agent /usr/local/bin/mock-agent
ENTRYPOINT ["mock-agent"]
