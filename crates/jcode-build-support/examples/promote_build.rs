//! One-off: publish the current selfdev build and promote it to shared-server,
//! then let `jcode server reload` (non-forced) pick it up.
fn main() -> anyhow::Result<()> {
    let repo = std::env::current_dir()?;
    let source = jcode_build_support::current_source_state(&repo)?;
    let published = jcode_build_support::publish_local_current_build(&repo)?;
    let prev = jcode_build_support::promote_version_to_shared_server(&source.version_label)?;
    println!(
        "published {} -> {}; shared-server {} -> {}",
        source.version_label,
        published.display(),
        prev.unwrap_or_else(|| "<none>".into()),
        source.version_label
    );
    Ok(())
}
