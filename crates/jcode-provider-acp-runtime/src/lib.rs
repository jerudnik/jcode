//! Shared ACP stdio subprocess runtime for jcode providers.

pub use agent_client_protocol as acp;

#[cfg(test)]
mod dependency_spike_tests {
    use super::acp;
    use serde_json::{Map, Value, json};

    #[test]
    fn permission_option_ids_round_trip_without_interpretation() {
        let option_id = "provider.choice/opaque:β-17";
        let option = acp::PermissionOption::new(
            option_id,
            "Provider-defined choice",
            acp::PermissionOptionKind::AllowOnce,
        );
        let encoded = serde_json::to_value(&option).expect("serialize permission option");
        let decoded: acp::PermissionOption =
            serde_json::from_value(encoded).expect("deserialize permission option");

        assert_eq!(decoded.option_id.0.as_ref(), option_id);
    }

    #[test]
    fn initialize_meta_round_trips_as_arbitrary_json() {
        let raw = json!({
            "modelState": {
                "currentModelId": "model-a",
                "availableModels": ["model-a", {"id": "model-b", "vendor": {"x": [1, true, null]}}]
            },
            "reasonix.io": {"futureExtension": {"nested": [1, 2, 3]}},
            "unknown": [false, {"deep": "value"}]
        });
        let meta: Map<String, Value> = raw.as_object().expect("object").clone();
        let response = acp::InitializeResponse::new(acp::ProtocolVersion::V1).meta(meta);
        let encoded = serde_json::to_value(&response).expect("serialize initialize response");
        let decoded: acp::InitializeResponse =
            serde_json::from_value(encoded).expect("deserialize initialize response");

        assert_eq!(Value::Object(decoded.meta.expect("meta")), raw);
    }

    #[test]
    fn session_resume_model_and_config_surfaces_are_representable() {
        let models = acp::SessionModelState::new(
            "model-b",
            vec![
                acp::ModelInfo::new("model-a", "Model A"),
                acp::ModelInfo::new("model-b", "Model B"),
            ],
        );
        let config = acp::SessionConfigOption::select(
            "thinking",
            "Thinking",
            "high",
            vec![
                acp::SessionConfigSelectOption::new("low", "Low"),
                acp::SessionConfigSelectOption::new("high", "High"),
            ],
        );
        let response = acp::ResumeSessionResponse::new()
            .models(models)
            .config_options(vec![config]);
        let encoded = serde_json::to_value(&response).expect("serialize resume response");
        let decoded: acp::ResumeSessionResponse =
            serde_json::from_value(encoded).expect("deserialize resume response");

        assert_eq!(
            decoded.models.expect("models").current_model_id.0.as_ref(),
            "model-b"
        );
        assert_eq!(decoded.config_options.expect("config options").len(), 1);
    }
}
