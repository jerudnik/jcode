use super::*;
use jcode_base::message::{ContentBlock, Message, Role};

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set_path(key: &'static str, value: &std::path::Path) -> Self {
        let previous = std::env::var_os(key);
        jcode_base::env::set_var(key, value);
        Self { key, previous }
    }

    fn set_value(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        jcode_base::env::set_var(key, value);
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        jcode_base::env::remove_var(key);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            jcode_base::env::set_var(self.key, previous);
        } else {
            jcode_base::env::remove_var(self.key);
        }
    }
}

#[test]
fn available_models_include_gemini_defaults() {
    let provider = GeminiProvider::new();
    let models = provider.available_models();
    assert!(models.contains(&"gemini-3-pro-preview"));
    assert!(models.contains(&"gemini-3.1-pro-preview"));
    assert!(models.contains(&"gemini-2.5-pro"));
    assert!(models.contains(&"gemini-2.5-flash"));
}

#[test]
fn set_model_accepts_gemini_models() {
    let provider = GeminiProvider::new();
    provider.set_model("gemini-2.5-flash").unwrap();
    assert_eq!(provider.model(), "gemini-2.5-flash");
}

#[test]
fn detects_model_not_found_errors() {
    let err = anyhow::anyhow!(
        "Gemini request generateContent failed (HTTP 404 Not Found): {{\"error\":{{\"status\":\"NOT_FOUND\",\"message\":\"Requested entity was not found.\"}}}}"
    );
    assert!(is_gemini_model_not_found_error(&err));
}

#[test]
fn fallback_models_skip_current_model() {
    assert_eq!(
        gemini_fallback_models("gemini-2.5-flash"),
        vec![
            "gemini-3.1-pro-preview",
            "gemini-3-pro-preview",
            "gemini-2.5-pro",
            "gemini-3-flash-preview",
            "gemini-2.0-flash",
        ]
    );
}

#[test]
fn extract_gemini_model_ids_discovers_nested_models() {
    let response = json!({
        "routing": {
            "manual": {
                "models": [
                    {"id": "gemini-3-pro-preview"},
                    {"name": "gemini-3.1-pro-preview"}
                ]
            },
            "auto": ["gemini-3-flash-preview", "not-a-model"]
        }
    });

    assert_eq!(
        extract_gemini_model_ids(&response),
        vec![
            "gemini-3.1-pro-preview".to_string(),
            "gemini-3-pro-preview".to_string(),
            "gemini-3-flash-preview".to_string(),
        ]
    );
}

#[test]
fn available_models_display_prefers_discovered_models_and_current_model() {
    let provider = GeminiProvider::new();
    provider.set_model("gemini-4-pro-preview").unwrap();
    *provider.fetched_models.write().unwrap() = vec![
        "gemini-3-flash-preview".to_string(),
        "gemini-3-pro-preview".to_string(),
    ];

    assert_eq!(
        provider.available_models_display(),
        vec![
            "gemini-3-pro-preview".to_string(),
            "gemini-3-flash-preview".to_string(),
            "gemini-4-pro-preview".to_string(),
        ]
    );
}

#[test]
fn available_models_display_without_discovery_uses_current_model_only() {
    let _guard = jcode_base::storage::lock_test_env();
    let temp = tempfile::TempDir::new().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());

    let provider = GeminiProvider::new();
    provider.set_model("gemini-4-pro-preview").unwrap();

    assert_eq!(
        provider.available_models_display(),
        vec!["gemini-4-pro-preview".to_string()]
    );
}

#[test]
fn available_models_display_seeds_from_persisted_catalog() {
    let _guard = jcode_base::storage::lock_test_env();
    let temp = tempfile::TempDir::new().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());

    let path = GeminiProvider::persisted_catalog_path().expect("catalog path");
    jcode_base::storage::write_json(
        &path,
        &PersistedCatalog {
            models: vec!["gemini-3-pro-preview".to_string()],
            fetched_at_rfc3339: chrono::Utc::now().to_rfc3339(),
        },
    )
    .expect("write persisted catalog");

    let provider = GeminiProvider::new();
    assert!(
        provider
            .available_models_display()
            .contains(&"gemini-3-pro-preview".to_string())
    );
}

