# Shared dev workflow for agent variants
# Run from a variant directory: make -f ../shared/Makefile dev
#
# Expects docker-compose.yml and docker-compose.dev.yml in CWD.

ROOT := $(shell git rev-parse --show-toplevel)
COMPOSE := docker compose -f docker-compose.yml -f docker-compose.dev.yml

.PHONY: dev dev-base up down logs

dev: dev-base up

dev-base:
	docker build -f $(ROOT)/Dockerfile --build-arg PROFILE=debug --target builder-export -t anyclaw-dev-base:latest $(ROOT)

up:
	$(COMPOSE) up -d --build

down:
	$(COMPOSE) down

logs:
	$(COMPOSE) logs -f anyclaw
