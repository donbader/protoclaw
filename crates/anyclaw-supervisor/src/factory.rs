use std::sync::Arc;

use anyclaw_config::{AnyclawConfig, SessionStoreConfig};
use anyclaw_core::{
    DynContextStore, DynSessionStore, Manager, ManagerError, ManagerHandle, NoopContextStore,
    NoopSessionStore, SqliteSessionStore,
};
use tokio_util::sync::CancellationToken;

use anyclaw_agents::{AgentsCommand, AgentsManager};
use anyclaw_channels::ChannelsManager;
use anyclaw_core::ChannelEvent;
use anyclaw_tools::{ToolsCommand, ToolsManager};

pub(crate) struct Stores {
    pub session: Arc<dyn DynSessionStore>,
    pub context: Arc<dyn DynContextStore>,
}

pub(crate) fn build_stores(config: &SessionStoreConfig) -> Stores {
    match config {
        SessionStoreConfig::None => Stores {
            session: Arc::new(NoopSessionStore),
            context: Arc::new(NoopContextStore),
        },
        SessionStoreConfig::Sqlite(sqlite_cfg) => {
            let result = match &sqlite_cfg.path {
                Some(path) => SqliteSessionStore::open(path),
                None => SqliteSessionStore::open_in_memory(),
            };
            match result {
                Ok(s) => {
                    let shared = Arc::new(s);
                    Stores {
                        session: Arc::clone(&shared) as Arc<dyn DynSessionStore>,
                        context: shared as Arc<dyn DynContextStore>,
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to open store, falling back to noop");
                    Stores {
                        session: Arc::new(NoopSessionStore),
                        context: Arc::new(NoopContextStore),
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_manager(
    name: &str,
    config: &AnyclawConfig,
    tools_tx: &tokio::sync::mpsc::Sender<ToolsCommand>,
    tools_rx: Option<tokio::sync::mpsc::Receiver<ToolsCommand>>,
    agents_cmd_tx: Option<&tokio::sync::mpsc::Sender<AgentsCommand>>,
    channel_events_tx: Option<tokio::sync::mpsc::Sender<ChannelEvent>>,
    channel_events_rx: Option<tokio::sync::mpsc::Receiver<ChannelEvent>>,
    stores: Option<Stores>,
) -> ManagerKind {
    match name {
        "tools" => {
            let m = ToolsManager::new(
                config.tools_manager.tools.clone(),
                config.tools_manager.tools_server_host.clone(),
            )
            .with_cmd_rx(tools_rx.expect("tools_rx required for tools manager"));
            ManagerKind::Tools(m)
        }
        "agents" => {
            let handle = anyclaw_core::ManagerHandle::new(tools_tx.clone());
            let session_store = stores
                .map(|s| s.session)
                .unwrap_or_else(|| Arc::new(NoopSessionStore));
            let mut agents = AgentsManager::new(config.agents_manager.clone(), handle)
                .with_log_level(config.log_level.clone())
                .with_session_store(session_store);
            if let SessionStoreConfig::Sqlite(ref sqlite_cfg) = config.session_store {
                agents = agents.with_session_ttl_secs(i64::from(sqlite_cfg.ttl_days) * 86400);
            }
            if let Some(tx) = channel_events_tx {
                agents = agents.with_channels_sender(tx);
            }
            ManagerKind::Agents(Box::new(agents))
        }
        "channels" => {
            let tx = agents_cmd_tx.expect("agents_cmd_tx required for channels manager");
            let agents_handle = ManagerHandle::new(tx.clone());
            let default_agent = config.default_agent_name().unwrap_or("default").to_string();
            let context_store = stores
                .map(|s| s.context)
                .unwrap_or_else(|| Arc::new(NoopContextStore));
            let mut cm = ChannelsManager::new(
                config.channels_manager.channels.clone(),
                config.channels_manager.init_timeout_secs,
                config.channels_manager.exit_timeout_secs,
                default_agent,
            )
            .with_agents_handle(agents_handle)
            .with_permission_timeout(config.supervisor.permission_timeout_secs)
            .with_log_level(config.log_level.clone())
            .with_context_store(context_store);
            if let Some(rx) = channel_events_rx {
                cm = cm.with_channel_events_rx(rx);
            }
            ManagerKind::Channels(cm)
        }
        _ => unreachable!("unknown manager: {name}"),
    }
}

pub(crate) enum ManagerKind {
    Tools(ToolsManager),
    Agents(Box<AgentsManager>),
    Channels(ChannelsManager),
}

impl ManagerKind {
    pub(crate) async fn start(&mut self) -> Result<(), ManagerError> {
        match self {
            Self::Tools(m) => m.start().await,
            Self::Agents(m) => m.start().await,
            Self::Channels(m) => m.start().await,
        }
    }

    pub(crate) async fn run(self, cancel: CancellationToken) -> Result<(), ManagerError> {
        match self {
            Self::Tools(m) => m.run(cancel).await,
            Self::Agents(m) => m.run(cancel).await,
            Self::Channels(m) => m.run(cancel).await,
        }
    }
}
