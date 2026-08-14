use jcode_provider_core::Provider;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires an installed and configured Reasonix CLI; performs ACP discovery only"]
async fn configured_reasonix_cli_discovers_models_over_workspace_only_acp() {
    let command = std::env::var_os("REASONIX_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("reasonix"));
    let provider = jcode_provider_reasonix_runtime::provider(command);

    tokio::time::timeout(Duration::from_secs(30), provider.prefetch_models())
        .await
        .expect("Reasonix ACP discovery timed out")
        .expect("Reasonix ACP discovery failed");

    assert_ne!(provider.model(), "unknown");
    assert!(!provider.available_models_display().is_empty());
}
