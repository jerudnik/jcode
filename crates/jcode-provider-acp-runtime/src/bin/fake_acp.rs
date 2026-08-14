use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

fn scenario() -> String {
    std::env::var("JCODE_FAKE_ACP_SCENARIO").unwrap_or_else(|_| "happy".to_string())
}

fn json_env(name: &str) -> Result<Option<Value>> {
    std::env::var_os(name)
        .map(|value| {
            serde_json::from_str(&value.to_string_lossy())
                .with_context(|| format!("parse {name} as JSON"))
        })
        .transpose()
}

fn append_log(value: &Value) -> Result<()> {
    let Some(path) = std::env::var_os("JCODE_FAKE_ACP_LOG") else {
        return Ok(());
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .context("open fake ACP log")?;
    writeln!(file, "{value}").context("write fake ACP log")
}

fn send(value: Value) -> Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{value}").context("write fake ACP response")?;
    stdout.flush().context("flush fake ACP response")
}

fn send_raw(value: &str) -> Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{value}").context("write fake ACP raw response")?;
    stdout.flush().context("flush fake ACP raw response")
}

fn response(id: Value, result: Value) -> Result<()> {
    send(json!({"jsonrpc":"2.0", "id":id, "result":result}))
}

fn model_state(current: &str) -> Value {
    json!({
        "currentModelId": current,
        "availableModels": [
            {"modelId":"model-a", "name":"Model A"},
            {"modelId":"model-b", "name":"Model B"}
        ]
    })
}

fn config_options(scenario: &str) -> Value {
    if scenario == "config_catalog" {
        json!([{
            "id":"model",
            "name":"Model",
            "category":"model",
            "type":"select",
            "currentValue":"model-b",
            "options":[
                {"value":"model-a", "name":"Model A"},
                {"value":"model-b", "name":"Model B"}
            ],
            "_meta":{"config":"kept"}
        }])
    } else {
        json!([{
            "id":"thinking",
            "name":"Thinking",
            "category":"thought_level",
            "type":"select",
            "currentValue":"low",
            "options":[
                {"value":"low", "name":"Low"},
                {"value":"high", "name":"High"}
            ]
        }])
    }
}

fn finish_prompt(id: Value) -> Result<()> {
    send(json!({
        "jsonrpc":"2.0",
        "method":"_fake/unknown_notification",
        "params":{"ignored":true}
    }))?;
    send(json!({
        "jsonrpc":"2.0",
        "method":"session/update",
        "params":{
            "sessionId":"fake-session-new",
            "update":{
                "sessionUpdate":"agent_thought_chunk",
                "content":{"type":"text", "text":"thinking"}
            }
        }
    }))?;
    send(json!({
        "jsonrpc":"2.0",
        "method":"session/update",
        "params":{
            "sessionId":"fake-session-new",
            "update":{
                "sessionUpdate":"tool_call",
                "toolCallId":"tool-status",
                "title":"provider tool running"
            }
        }
    }))?;
    send(json!({
        "jsonrpc":"2.0",
        "method":"session/update",
        "params":{
            "sessionId":"fake-session-new",
            "update":{
                "sessionUpdate":"agent_message_chunk",
                "content":{"type":"text", "text":"ACP_TEST_OK"}
            }
        }
    }))?;
    response(id, json!({"stopReason":"end_turn"}))
}