#[test]
fn build_contents_replays_thought_signature_on_function_call() {
    // Gemini 3 (Antigravity Cloud Code backend) rejects function calls that
    // omit the original thoughtSignature on later turns. Verify the signature
    // captured on the ToolUse block is replayed verbatim on the functionCall
    // part. A later unsigned call inherits the most recent real signature so the
    // backend (which 400s a fully-unsigned turn) accepts it (issue #339).
    let messages = vec![
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_sig".to_string(),
                name: "read".to_string(),
                input: json!({"path":"README.md"}),
                thought_signature: Some("SIGNATURE_ABC".to_string()),
            }],
            timestamp: None,
            tool_duration_ms: None,
        },
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_nosig".to_string(),
                name: "bash".to_string(),
                input: json!({"command":"ls"}),
                thought_signature: None,
            }],
            timestamp: None,
            tool_duration_ms: None,
        },
    ];

    let contents = build_contents(&messages);
    assert_eq!(
        contents[0].parts[0].thought_signature.as_deref(),
        Some("SIGNATURE_ABC"),
        "signature must be replayed on the matching function call part"
    );
    assert_eq!(
        contents[1].parts[0].thought_signature.as_deref(),
        Some("SIGNATURE_ABC"),
        "an unsigned later call must inherit the most recent real signature so \
         the backend does not reject a fully-unsigned turn"
    );
}

#[test]
fn build_contents_replays_every_signature_across_multi_tool_history() {
    // Regression guard for the Antigravity/Cloud Code 400
    // ("Function call is missing a thought_signature ... position 5"): the
    // backend validates *every* functionCall in the replayed history, not just
    // the latest one. A multi-turn transcript where an earlier tool_use drops
    // its signature is exactly what triggers the field failure, so assert that
    // each captured signature survives serialization onto its matching part.
    let signatures = ["SIG_A", "SIG_B", "SIG_C"];
    let mut messages = Vec::new();
    for (idx, sig) in signatures.iter().enumerate() {
        messages.push(Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: format!("call_{idx}"),
                name: "bash".to_string(),
                input: json!({ "command": format!("echo {idx}") }),
                thought_signature: Some(sig.to_string()),
            }],
            timestamp: None,
            tool_duration_ms: None,
        });
        messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: format!("call_{idx}"),
                content: format!("out {idx}"),
                is_error: Some(false),
            }],
            timestamp: None,
            tool_duration_ms: None,
        });
    }

    let contents = build_contents(&messages);
    let replayed: Vec<Option<&str>> = contents
        .iter()
        .flat_map(|content| content.parts.iter())
        .filter(|part| part.function_call.is_some())
        .map(|part| part.thought_signature.as_deref())
        .collect();
    assert_eq!(
        replayed,
        vec![Some("SIG_A"), Some("SIG_B"), Some("SIG_C")],
        "every functionCall in the history must carry its captured thought_signature, \
         not just the most recent one"
    );
}

#[test]
fn build_contents_carries_first_signature_onto_unsigned_same_turn_siblings() {
    // Issue #339: when Gemini-3 emits MULTIPLE function calls in ONE turn it
    // signs only the first; the siblings persist without a signature. The
    // Antigravity/Cloud Code backend then rejects the unsigned siblings with
    // "Function call is missing a thought_signature ... position N". Verify the
    // first call's signature is carried forward onto same-turn siblings that
    // lack one (the backend accepts a replayed signature on sibling calls).
    let messages = vec![Message {
        role: Role::Assistant,
        content: vec![
            ContentBlock::ToolUse {
                id: "call_todo".to_string(),
                name: "todo".to_string(),
                input: json!({ "items": ["a", "b"] }),
                thought_signature: Some("SIG_TURN_1".to_string()),
            },
            ContentBlock::ToolUse {
                id: "call_bash".to_string(),
                name: "bash".to_string(),
                input: json!({ "command": "ls" }),
                thought_signature: None,
            },
            ContentBlock::ToolUse {
                id: "call_write".to_string(),
                name: "write".to_string(),
                input: json!({ "path": "a.txt", "content": "hi" }),
                thought_signature: None,
            },
        ],
        timestamp: None,
        tool_duration_ms: None,
    }];

    let contents = build_contents(&messages);
    let replayed: Vec<Option<&str>> = contents
        .iter()
        .flat_map(|content| content.parts.iter())
        .filter(|part| part.function_call.is_some())
        .map(|part| part.thought_signature.as_deref())
        .collect();
    assert_eq!(
        replayed,
        vec![Some("SIG_TURN_1"), Some("SIG_TURN_1"), Some("SIG_TURN_1")],
        "every functionCall in a multi-call turn must carry a signature so the \
         backend does not reject unsigned siblings"
    );
}

