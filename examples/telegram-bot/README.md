# Protoclaw Telegram Bot Example

Run a Telegram bot powered by an AI agent with a single command. This example uses protoclaw as the infrastructure sidecar to connect an ACP-compatible agent to Telegram.

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and Docker Compose v2
- A Telegram Bot Token — message [@BotFather](https://core.telegram.org/bots#botfather) on Telegram, send `/newbot`, follow the prompts, and copy the token
- An ACP-compatible agent binary (e.g., `opencode`) available in the container

## Quick Start

1. Copy this directory to your project (or clone the repo)

2. Create your environment file:
   ```sh
   cp .env.example .env
   ```

3. Edit `.env` and set your bot token:
   ```
   TELEGRAM_BOT_TOKEN=your-actual-token-here
   ```

4. Start the bot:
   ```sh
   docker compose up
   ```
   The first build takes a few minutes to compile Rust. Subsequent starts use cached layers.

5. Message your bot on Telegram — it should respond through the connected agent.

## Configuration

### protoclaw.toml

| Section | Purpose |
|---------|---------|
| `[agent]` | Which agent binary to run and its arguments |
| `[[channels]]` | Channel subprocesses — this example uses `telegram-channel` |
| `[supervisor]` | Restart policy, health checks, shutdown behavior |

### Changing the agent

Edit `protoclaw.toml` to use a different ACP-compatible agent:

```toml
[agent]
binary = "your-agent"
args = ["your", "args"]
```

The agent binary must be available inside the container. Add it to the Dockerfile or mount it as a volume.

### Adding MCP tool servers

Give the agent access to tools by adding `[[mcp_servers]]` entries:

```toml
[[mcp_servers]]
name = "filesystem"
binary = "mcp-server-filesystem"
args = ["--root", "/workspace"]
```

## Troubleshooting

**Bot doesn't respond**
- Verify `TELEGRAM_BOT_TOKEN` is correct in `.env`
- Check logs: `docker compose logs -f`
- Ensure the bot isn't already running elsewhere (Telegram only allows one connection per token)

**Build fails**
- Ensure Docker has at least 4GB memory allocated (Rust compilation is memory-intensive)
- Try `docker compose build --no-cache` for a clean build

**Agent crashes on startup**
- Check the agent binary is available in the container's `$PATH`
- Set `RUST_LOG=debug` in `docker-compose.yml` for detailed output
- Review logs: `docker compose logs protoclaw`

## Files

| File | Purpose |
|------|---------|
| `docker-compose.yml` | Container orchestration — builds and runs protoclaw |
| `protoclaw.toml` | Protoclaw configuration — agent, channels, supervisor |
| `.env.example` | Environment variable template — copy to `.env` |
| `README.md` | This file |