fn main() -> Result<()> {
    let marker =
        std::env::var_os("JCODE_FAKE_MARKER").map(|marker| marker.to_string_lossy().into_owned());
    append_log(&json!({
        "fakeProcess": {
            "args": std::env::args().skip(1).collect::<Vec<_>>(),
            "cwd": std::env::current_dir().context("fake ACP cwd")?,
            "marker": marker,
            "pid": std::process::id()
        }
    }))?;
    let stdin = std::io::stdin();
    let mut lines = BufReader::new(stdin.lock()).lines();
    while let Some(line) = lines.next() {
        let line = line.context("read fake ACP request")?;
        let value: Value = serde_json::from_str(&line).context("valid JSON-RPC request")?;
        append_log(&value)?;
        let Some(method) = value.get("method").and_then(Value::as_str) else {
            continue;
        };
        let id = value.get("id").cloned().unwrap_or(Value::Null);
        let scenario = scenario();
        match method {
            "initialize" => {
                match scenario.as_str() {
                    "stderr_failure" => {
                        eprintln!(
                            "bounded diagnostic before initialize failure {}",
                            "x".repeat(256)
                        );
                        std::process::exit(17);
                    }
                    "child_exit" => std::process::exit(18),
                    "oversized_rpc_error" => {
                        send(json!({
                            "jsonrpc":"2.0",
                            "id":id,
                            "error":{
                                "code":-32000,
                                "message":format!("oversized protocol diagnostic {}", "x".repeat(256))
                            }
                        }))?;
                        continue;
                    }
                    "malformed_json" => {
                        send_raw("{this is not json-rpc")?;
                        std::process::exit(19);
                    }
                    "cancel_initialize" => {
                        std::thread::sleep(Duration::from_millis(400));
                        append_log(&json!({"cancelInitializeCompleted":true}))?;
                    }
                    "slow_initialize" => std::thread::sleep(Duration::from_secs(2)),
                    _ => {}
                }
                let meta = if scenario == "config_catalog" {
                    json!({"vendor": {"arbitrary": [1, true, null]}})
                } else {
                    json!({
                        "modelState": json_env("JCODE_FAKE_ACP_MODEL_STATE")?
                            .unwrap_or_else(|| model_state("model-a")),
                        "vendor": {"arbitrary": [1, true, null]}
                    })
                };
                let auth_methods = json_env("JCODE_FAKE_ACP_AUTH_METHODS")?.unwrap_or_else(|| {
                    if scenario == "auth_missing" {
                        json!([{"id":"other", "name":"Other"}])
                    } else {
                        json!([
                            {"id":"other", "name":"Other"},
                            {"id":"cached", "name":"Cached"}
                        ])
                    }
                });
                response(
                    id,
                    json!({
                        "protocolVersion": if scenario == "unsupported_version" { 999 } else { 1 },
                        "agentCapabilities": {
                            "loadSession": true,
                            "sessionCapabilities": {"resume": {}}
                        },
                        "authMethods": auth_methods,
                        "agentInfo": {"name":"scriptable-fake-acp", "version":"1.0.0"},
                        "_meta": meta
                    }),
                )?;
            }
            "authenticate" => response(id, json!({}))?,
            "session/new" => {
                let mut result = json!({
                    "sessionId":"fake-session-new",
                    "configOptions": config_options(&scenario)
                });
                if scenario != "config_catalog" {
                    result["models"] = model_state("model-a");
                }
                response(id, result)?;
            }
            "session/resume" => {
                if scenario == "resume_hang" {
                    continue;
                } else if scenario == "unknown_resume" {
                    send(json!({
                        "jsonrpc":"2.0",
                        "id":id,
                        "error":{"code":-32000, "message":"unknown session"}
                    }))?;
                } else {
                    response(
                        id,
                        json!({
                            "models": model_state("model-b"),
                            "configOptions": config_options(&scenario)
                        }),
                    )?;
                }
            }
            "session/set_model" => {
                if scenario == "set_model_hang" {
                    continue;
                }
                response(id, json!({}))?;
            }
            "session/set_config_option" => response(id, json!({"configOptions":[]}))?,
            "session/prompt" => {
                if scenario == "prompt_hang" || scenario == "cancel_stall" {
                    continue;
                }
                if scenario == "permission" || scenario == "permission_no_reject" {
                    send(json!({
                        "jsonrpc":"2.0",
                        "id":900,
                        "method":"session/request_permission",
                        "params":{
                            "sessionId":"fake-session-new",
                            "toolCall":{
                                "toolCallId":"permission-tool",
                                "title":"Choose an action",
                                "kind":"execute",
                                "rawInput":{"command":["echo","safe"]},
                                "content":[{"type":"content", "content":{"type":"text", "text":"details"}}],
                                "locations":[{"path":"/tmp/example", "line":7, "_meta":{"location":"kept"}}]
                            },
                            "options": if scenario == "permission_no_reject" {
                                json!([
                                    {"optionId":"allow_once", "name":"Allow once", "kind":"allow_once"}
                                ])
                            } else {
                                json!([
                                    {"optionId":"allow_once", "name":"Allow once", "kind":"allow_once"},
                                    {"optionId":"allow_always", "name":"Allow always", "kind":"allow_always"},
                                    {"optionId":"reject_once", "name":"Reject once", "kind":"reject_once"},
                                    {"optionId":"plan.choice/opaque:β-17", "name":"Use alternate plan", "kind":"allow_once", "_meta":{"choice":3}}
                                ])
                            },
                            "_meta":{"permission":{"nested":[1,2,3]}}
                        }
                    }))?;
                    loop {
                        let permission_line = lines
                            .next()
                            .transpose()
                            .context("read permission response")?
                            .context("permission response line")?;
                        let permission_message: Value = serde_json::from_str(&permission_line)
                            .context("valid permission response")?;
                        append_log(&permission_message)?;
                        if permission_message.get("id") == Some(&json!(900))
                            && permission_message.get("result").is_some()
                        {
                            break;
                        }
                        if permission_message.get("method") != Some(&json!("session/cancel")) {
                            bail!("unexpected message while permission was pending");
                        }
                    }
                }
                finish_prompt(id)?;
            }
            "session/cancel" => {
                if scenario == "cancel_stall" {
                    std::thread::sleep(Duration::from_secs(5));
                    append_log(&json!({"cancelStallCompleted":true}))?;
                }
                break;
            }
            other => bail!("unexpected ACP method: {other}"),
        }
    }
    Ok(())
}
