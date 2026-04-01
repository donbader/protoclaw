# protoclaw — the idea

## One Line

Infrastructure sidecar that connects AI agents to the outside world.

## What It Is

Protoclaw sits around an existing AI agent (opencode, claude-code, gemini-cli — any ACP-speaking process) and gives it two things it doesn't have: channels to talk to users, and tools to act on the world.

The agent is a black box. Protoclaw doesn't manage its thinking, its subagents, or its streaming. The agent handles all of that. Protoclaw just keeps the pipes connected.

## The Protocol

Protoclaw speaks ACP (Agent Client Protocol) — the standard for IDE↔agent communication. JSON-RPC 2.0 over stdio. Protoclaw is the **client**, the agent is the server.

```
protoclaw (client)                    agent (server)
    │                                      │
    │──── initialize ────────────────────►│  negotiate capabilities
    │──── session/new (mcpServers:[...]) ►│  start conversation, register tools
    │──── session/prompt ────────────────►│  user says something
    │◄─── session/update (streaming) ─────│  agent thinks, acts, responds
    │◄─── session/request_permission ─────│  "can I do X?" → ask channel user
    │──── session/cancel ────────────────►│  user cancels
```

## The Architecture

```
                                    ┌─────────────────┐
                                    │    Supervisor    │
                                    └──┬──────┬──────┬┘
                                       │      │      │
                              ┌────────┘      │      └────────┐
                              ▼               ▼               ▼
                      ┌──────────────┐ ┌─────────────┐ ┌─────────────┐
                      │    Agents    │ │  Channels   │ │    Tools    │
                      │   Manager   │ │   Manager   │ │   Manager   │
                      └──────────────┘ └─────────────┘ └─────────────┘
                              │               │               │
                              ▼               ▼               ▼
                      ┌──────────────┐  ┌──────────┐  ┌──────────────┐
                      │ Primary Agent│  │ Telegram  │  │  Workspace   │
                      │    (ACP)     │  │ Slack     │  │  MCPs (WASM) │
                      │              │  │ Discord   │  │  Custom Tools│
                      │ opencode     │  │ IPC       │  │    (WASM)    │
                      │ claude-code  │  │ debug-http│  │              │
                      │ gemini-cli   │  │           │  │              │
                      └──────────────┘  └──────────┘  └──────────────┘
```

## Three Managers

**Agents Manager** — Owns the primary agent process. Spawns it, restarts it on crash, feeds it messages from channels. The agent is long-lived, one per workspace. Protoclaw doesn't care what happens inside.

**Channels Manager** — Bridges the outside world to the agent. Each channel is a subprocess. Telegram, Slack, Discord, IPC (agent-to-agent), debug-http. All normalize to the same message format in and out. Also handles `session/request_permission` — when the agent asks "can I edit this file?", the channels manager forwards it to the user's channel and returns their answer.

**Tools Manager** — An MCP server (or set of them) that protoclaw stands up and registers with the agent at `session/new`. The agent discovers tools through normal MCP tool discovery and calls them directly. Protoclaw just brokers the introduction.

```
1. Protoclaw boots
2. Tools Manager starts its MCP server(s)
3. Agents Manager calls session/new with mcpServers: [tools-manager, ...user MCPs...]
4. Agent connects to all MCP servers, discovers tools
5. Agent calls tools directly — no routing through protoclaw's pipe
```

Three kinds of tools:

- **Workspace** — controlling Docker containers, file access, environment setup
- **MCPs** — user-configured MCP servers, optionally WASM-sandboxed
- **Custom Tools** — user-defined in config, WASM-sandboxed

## Three SDKs

**Agents SDK** — Adapter layer for different agent runtimes. Each agent (opencode, claude-code, gemini-cli) speaks ACP slightly differently. The SDK normalizes that into a common interface protoclaw can manage. Write an adapter, plug in any agent.

**Channels SDK** — Trait for building channel extensions. Each channel is a subprocess over JSON-RPC stdio. The SDK handles framing and protocol. The channel author handles the platform (Telegram API, Slack socket, Discord gateway).

**Tools SDK** — Trait for building tool extensions as MCP servers. Native subprocesses or WASM modules. Declares tools (name, schema, description) and executes calls. WASM tools run sandboxed with resource limits.

## How They Work Together

### Supervisor

Controls the lifecycle of all three managers. Single authority.

```
start()              — boot all managers in order: Tools → Agents → Channels
stop()               — graceful shutdown in reverse: Channels → Agents → Tools
health_check()       — poll each manager, restart on failure
on_crash(manager)    — exponential backoff restart (100ms → 30s), crash-loop protection
```

### Agents Manager

Owns the ACP connection to the primary agent. One agent per workspace.

```
send_message(session_id, message)     — forward channel message → session/prompt
cancel(session_id)                    — send session/cancel to agent
get_mcp_configs()                     — ask Tools Manager for MCP server details
```

Callbacks (agent → protoclaw):

```
on_update(session_id, update)         — agent is streaming → forward to Channels Manager
on_permission(session_id, request)    — agent asks permission → forward to Channels Manager, block until answer
```

### Channels Manager

Owns all channel subprocesses. Routes messages in and out.

```
route_inbound(channel_id, message)    — channel received a user message → forward to Agents Manager
route_outbound(session_id, update)    — agent streaming update → forward to the right channel
request_permission(session_id, req)   — show permission prompt to user, return their answer
```

### Tools Manager

Stands up MCP servers. The agent talks to them directly after introduction.

```
get_mcp_configs()                     — return connection details for all managed MCP servers
start_servers()                       — boot workspace tools, WASM sandboxes, user MCPs
stop_servers()                        — tear down all MCP servers
```

### The Flows

**User sends a message:**

```
Telegram → Channels Manager.route_inbound() → Agents Manager.send_message() → session/prompt → Agent
```

**Agent responds (streaming):**

```
Agent → session/update → Agents Manager.on_update() → Channels Manager.route_outbound() → Telegram
```

**Agent asks permission:**

```
Agent → session/request_permission → Agents Manager.on_permission()
  → Channels Manager.request_permission() → Telegram shows inline keyboard
  → user taps "Allow" → response flows back → Agent continues
```

**Agent calls a tool:**

```
Agent → MCP tools/call → Tools Manager's MCP server (direct, no protoclaw routing)
```

## The Primary Agent

This is NOT something protoclaw builds. This IS opencode. IS claude-code. IS gemini-cli.

The agent receives messages, thinks, calls tools, spawns subagents, streams responses — all on its own. Protoclaw's job is to keep it alive and connected. That's it.

## What Protoclaw Does NOT Do

- Manage agent internals (thinking, subagents, context windows)
- Own the streaming protocol
- Build agents
- Implement AI logic of any kind

## Why This Exists

AI agents are powerful but isolated. They can think and code but can't receive a Telegram message, can't survive crashes, can't share tools safely. Protoclaw is the missing infrastructure layer — the thing that turns a CLI agent into a connected, supervised, tool-equipped service.

