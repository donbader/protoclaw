# Dev workflow for OpenCode variant
# Usage: make dev       — build from source + start
#        make dev-base  — rebuild anyclaw binaries only
#        make down      — stop everything

ROOT := $(shell git -C ../../../ rev-parse --show-toplevel)
COMPOSE := docker compose -f docker-compose.yml -f docker-compose.dev.yml

.PHONY: dev dev-base up down logs

dev: dev-base up

dev-base:
	docker build -f ../Dockerfile.dev-builder -t anyclaw-dev-base:latest $(ROOT)

up:
	$(COMPOSE) up -d --build

down:
	$(COMPOSE) down

logs:
	$(COMPOSE) logs -f anyclaw