#[test]
fn build_contents_carries_signature_forward_across_turns_for_unsigned_calls() {
    // Issue #339: the Antigravity/Cloud Code backend 400s an assistant turn
    // whose function calls are ALL unsigned. A later turn made entirely of
    // locally synthesized / unsigned tool calls (auto-poke continuation, batch,
    // manual tool use, or an imported pre-signature session) must inherit the
    // most recent real signature from earlier in the conversation so at least
    // one call carries a signature and the backend accepts the turn.
    let messages = vec![
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "turn1".to_string(),
                name: "read".to_string(),
                input: json!({ "path": "README.md" }),
                thought_signature: Some("SIG_TURN_1".to_string()),
            }],
            timestamp: None,
            tool_duration_ms: None,
        },
        Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "turn1".to_string(),
                content: "ok".to_string(),
                is_error: Some(false),
            }],
            timestamp: None,
            tool_duration_ms: None,
        },
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "turn2".to_string(),
                name: "bash".to_string(),
                input: json!({ "command": "ls" }),
                thought_signature: None,
            }],
            timestamp: None,
            tool_duration_ms: None,
        },
    ];

    let contents = build_contents(&messages);
    let last_turn_sig = contents
        .last()
        .and_then(|content| content.parts.first())
        .and_then(|part| part.thought_signature.as_deref());
    assert_eq!(
        last_turn_sig,
        Some("SIG_TURN_1"),
        "a fully-unsigned later turn must inherit the most recent real signature \
         so the backend does not reject it"
    );
}

#[test]
fn build_contents_leaves_unsigned_calls_unsigned_when_no_prior_signature_exists() {
    // If the conversation has never produced a real signature there is nothing
    // to carry; we must not fabricate one out of thin air.
    let messages = vec![Message {
        role: Role::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "call".to_string(),
            name: "bash".to_string(),
            input: json!({ "command": "ls" }),
            thought_signature: None,
        }],
        timestamp: None,
        tool_duration_ms: None,
    }];

    let contents = build_contents(&messages);
    assert_eq!(
        contents[0].parts[0].thought_signature, None,
        "with no prior signature in the conversation, an unsigned call stays unsigned"
    );
}

#[test]
fn build_contents_preserves_tool_calls_and_results() {
    let messages = vec![
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_1".to_string(),
                name: "read".to_string(),
                input: json!({"path":"README.md"}),
                thought_signature: None,
            }],
            timestamp: None,
            tool_duration_ms: None,
        },
        Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: "ok".to_string(),
                is_error: None,
            }],
            timestamp: None,
            tool_duration_ms: None,
        },
    ];

    let contents = build_contents(&messages);
    assert_eq!(contents.len(), 2);
    assert_eq!(contents[0].role, "model");
    assert_eq!(contents[1].role, "user");
    assert_eq!(
        contents[0].parts[0].function_call.as_ref().unwrap().name,
        "read"
    );
    assert_eq!(
        contents[1].parts[0]
            .function_response
            .as_ref()
            .unwrap()
            .name,
        "read"
    );
}

#[test]
fn build_contents_normalizes_non_object_tool_call_args_for_gemini_struct() {
    let messages = vec![Message {
        role: Role::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "call_primitive".to_string(),
            name: "read".to_string(),
            input: json!(20),
            thought_signature: None,
        }],
        timestamp: None,
        tool_duration_ms: None,
    }];

    let contents = build_contents(&messages);
    assert_eq!(
        contents[0].parts[0].function_call.as_ref().unwrap().args,
        json!({})
    );
}

#[test]
fn build_tools_uses_function_declarations() {
    let defs = vec![ToolDefinition {
        name: "read".to_string(),
        description: "Read a file".to_string(),
        input_schema: json!({"type":"object","properties":{"path":{"type":"string"}}}),
    }];

    let built = build_tools(&defs).unwrap();
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].function_declarations[0].name, "read");
}

fn schema_contains_key(schema: &Value, key: &str) -> bool {
    match schema {
        Value::Object(map) => {
            map.contains_key(key) || map.values().any(|value| schema_contains_key(value, key))
        }
        Value::Array(items) => items.iter().any(|value| schema_contains_key(value, key)),
        _ => false,
    }
}

#[test]
fn build_tools_rewrites_const_for_gemini_schema_compatibility() {
    let defs = vec![ToolDefinition {
        name: "batch".to_string(),
        description: "Batch tools".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "tool_calls": {
                    "type": "array",
                    "items": {
                        "oneOf": [
                            {
                                "type": "object",
                                "properties": {
                                    "tool": { "type": "string", "const": "read" },
                                    "file_path": { "type": "string" }
                                },
                                "required": ["tool", "file_path"]
                            }
                        ]
                    }
                }
            }
        }),
    }];

    let built = build_tools(&defs).expect("gemini tools");
    let parameters = &built[0].function_declarations[0].parameters;

    assert!(!schema_contains_key(parameters, "const"));
    assert_eq!(
        parameters["properties"]["tool_calls"]["items"]["oneOf"][0]["properties"]["tool"]["enum"],
        json!(["read"])
    );
}

