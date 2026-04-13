use crate::adapter::AgentAdapter;

/// Default passthrough [`AgentAdapter`] that forwards all ACP messages unchanged.
///
/// Use this when no message transformation is needed.
#[derive(Debug, Default, Clone)]
pub struct GenericAcpAdapter;

impl AgentAdapter for GenericAcpAdapter {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentAdapter, DynAgentAdapter};
    use rstest::rstest;

    #[rstest]
    #[test]
    fn when_generic_adapter_default_constructed_then_it_can_be_cloned_and_debugged() {
        let adapter = GenericAcpAdapter;
        let cloned = adapter.clone();

        assert_eq!(format!("{adapter:?}"), format!("{cloned:?}"));
    }

    #[rstest]
    #[test]
    fn when_generic_adapter_boxed_as_dyn_agent_adapter_then_dyn_dispatch_is_available() {
        let _: Box<dyn DynAgentAdapter> = Box::new(GenericAcpAdapter);
    }

    #[rstest]
    #[tokio::test]
    async fn when_generic_adapter_on_initialize_params_called_from_own_module_then_passthrough() {
        let adapter = GenericAcpAdapter;
        let input = serde_json::json!({"protocolVersion": 1});

        let output = AgentAdapter::on_initialize_params(&adapter, input.clone())
            .await
            .unwrap();

        assert_eq!(output, input);
    }
}
