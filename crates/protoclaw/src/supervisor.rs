use protoclaw_config::ProtoclawConfig;

pub struct Supervisor {
    config: ProtoclawConfig,
}

impl Supervisor {
    pub fn new(config: ProtoclawConfig) -> Self {
        Self { config }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let _ = self.config;
        Ok(())
    }
}
