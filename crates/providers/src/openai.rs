//! OpenAI provider implementation.

use std::sync::Arc;

use tiygate_auth::bearer::BearerAuthApplier;
use tiygate_core::{
    AuthApplier, AuthMode, ProtocolEndpoint, ProtocolSuite, Provider, ProviderMetadata,
};

pub struct OpenAiProvider {
    metadata: ProviderMetadata,
}

impl Default for OpenAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAiProvider {
    pub fn new() -> Self {
        Self {
            metadata: ProviderMetadata {
                display_name: "OpenAI".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                auth_mode: AuthMode::Bearer,
                channels: vec!["default".to_string()],
                protocols: vec![
                    ProtocolSuite::OpenAiResponses.default_endpoint(),
                    ProtocolEndpoint::new(
                        ProtocolSuite::OpenAiCompatible,
                        "images-generations",
                        "v1",
                    ),
                    ProtocolEndpoint::new(ProtocolSuite::OpenAiCompatible, "images-edits", "v1"),
                ],
                defaults: serde_json::json!({}),
            },
        }
    }
}

impl Provider for OpenAiProvider {
    fn id(&self) -> &str {
        "openai"
    }

    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    fn supported_protocols(&self) -> &[ProtocolEndpoint] {
        &self.metadata.protocols
    }

    fn auth(&self) -> Arc<dyn AuthApplier> {
        Arc::new(BearerAuthApplier)
    }

    /// OpenAI supports multiple protocol suites. Images models (model id
    /// contains "image") route to the images-generations endpoint;
    /// everything else uses the Responses API.
    fn egress_protocol_for_model(&self, model_id: &str) -> ProtocolEndpoint {
        if is_image_model(model_id) {
            ProtocolEndpoint::new(ProtocolSuite::OpenAiCompatible, "images-generations", "v1")
        } else {
            ProtocolSuite::OpenAiResponses.default_endpoint()
        }
    }
}

/// Heuristic: return `true` when the model id looks like an OpenAI
/// image-generation model (e.g. `gpt-image-1`, `dall-e-3`).
fn is_image_model(model_id: &str) -> bool {
    let body = model_id.split(':').next().unwrap_or(model_id);
    let body = body.rsplit('/').next().unwrap_or(body);
    let body = body.to_ascii_lowercase();
    body.contains("image") || body.contains("dall-e")
}

inventory::submit! {
    tiygate_core::provider::ProviderRegistration {
        make: || Box::new(OpenAiProvider::new()),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_provider_metadata() {
        let provider = OpenAiProvider::new();
        assert_eq!(provider.id(), "openai");
        assert_eq!(provider.metadata().display_name, "OpenAI");
        assert_eq!(provider.metadata().base_url, "https://api.openai.com/v1");
        assert!(matches!(provider.metadata().auth_mode, AuthMode::Bearer));
        assert_eq!(provider.metadata().channels.len(), 1);
        assert_eq!(provider.metadata().channels[0], "default");
    }

    #[test]
    fn test_openai_supported_protocols() {
        let provider = OpenAiProvider::new();
        let protocols = provider.supported_protocols();
        assert!(!protocols.is_empty());
        assert_eq!(protocols[0].suite, ProtocolSuite::OpenAiResponses);
        // Images endpoints are also declared for metadata completeness.
        assert_eq!(protocols.len(), 3);
        assert!(protocols.iter().any(|p| p.name == "images-generations"));
        assert!(protocols.iter().any(|p| p.name == "images-edits"));
    }

    #[test]
    fn test_openai_auth_applier() {
        let provider = OpenAiProvider::new();
        let auth = provider.auth();
        // AuthApplier should exist
        assert!(std::sync::Arc::strong_count(&auth) >= 1);
    }

    #[test]
    fn test_openai_egress_protocol_for_image_model() {
        let provider = OpenAiProvider::new();
        let endpoint = provider.egress_protocol_for_model("gpt-image-1");
        assert_eq!(endpoint.suite, ProtocolSuite::OpenAiCompatible);
        assert_eq!(endpoint.name, "images-generations");
    }

    #[test]
    fn test_openai_egress_protocol_for_dall_e_model() {
        let provider = OpenAiProvider::new();
        let endpoint = provider.egress_protocol_for_model("dall-e-3");
        assert_eq!(endpoint.suite, ProtocolSuite::OpenAiCompatible);
        assert_eq!(endpoint.name, "images-generations");
    }

    #[test]
    fn test_openai_egress_protocol_for_chat_model() {
        let provider = OpenAiProvider::new();
        let endpoint = provider.egress_protocol_for_model("gpt-4o");
        assert_eq!(endpoint.suite, ProtocolSuite::OpenAiResponses);
    }
}