#[test]
fn build_tools_strips_additional_properties_for_gemini_schema_compatibility() {
    // The Gemini Code Assist generateContent endpoint rejects `additionalProperties`
    // (and other draft-JSON-Schema keywords) with HTTP 400, so build_tools must
    // strip them recursively while preserving the rest of the schema.
    let defs = vec![ToolDefinition {
        name: "read".to_string(),
        description: "Reads a file".to_string(),
        input_schema: json!({
            "type": "object",
            "$schema": "http://json-schema.org/draft-07/schema#",
            "properties": {
                "file_path": { "type": "string" },
                "opts": {
                    "type": "object",
                    "properties": { "limit": { "type": "integer" } },
                    "additionalProperties": false
                }
            },
            "required": ["file_path"],
            "additionalProperties": false
        }),
    }];

    let built = build_tools(&defs).expect("gemini tools");
    let parameters = &built[0].function_declarations[0].parameters;

    assert!(!schema_contains_key(parameters, "additionalProperties"));
    assert!(!schema_contains_key(parameters, "$schema"));
    // Real schema content is preserved.
    assert_eq!(
        parameters["properties"]["file_path"]["type"],
        json!("string")
    );
    assert_eq!(
        parameters["properties"]["opts"]["properties"]["limit"]["type"],
        json!("integer")
    );
    assert_eq!(parameters["required"], json!(["file_path"]));
}

#[test]
fn parses_prompt_feedback_block_reason() {
    let response: VertexGenerateContentResponse = serde_json::from_value(json!({
        "promptFeedback": {
            "blockReason": "PROHIBITED_CONTENT",
            "blockReasonMessage": "Prompt violated policy"
        }
    }))
    .expect("parse prompt feedback");

    let feedback = response.prompt_feedback.expect("missing prompt feedback");
    assert_eq!(feedback.block_reason.as_deref(), Some("PROHIBITED_CONTENT"));
    assert_eq!(
        feedback.block_reason_message.as_deref(),
        Some("Prompt violated policy")
    );
}

#[test]
fn parses_candidate_finish_message() {
    let response: VertexGenerateContentResponse = serde_json::from_value(json!({
        "candidates": [
            {
                "finishReason": "SAFETY",
                "finishMessage": "Response blocked by safety filters"
            }
        ]
    }))
    .expect("parse candidate");

    let candidate = response
        .candidates
        .expect("missing candidates")
        .into_iter()
        .next()
        .expect("missing first candidate");
    assert_eq!(candidate.finish_reason.as_deref(), Some("SAFETY"));
    assert_eq!(
        candidate.finish_message.as_deref(),
        Some("Response blocked by safety filters")
    );
}

#[test]
fn auth_mode_prefers_api_key_when_present() {
    let _guard = jcode_base::storage::lock_test_env();
    let temp = tempfile::TempDir::new().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let _google = EnvVarGuard::unset("GOOGLE_API_KEY");
    let _force = EnvVarGuard::unset("JCODE_GEMINI_FORCE_OAUTH");
    let _key = EnvVarGuard::set_value("GEMINI_API_KEY", "test-developer-key");

    match GeminiProvider::auth_mode() {
        GeminiAuthMode::ApiKey(key) => assert_eq!(key, "test-developer-key"),
        GeminiAuthMode::Oauth => panic!("expected API-key auth mode when GEMINI_API_KEY is set"),
    }
}

#[test]
fn auth_mode_force_oauth_overrides_api_key() {
    let _guard = jcode_base::storage::lock_test_env();
    let temp = tempfile::TempDir::new().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let _google = EnvVarGuard::unset("GOOGLE_API_KEY");
    let _key = EnvVarGuard::set_value("GEMINI_API_KEY", "test-developer-key");
    let _force = EnvVarGuard::set_value("JCODE_GEMINI_FORCE_OAUTH", "1");

    assert!(matches!(GeminiProvider::auth_mode(), GeminiAuthMode::Oauth));
}

#[test]
fn auth_mode_defaults_to_oauth_without_api_key() {
    let _guard = jcode_base::storage::lock_test_env();
    let temp = tempfile::TempDir::new().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let _key = EnvVarGuard::unset("GEMINI_API_KEY");
    let _google = EnvVarGuard::unset("GOOGLE_API_KEY");
    let _force = EnvVarGuard::unset("JCODE_GEMINI_FORCE_OAUTH");

    assert!(matches!(GeminiProvider::auth_mode(), GeminiAuthMode::Oauth));
}

#[test]
fn developer_api_base_url_defaults_to_generativelanguage() {
    let _guard = jcode_base::storage::lock_test_env();
    let _endpoint = EnvVarGuard::unset("GEMINI_API_ENDPOINT");
    let _version = EnvVarGuard::unset("GEMINI_API_VERSION");

    assert_eq!(
        GeminiProvider::developer_api_base_url(),
        "https://generativelanguage.googleapis.com/v1beta"
    );
}

#[test]
fn developer_api_base_url_honors_env_overrides() {
    let _guard = jcode_base::storage::lock_test_env();
    let _endpoint = EnvVarGuard::set_value("GEMINI_API_ENDPOINT", "https://example.test/");
    let _version = EnvVarGuard::set_value("GEMINI_API_VERSION", "/v9/");

    assert_eq!(
        GeminiProvider::developer_api_base_url(),
        "https://example.test/v9"
    );
}

