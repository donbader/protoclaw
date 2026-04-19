# Shared dev workflow for agent variants
# Run from a variant directory: make -f ../dev/Makefile dev
#
# Expects docker-compose.yml and docker-compose.dev.yml in CWD.

ROOT := $(shell git rev-parse --show-toplevel)
COMPOSE := docker compose -f docker-compose.yml -f docker-compose.dev.yml
VERSION := $(shell git -C $(ROOT) describe --tags --always --dirty --match 'v*' 2>/dev/null || echo "unknown")

.PHONY: dev dev-base up down logs

dev: dev-base up

dev-base:
	docker build -f $(ROOT)/Dockerfile --build-arg PROFILE=debug --build-arg ANYCLAW_VERSION=$(VERSION) --target core -t anyclaw-dev-core:latest $(ROOT)
	docker build -f $(ROOT)/ext/Dockerfile --build-arg PROFILE=debug --target ext -t anyclaw-dev-ext:latest $(ROOT)

up:
	$(COMPOSE) up -d --build

down:
	$(COMPOSE) down

logs:
	$(COMPOSE) logs -f anyclaw
