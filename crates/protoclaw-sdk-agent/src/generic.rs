use crate::adapter::AgentAdapter;
use async_trait::async_trait;

#[derive(Debug, Default, Clone)]
pub struct GenericAcpAdapter;

#[async_trait]
impl AgentAdapter for GenericAcpAdapter {}