#[test]
fn developer_api_response_parses_without_code_assist_envelope() {
    // The Developer API returns the bare generateContent body; ensure it maps
    // onto the same response type the Code Assist envelope yields.
    let response: VertexGenerateContentResponse = serde_json::from_value(json!({
        "candidates": [
            {
                "content": {
                    "role": "model",
                    "parts": [{ "text": "hello from developer api" }]
                },
                "finishReason": "STOP"
            }
        ],
        "usageMetadata": {
            "promptTokenCount": 3,
            "candidatesTokenCount": 5
        }
    }))
    .expect("parse developer api response");

    let candidate = response
        .candidates
        .expect("missing candidates")
        .into_iter()
        .next()
        .expect("missing first candidate");
    assert_eq!(candidate.finish_reason.as_deref(), Some("STOP"));
    let text = candidate
        .content
        .expect("missing content")
        .parts
        .into_iter()
        .next()
        .and_then(|part| part.text)
        .expect("missing text");
    assert_eq!(text, "hello from developer api");
}

#[test]
fn system_instruction_tool_guard_only_applies_with_tools() {
    // Without tools, the system instruction is passed through unchanged.
    let plain = super::build_system_instruction_with_tool_guard("You are helpful.", false)
        .expect("system instruction present");
    let plain_text = plain.parts[0].text.clone().unwrap();
    assert_eq!(plain_text, "You are helpful.");
    assert!(!plain_text.contains("Function calling"));

    // With tools, the MALFORMED_FUNCTION_CALL prevention guidance is appended.
    let guarded = super::build_system_instruction_with_tool_guard("You are helpful.", true)
        .expect("system instruction present");
    let guarded_text = guarded.parts[0].text.clone().unwrap();
    assert!(guarded_text.starts_with("You are helpful."));
    assert!(guarded_text.contains("Function calling"));
    assert!(guarded_text.contains("native function call, not code"));
    assert!(guarded_text.contains("default_api."));
}

#[test]
fn system_instruction_tool_guard_with_empty_system_still_emits_guidance() {
    // An empty base system prompt plus tools must still carry the guard so the
    // model is steered away from pseudo-code tool calls.
    let guarded = super::build_system_instruction_with_tool_guard("", true)
        .expect("guard-only instruction present");
    let text = guarded.parts[0].text.clone().unwrap();
    assert!(text.contains("Function calling"));

    // Empty system and no tools yields no instruction at all.
    assert!(super::build_system_instruction_with_tool_guard("", false).is_none());
}

/// Issues #482 / #518: carrying a signature forward only works if the history
/// contains at least one real signature. A conversation where *nothing* was ever
/// signed has none to carry, so every turn 400s with no way out. The recovery
/// policy must downgrade tool calls to text so the turn can complete.
#[test]
fn fully_unsigned_history_has_no_signature_to_replay() {
    let messages = unsigned_tool_history();
    let contents = jcode_provider_gemini::build_contents(&messages);
    let signed_calls = contents
        .iter()
        .flat_map(|content| content.parts.iter())
        .filter(|part| part.function_call.is_some() && part.thought_signature.is_some())
        .count();
    assert_eq!(
        signed_calls, 0,
        "a fully unsigned history cannot produce a signed call; this is the \
         dead-end the downgrade policy exists to escape"
    );
}

#[test]
fn downgrade_policy_removes_every_function_call_part() {
    let messages = unsigned_tool_history();
    let contents = jcode_provider_gemini::build_contents_with_signature_policy(
        &messages,
        jcode_provider_gemini::SignaturePolicy::DowngradeToolCallsToText,
    );
    let parts: Vec<_> = contents
        .iter()
        .flat_map(|content| content.parts.iter())
        .collect();
    assert!(
        parts
            .iter()
            .all(|part| part.function_call.is_none() && part.function_response.is_none()),
        "downgrade must leave no functionCall/functionResponse part for the \
         backend to reject"
    );
    // The content must survive as text, otherwise the retry loses the whole
    // conversation and the model repeats work.
    let text = parts
        .iter()
        .filter_map(|part| part.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("bash"), "tool name should survive: {text}");
    assert!(text.contains("ls"), "tool args should survive: {text}");
    assert!(
        text.contains("total 0"),
        "tool result should survive: {text}"
    );
}

