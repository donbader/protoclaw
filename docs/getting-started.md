# Getting Started

This guide takes you from "I found this project" to "I have my own bot running." You'll pick an example, copy it into your own repo, customize it, and deploy.

Anyclaw is infrastructure, not an agent. You bring the AI; anyclaw handles routing, subprocess supervision, crash recovery, and channel integrations.

## Prerequisites

- **Docker** and **Docker Compose** (v2) installed
- **git**
- A **Telegram bot token** — message [@BotFather](https://t.me/BotFather), send `/newbot`, copy the token (optional for the fake-agent example, required for real agents)
- An **AI agent API key** (only needed for real-agent variants — see the table below)

## Choose Your Starting Point

| Example | Agent | API Key Required | Best For |
|---------|-------|-----------------|----------|
| [`examples/01-fake-agent-telegram-bot/`](../examples/01-fake-agent-telegram-bot/) | Mock (echo + simulated thinking) | None | Verifying the setup works before touching real agents |
| [`examples/02-real-agent-telegram/opencode/`](../examples/02-real-agent-telegram/opencode/) | OpenCode (`opencode acp`) | None (uses ambient credentials) | OpenCode users who already have it configured |
| [`examples/02-real-agent-telegram/kiro/`](../examples/02-real-agent-telegram/kiro/) | Kiro CLI (`kiro-cli acp`) | `KIRO_API_KEY` or browser login | Kiro subscribers |
| [`examples/02-real-agent-telegram/claude-code/`](../examples/02-real-agent-telegram/claude-code/) | Claude Code (via ACP adapter) | `ANTHROPIC_API_KEY` | Claude / Anthropic users |

**Not sure?** Start with the fake-agent example. It has zero API requirements, runs in two commands, and shows the full message flow. You can swap in a real agent later.

## Copy the Example Into Your Own Repo

1. Create a new repo on GitHub (or wherever you host). Keep it private if it'll contain credentials.

2. Clone anyclaw to get the files, then copy your chosen example:

   ```sh
   git clone https://github.com/donbader/anyclaw.git
   cp -r anyclaw/examples/01-fake-agent-telegram-bot/ my-bot/
   cd my-bot/
   git init && git add . && git commit -m "feat: initial bot from anyclaw example"
   git remote add origin <your-repo-url>
   git push -u origin main
   ```

   Replace `01-fake-agent-telegram-bot` with whichever example you chose.

3. Copy the env template and fill it in:

   ```sh
   cp .env.example .env
   # Edit .env — add your Telegram token and any agent API keys
   ```

   The `.env` file is gitignored in all examples. Don't commit it.

## Customize `anyclaw.yaml`

Every example includes an `anyclaw.yaml` that controls agents, channels, tools, and supervisor behavior. The most common things to change when setting up your own bot:

- **Enable Telegram** — set `TELEGRAM_ENABLED=true` and `TELEGRAM_BOT_TOKEN` in `.env`, then enable the Telegram channel in `anyclaw.yaml`
- **Swap the agent** — change the `agent` field on your channel config to point at a different agent entry
- **Add or remove tools** — edit the `tools` list on the agent config

For the full config schema, all sections, and all available options, see the [Configuration Reference](../examples/02-real-agent-telegram/CONFIGURATION.md).

> **Note for real-agent variants:** `anyclaw.yaml` is baked into the Docker image at build time. After any change, rebuild with `docker compose up --build -d`.

## Deploy

```sh
docker compose up -d
```

That's it. Anyclaw pulls pre-built images from `ghcr.io/donbader/anyclaw` — no Rust compilation needed.

Verify it's running:

```sh
curl http://localhost:8080/health
# {"status":"ok"}
```

Send a test message:

```sh
curl -X POST http://localhost:8080/message \
  -H "Content-Type: application/json" \
  -d '{"message": "hello"}'
```

Then open Telegram and message your bot.

To follow logs:

```sh
docker compose logs -f
```

To stop:

```sh
docker compose down
```

## Next Steps

Once your bot is running, you can extend it:

- **Add tools** — connect MCP servers or build WASM-sandboxed tools. See [Building Extensions](./building-extensions.md).
- **Add channels** — wire up Slack, Discord, or a custom HTTP endpoint alongside Telegram. See [Building Extensions](./building-extensions.md).
- **Build a custom agent** — any ACP-compatible agent binary works. See the [ext/agents guide](../ext/agents/AGENTS.md) for the protocol details.
- **Add a new agent variant** — contributing a new variant back to the project? See [examples/02-real-agent-telegram/AGENTS.md](../examples/02-real-agent-telegram/AGENTS.md).
