use super::*;

/// Process-wide handle to the live agent provider.
///
/// The memory sidecar ([`crate::sidecar::Sidecar`]) needs to make small,
/// cheap model calls (rerank / relevance / extraction). It has dedicated fast
/// paths for OpenAI (codex-spark) and Claude (haiku) OAuth, but jcode also runs
/// on Copilot, Antigravity, Gemini, Cursor, Bedrock, and OpenRouter. For those
/// providers there is no standalone sidecar HTTP client, so the sidecar falls
/// back to *this* handle and dispatches through the already-working
/// [`Provider::complete_simple`] path. `Server::new` registers the active
/// provider here at startup.
static ACTIVE_PROVIDER: RwLock<Option<Arc<dyn Provider>>> = RwLock::new(None);
static ACTIVE_PROVIDER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Register the live agent provider so background helpers (memory sidecar) can
/// reach whatever provider the user is actually running on. Safe to call more
/// than once; the most recent registration wins.
pub fn set_active_provider(provider: Arc<dyn Provider>) {
    *ACTIVE_PROVIDER
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(provider);
    ACTIVE_PROVIDER_GENERATION.fetch_add(1, Ordering::Release);
}

pub(crate) fn active_provider_generation() -> u64 {
    ACTIVE_PROVIDER_GENERATION.load(Ordering::Acquire)
}

/// Fetch the registered active provider, if any. Returns a forked handle so the
/// caller gets an independent provider instance (per the [`Provider::fork`]
/// contract) that will not interfere with the main agent's model selection.
pub fn active_provider_fork() -> Option<Arc<dyn Provider>> {
    ACTIVE_PROVIDER
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .map(|p| p.fork())
}

/// Fetch the registered active provider, if any, and return a fork pinned to
/// `model_spec`. Delegates to [`Provider::fork_with_model_spec`], so the fork is
/// an independent instance and the live agent's own selection is never mutated.
/// Returns `None` when no provider is registered, and propagates a
/// fork/`set_model` failure (e.g. Copilot's transient `try_write`) so callers
/// can fall back explicitly instead of silently running the wrong model.
pub fn active_provider_fork_with_model_spec(model_spec: &str) -> Option<Result<Arc<dyn Provider>>> {
    ACTIVE_PROVIDER
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .map(|p| p.fork_with_model_spec(model_spec))
}

/// Provider-agnostic streaming idle timeout: max seconds to wait between
/// streamed chunks/events before treating the connection as dead. Resolved
/// from `[provider] stream_idle_timeout_secs` / `JCODE_STREAM_IDLE_TIMEOUT_SECS`
/// (default 180). Shared by every streaming provider path so slow reasoning
/// models that think silently for minutes don't trip a premature timeout on
/// one transport but not another (issue #434).
pub fn stream_idle_timeout() -> std::time::Duration {
    let secs = crate::config::config()
        .provider
        .stream_idle_timeout_secs
        .max(1);
    std::time::Duration::from_secs(secs)
}

/// Whether reasoning deltas should be persisted in session history for later
/// provider context reconstruction.
///
/// Display is controlled separately by `display.show_thinking`. Persist only
/// when a provider request builder can safely send the stored block back in
/// the provider-native shape. Anthropic is included only because we preserve
/// its thinking signatures in `ContentBlock::AnthropicThinking`.
pub fn stores_reasoning_content_for_context(provider_name: &str) -> bool {
    if !crate::config::config().provider.preserve_reasoning_context {
        return false;
    }
    matches!(
        provider_name.to_ascii_lowercase().as_str(),
        "openrouter" | "anthropic" | "openai"
    )
}