#[test]
fn missing_thought_signature_errors_are_recognized_from_backend_bodies() {
    // Exact bodies reported in #482 and #518.
    assert!(jcode_provider_gemini::is_missing_thought_signature_error(
        "Antigravity generateContent failed (HTTP 400 Bad Request): {\"error\": {\"code\": 400, \
         \"message\": \"Function call is missing a thought_signature in functionCall parts. This \
         is required for tools to work correctly, and missing thought_signature may lead to \
         degraded model performance. Additional data, function call [default_api:bash], position \
         7.\", \"status\": \"INVALID_ARGUMENT\"}}"
    ));
    assert!(jcode_provider_gemini::is_missing_thought_signature_error(
        "missing a thoughtSignature"
    ));
    // Unrelated failures must not trigger the lossy downgrade retry.
    assert!(!jcode_provider_gemini::is_missing_thought_signature_error(
        "Antigravity generateContent failed (HTTP 429): rate limit exceeded"
    ));
    assert!(!jcode_provider_gemini::is_missing_thought_signature_error(
        "MALFORMED_FUNCTION_CALL"
    ));
}

/// An assistant tool call plus its result, with no thought signature anywhere.
fn unsigned_tool_history() -> Vec<Message> {
    vec![
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call1".to_string(),
                name: "bash".to_string(),
                input: json!({ "command": "ls" }),
                thought_signature: None,
            }],
            timestamp: None,
            tool_duration_ms: None,
        },
        Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call1".to_string(),
                content: "total 0".to_string(),
                is_error: Some(false),
            }],
            timestamp: None,
            tool_duration_ms: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Code Assist transient-response retries (429 / 5xx before any output).
// ---------------------------------------------------------------------------

