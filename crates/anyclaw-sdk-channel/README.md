# anyclaw-sdk-channel

Build messaging channel integrations for [anyclaw](https://github.com/donbader/anyclaw) — implement the `Channel` trait and the SDK handles all JSON-RPC framing, the initialize handshake, and bidirectional message routing.

[![crates.io](https://img.shields.io/crates/v/anyclaw-sdk-channel.svg)](https://crates.io/crates/anyclaw-sdk-channel)
[![docs.rs](https://img.shields.io/docsrs/anyclaw-sdk-channel)](https://docs.rs/anyclaw-sdk-channel)

> ⚠️ **Unstable** — APIs may change between releases.

## Quick Example

```rust
use anyclaw_sdk_channel::{Channel, ChannelCapabilities, ChannelHarness, ChannelSdkError, ChannelSendMessage};
use anyclaw_sdk_types::{ChannelRequestPermission, DeliverMessage, PermissionResponse};
use tokio::sync::mpsc;

struct MyChannel {
    outbound: Option<mpsc::Sender<ChannelSendMessage>>,
}

impl Channel for MyChannel {
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities { streaming: true, rich_text: false }
    }

    async fn on_ready(
        &mut self,
        outbound: mpsc::Sender<ChannelSendMessage>,
        _permission_tx: mpsc::Sender<PermissionResponse>,
    ) -> Result<(), ChannelSdkError> {
        self.outbound = Some(outbound);
        // Start your platform listener (HTTP server, webhook, polling loop, etc.)
        Ok(())
    }

    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
        // Render the agent's response to your platform
        Ok(())
    }

    async fn show_permission_prompt(&mut self, _req: ChannelRequestPermission) -> Result<(), ChannelSdkError> {
        // Show permission UI — return immediately, respond async via permission_tx
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    ChannelHarness::new(MyChannel { outbound: None })
        .run_stdio()
        .await
        .unwrap();
}
```

`on_initialize`, `handle_unknown`, and `on_session_created` have default no-op implementations. Override them only if needed.

## Going Further

- **[docs.rs](https://docs.rs/anyclaw-sdk-channel)** — full API reference, trait contract, `ChannelTester` for unit testing
- **[Building extensions guide](https://github.com/donbader/anyclaw/blob/main/docs/building-extensions.md)** — end-to-end walkthrough for building and deploying a channel
- **[debug-http reference implementation](https://github.com/donbader/anyclaw/tree/main/ext/channels/debug-http)** — a complete working channel (HTTP + SSE + permission handling)

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