mod transient_retry {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, mpsc};
    use tokio_stream::StreamExt;

    /// `(status, headers, body)` for one scripted response.
    type ScriptedResponse = (u16, Vec<(&'static str, String)>, String);

    /// A scripted fake Code Assist server. Each accepted connection is
    /// answered with the next `(status, headers, body)` entry; the request
    /// text is forwarded on the returned channel so tests can count attempts.
    fn spawn_scripted_server(
        responses: Vec<ScriptedResponse>,
    ) -> (String, mpsc::Receiver<String>, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake code assist server");
        let addr = listener.local_addr().expect("fake server addr");
        let (request_tx, request_rx) = mpsc::channel();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_thread = hits.clone();
        std::thread::spawn(move || {
            for (status, headers, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("set read timeout");
                let mut request = vec![0u8; 65536];
                let n = stream.read(&mut request).unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..n]).into_owned();
                hits_thread.fetch_add(1, Ordering::SeqCst);
                let _ = request_tx.send(request);
                let reason = match status {
                    200 => "OK",
                    400 => "Bad Request",
                    429 => "Too Many Requests",
                    500 => "Internal Server Error",
                    503 => "Service Unavailable",
                    _ => "Status",
                };
                let mut response = format!("HTTP/1.1 {status} {reason}\r\n");
                for (name, value) in headers {
                    response.push_str(&format!("{name}: {value}\r\n"));
                }
                response.push_str(&format!(
                    "Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                ));
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        (format!("http://{addr}"), request_rx, hits)
    }

    /// Fields drop in declaration order: env guards restore their values
    /// before the test-env lease is released, so a following test cannot
    /// observe (or be clobbered by) this sandbox's environment.
    struct Sandbox {
        _home: EnvVarGuard,
        _endpoint: EnvVarGuard,
        _force_oauth: EnvVarGuard,
        _gemini_key: EnvVarGuard,
        _google_key: EnvVarGuard,
        _temp: tempfile::TempDir,
        _guard: jcode_base::storage::TestEnvWriteLease,
    }

    /// Point the provider at `endpoint` with a valid (non-expired) OAuth
    /// token on disk so `post_json` reaches the fake server.
    fn sandbox(endpoint: &str) -> Sandbox {
        let guard = jcode_base::storage::lock_test_env();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let endpoint = EnvVarGuard::set_value("CODE_ASSIST_ENDPOINT", endpoint);
        let force_oauth = EnvVarGuard::set_value("JCODE_GEMINI_FORCE_OAUTH", "1");
        let gemini_key = EnvVarGuard::unset("GEMINI_API_KEY");
        let google_key = EnvVarGuard::unset("GOOGLE_API_KEY");
        gemini_auth::save_tokens(&gemini_auth::GeminiTokens {
            access_token: "test-access".into(),
            refresh_token: "test-refresh".into(),
            expires_at: Utc::now().timestamp_millis() + 3_600_000,
            email: None,
        })
        .expect("save test tokens");
        Sandbox {
            _home: home,
            _endpoint: endpoint,
            _force_oauth: force_oauth,
            _gemini_key: gemini_key,
            _google_key: google_key,
            _temp: temp,
            _guard: guard,
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    }

    fn fast_policy(max_retries: u32) -> TransientRetryPolicy {
        TransientRetryPolicy {
            max_retries,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_secs(5),
        }
    }

    fn quota_body() -> String {
        json!({"error": {"code": 429, "status": "RESOURCE_EXHAUSTED", "message": "quota will reset after 0s"}}).to_string()
    }

    fn ok_body() -> String {
        json!({"ok": true}).to_string()
    }

    #[test]
    fn retry_delay_grows_exponentially_and_stops_at_limit() {
        let policy = TransientRetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(60),
        };
        let status = reqwest::StatusCode::TOO_MANY_REQUESTS;
        assert_eq!(
            transient_retry_delay(status, None, 0, &policy),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            transient_retry_delay(status, None, 1, &policy),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            transient_retry_delay(status, None, 2, &policy),
            Some(Duration::from_millis(400))
        );
        assert_eq!(transient_retry_delay(status, None, 3, &policy), None);
        assert_eq!(transient_retry_delay(status, None, 40, &policy), None);
    }

    #[test]
    fn retry_delay_honors_retry_after_and_caps_at_max() {
        let policy = TransientRetryPolicy {
            max_retries: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
        };
        let status = reqwest::StatusCode::SERVICE_UNAVAILABLE;
        assert_eq!(
            transient_retry_delay(status, Some("3"), 0, &policy),
            Some(Duration::from_secs(3))
        );
        assert_eq!(
            transient_retry_delay(status, Some("120"), 0, &policy),
            Some(Duration::from_secs(10))
        );
        let http_date = (Utc::now() + chrono::Duration::seconds(4)).to_rfc2822();
        let delay = transient_retry_delay(status, Some(&http_date), 0, &policy).expect("delay");
        assert!(
            delay >= Duration::from_secs(2) && delay <= Duration::from_secs(5),
            "{delay:?}"
        );
        assert_eq!(
            transient_retry_delay(status, Some("garbage"), 1, &policy),
            Some(Duration::from_millis(200))
        );
        assert_eq!(transient_retry_delay(status, Some("3"), 5, &policy), None);
    }

    #[test]
    fn retry_delay_only_applies_to_429_and_5xx() {
        let policy = fast_policy(5);
        for status in [
            reqwest::StatusCode::BAD_REQUEST,
            reqwest::StatusCode::UNAUTHORIZED,
            reqwest::StatusCode::FORBIDDEN,
            reqwest::StatusCode::NOT_FOUND,
        ] {
            assert_eq!(transient_retry_delay(status, Some("1"), 0, &policy), None);
        }
        for status in [
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            reqwest::StatusCode::BAD_GATEWAY,
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert!(
                transient_retry_delay(status, None, 0, &policy).is_some(),
                "{status}"
            );
        }
    }

    #[test]
    fn burst_429_then_5xx_are_retried_until_success() {
        let (endpoint, _rx, hits) = spawn_scripted_server(vec![
            (429, vec![], quota_body()),
            (
                503,
                vec![],
                json!({"error": {"message": "No capacity available"}}).to_string(),
            ),
            (200, vec![], ok_body()),
        ]);
        let _sandbox = sandbox(&endpoint);
        let provider = GeminiProvider::new();
        let result: Result<Value> = runtime().block_on(provider.post_json_with_policy(
            "generateContent",
            &json!({}),
            &fast_policy(5),
            &|| false,
        ));
        let value = result.expect("request should succeed after transient retries");
        assert_eq!(value, json!({"ok": true}));
        assert_eq!(hits.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn attempts_are_bounded_and_final_error_reports_exhaustion() {
        let (endpoint, _rx, hits) = spawn_scripted_server(vec![
            (429, vec![], quota_body()),
            (429, vec![], quota_body()),
            (429, vec![], quota_body()),
            (200, vec![], ok_body()),
        ]);
        let _sandbox = sandbox(&endpoint);
        let provider = GeminiProvider::new();
        let result: Result<Value> = runtime().block_on(provider.post_json_with_policy(
            "generateContent",
            &json!({}),
            &fast_policy(2),
            &|| false,
        ));
        let err = result.expect_err("retries must stop after max_retries");
        let text = format!("{err:#}");
        assert!(text.contains("HTTP 429"), "{text}");
        assert!(text.contains("quota will reset"), "{text}");
        assert!(text.contains("3 attempts"), "{text}");
        assert_eq!(hits.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn retry_after_header_is_honored_over_backoff() {
        let (endpoint, _rx, hits) = spawn_scripted_server(vec![
            (429, vec![("Retry-After", "1".to_string())], quota_body()),
            (200, vec![], ok_body()),
        ]);
        let _sandbox = sandbox(&endpoint);
        let provider = GeminiProvider::new();
        let started = std::time::Instant::now();
        let result: Result<Value> = runtime().block_on(provider.post_json_with_policy(
            "generateContent",
            &json!({}),
            &fast_policy(5),
            &|| false,
        ));
        result.expect("request should succeed after Retry-After wait");
        assert!(
            started.elapsed() >= Duration::from_millis(900),
            "Retry-After: 1 was not honored ({:?})",
            started.elapsed()
        );
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn cancellation_stops_retrying() {
        let (endpoint, _rx, hits) = spawn_scripted_server(vec![
            (429, vec![("Retry-After", "5".to_string())], quota_body()),
            (200, vec![], ok_body()),
        ]);
        let _sandbox = sandbox(&endpoint);
        let provider = GeminiProvider::new();
        let cancelled = AtomicBool::new(true);
        let started = std::time::Instant::now();
        let result: Result<Value> = runtime().block_on(provider.post_json_with_policy(
            "generateContent",
            &json!({}),
            &fast_policy(5),
            &|| cancelled.load(Ordering::SeqCst),
        ));
        let err = result.expect_err("cancelled request must not keep retrying");
        let text = format!("{err:#}").to_ascii_lowercase();
        assert!(text.contains("cancel"), "{text}");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn non_transient_errors_are_not_retried() {
        let (endpoint, _rx, hits) = spawn_scripted_server(vec![
            (
                400,
                vec![],
                json!({"error": {"message": "bad"}}).to_string(),
            ),
            (200, vec![], ok_body()),
        ]);
        let _sandbox = sandbox(&endpoint);
        let provider = GeminiProvider::new();
        let result: Result<Value> = runtime().block_on(provider.post_json_with_policy(
            "generateContent",
            &json!({}),
            &fast_policy(5),
            &|| false,
        ));
        let err = result.expect_err("400 must fail immediately");
        assert!(format!("{err:#}").contains("HTTP 400"));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    fn collect_events(endpoint: &str) -> (Vec<StreamEvent>, Vec<String>) {
        let _sandbox = sandbox(endpoint);
        // Skip the loadCodeAssist handshake: seed runtime state directly.
        let provider = GeminiProvider::new();
        let rt = runtime();
        rt.block_on(async {
            *provider.state.lock().await = Some(GeminiRuntimeState {
                project_id: "test-project".into(),
                session_id: "test-session".into(),
            });
        });
        let mut events = Vec::new();
        let mut errors = Vec::new();
        rt.block_on(async {
            let mut stream = provider
                .complete(&[], &[], "sys", None)
                .await
                .expect("complete returns stream");
            while let Some(event) = stream.next().await {
                match event {
                    Ok(event) => events.push(event),
                    Err(err) => errors.push(format!("{err:#}")),
                }
            }
        });
        (events, errors)
    }

    #[test]
    fn no_retry_after_streamed_text_and_tool_call() {
        // A 200 response whose body already carries text and a tool call
        // must be delivered once; the follow-up 429 must never be requested.
        let body = json!({"response": {"candidates": [{"finishReason": "STOP", "content": {"role": "model", "parts": [
            {"text": "hello"},
            {"functionCall": {"name": "read", "args": {"path": "x"}}}
        ]}}]}})
        .to_string();
        let (endpoint, _rx, hits) =
            spawn_scripted_server(vec![(200, vec![], body), (429, vec![], quota_body())]);
        let (events, errors) = collect_events(&endpoint);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, StreamEvent::TextDelta(t) if t == "hello"))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, StreamEvent::ToolUseStart { name, .. } if name == "read"))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, StreamEvent::MessageEnd { .. }))
        );
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn no_retry_after_a_successful_status_with_unusable_body() {
        // Once a 2xx body has been received the turn is committed: a
        // malformed-function-call dead turn is reported, not replayed.
        let body =
            json!({"response": {"candidates": [{"finishReason": "MALFORMED_FUNCTION_CALL"}]}})
                .to_string();
        let (endpoint, _rx, hits) =
            spawn_scripted_server(vec![(200, vec![], body), (200, vec![], ok_body())]);
        let (_events, errors) = collect_events(&endpoint);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("no usable output"), "{errors:?}");
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn complete_stops_retrying_when_stream_is_dropped() {
        let (endpoint, _rx, hits) = spawn_scripted_server(vec![
            (429, vec![("Retry-After", "1".to_string())], quota_body()),
            (200, vec![], ok_body()),
        ]);
        let _sandbox = sandbox(&endpoint);
        let provider = GeminiProvider::new();
        let rt = runtime();
        rt.block_on(async {
            *provider.state.lock().await = Some(GeminiRuntimeState {
                project_id: "test-project".into(),
                session_id: "test-session".into(),
            });
            let stream = provider
                .complete(&[], &[], "sys", None)
                .await
                .expect("complete returns stream");
            // Wait for the first attempt to land, then walk away.
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while hits.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            drop(stream);
            // Without cancellation the second attempt lands at ~1s
            // (Retry-After: 1); waiting past that proves the loop exited.
            tokio::time::sleep(Duration::from_millis(1500)).await;
        });
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
