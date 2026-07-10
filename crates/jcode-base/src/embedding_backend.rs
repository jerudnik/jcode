//! Pluggable embedding backends for memory retrieval.
//!
//! Memory dense-retrieval embeds two kinds of text: stored memories (passages)
//! and the current query. Historically jcode had exactly one embedder, the
//! bundled local all-MiniLM-L6-v2 ONNX model, reached directly via
//! [`crate::embedding`]. This module introduces a small abstraction so the
//! embedder can be swapped (e.g. a stronger local model, or a remote provider
//! like OpenAI when the user has an embeddings-capable key) without the rest of
//! the memory system caring which one is active.
//!
//! Design invariants:
//! - **One vector space per index.** Embeddings from different models are not
//!   comparable. Every backend reports a stable [`EmbeddingBackend::model_id`],
//!   which is stored on each `MemoryEntry` (`embedding_model`). Dense similarity
//!   only compares vectors sharing the active model id; mismatched memories stay
//!   reachable via lexical (BM25) search + RRF fusion, so switching backends
//!   never silently corrupts results.
//! - **Asymmetric query/passage formatting is per-model.** Some models (e5/bge)
//!   require instruction prefixes; others (MiniLM, OpenAI) do not. Each backend
//!   owns its own input formatting via [`EmbeddingBackend::format_query`] /
//!   [`EmbeddingBackend::format_passage`], so callers never hardcode prefixes.
//! - **Local is the always-available default.** Remote backends are opt-in and
//!   only selected when an embeddings-capable credential is present.

use anyhow::Result;

use crate::memory_types::LEGACY_EMBEDDING_MODEL;

/// A source of embedding vectors for memory retrieval.
///
/// Implementations must keep `model_id()` stable for a given vector space: it is
/// persisted alongside each embedding and used to gate cross-model comparisons.
pub trait EmbeddingBackend: Send + Sync {
    /// Stable identifier for the model/vector-space this backend produces, e.g.
    /// `"minilm-l6-v2"` or
    /// `"openai:https://api.openai.com/v1|text-embedding-3-small|dim=1536"`.
    /// Persisted on `MemoryEntry::embedding_model`.
    fn model_id(&self) -> &str;

    /// Embedding dimensionality (used for sanity checks and index metadata).
    fn dim(&self) -> usize;

    /// Embed a single text already formatted for this backend's role. Prefer
    /// [`Self::embed_query`] / [`Self::embed_passage`] which apply formatting.
    fn embed_raw(&self, text: &str) -> Result<Vec<f32>>;

    /// Apply this model's query-side formatting (e.g. an `"query: "` prefix).
    /// Default: identity (no prefix), correct for MiniLM and OpenAI.
    fn format_query(&self, text: &str) -> String {
        text.to_string()
    }

    /// Apply this model's passage-side formatting (e.g. a `"passage: "` prefix).
    /// Default: identity.
    fn format_passage(&self, text: &str) -> String {
        text.to_string()
    }

    /// Embed a retrieval query (applies query formatting).
    fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_raw(&self.format_query(text))
    }

    /// Embed a stored passage/memory (applies passage formatting).
    fn embed_passage(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_raw(&self.format_passage(text))
    }

    /// Batch-embed many passages. Backends with a remote API override this to
    /// amortize one HTTP round-trip over many inputs; the default loops over
    /// [`Self::embed_passage`] so local backends need no special-casing.
    fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed_passage(t)).collect()
    }
}

/// The bundled local ONNX embedder (currently all-MiniLM-L6-v2).
///
/// Wraps the process-wide embedder facade in [`crate::embedding`]. Requires no
/// network, no API key, and is always available, so it is the default backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalOnnxBackend;

impl EmbeddingBackend for LocalOnnxBackend {
    fn model_id(&self) -> &str {
        // Matches MemoryEntry::effective_embedding_model() for untagged legacy
        // memories, so existing embeddings remain comparable with new ones.
        LEGACY_EMBEDDING_MODEL
    }

    fn dim(&self) -> usize {
        crate::embedding::embedding_dim()
    }

    fn embed_raw(&self, text: &str) -> Result<Vec<f32>> {
        crate::embedding::embed(text)
    }

    // MiniLM is symmetric and prefix-free: default identity formatting is correct.
}

/// A remote OpenAI-compatible embeddings backend (`POST /v1/embeddings`).
///
/// Works against OpenAI proper and any OpenAI-compatible gateway that exposes
/// the same schema (the `base_url` + `Authorization: Bearer` contract). The
/// model id is stored as `"openai:<canonical-endpoint>|<model>|dim=<dim>"` so
/// its vectors never get compared against local MiniLM vectors, or against a
/// different endpoint/model/dimension (all distinct vector spaces): see the
/// module invariants. OpenAI embeddings are L2-normalized by the API, so plain
/// cosine over the returned vectors is correct.
#[derive(Debug, Clone)]
pub struct OpenAiEmbeddingBackend {
    /// Canonical vector-space identity persisted on each MemoryEntry, of the
    /// form `openai:<canonical-base-url>|<model>|dim=<effective-dim>` (never a
    /// credential). Endpoint, model, and declared dimension all participate so
    /// two distinct remote vector services never share a tag.
    model_id: String,
    /// Bare model name sent in the request body (e.g. `text-embedding-3-small`).
    model: String,
    /// Canonical API base (see `normalize_embedding_base_url`), e.g.
    /// `https://api.openai.com/v1`.
    base_url: String,
    /// Bearer credential.
    api_key: String,
    /// Effective embedding dimensionality (part of `model_id`). Every returned
    /// vector must match this exactly or the embed call hard-fails.
    dim: usize,
    /// The user's explicit `memory_embedding_dim`, if any. Drives whether the
    /// request carries a `"dimensions"` field (only for capable models).
    explicit_dim: Option<usize>,
}

/// Default OpenAI embedding model. 3-small is the cost/quality sweet spot
/// (1536-d, ~5x cheaper than ada-002 with higher MTEB scores).
pub const DEFAULT_OPENAI_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const OPENAI_EMBEDDINGS_BASE: &str = "https://api.openai.com/v1";

/// Protocol-local capability facts for the OpenAI embedding models jcode can
/// describe with confidence. `native_dim` is the model's default output width;
/// `supports_dimensions` is whether the `/v1/embeddings` request honors a
/// `"dimensions"` truncation parameter for that model.
///
/// This is intentionally NOT a general model catalog: only OpenAI's documented
/// first-party models appear. Any other (custom / gateway) model has no known
/// dimension and MUST have `memory_embedding_dim` set explicitly. The v3 vs
/// ada-002 `supports_dimensions` distinction is taken from OpenAI's public API
/// docs and is not live-verified (accepted residual risk).
struct EmbeddingModelCapabilities {
    native_dim: usize,
    supports_dimensions: bool,
}

/// Look up known capabilities for `model`, or `None` for unknown/custom models.
fn embedding_model_capabilities(model: &str) -> Option<EmbeddingModelCapabilities> {
    match model {
        "text-embedding-3-small" => Some(EmbeddingModelCapabilities {
            native_dim: 1536,
            supports_dimensions: true,
        }),
        "text-embedding-3-large" => Some(EmbeddingModelCapabilities {
            native_dim: 3072,
            supports_dimensions: true,
        }),
        "text-embedding-ada-002" => Some(EmbeddingModelCapabilities {
            native_dim: 1536,
            supports_dimensions: false,
        }),
        _ => None,
    }
}

/// Result of resolving the effective embedding dimension for a model.
///
/// A remote backend is only constructible with a `Known` dimension: it becomes
/// part of the persisted vector identity and every returned vector is validated
/// against it. A custom model with no explicit `memory_embedding_dim` yields
/// `MissingForCustomModel`, which refuses remote construction (local fallback)
/// rather than fabricating a `dim=unknown` identity that could later admit
/// mismatched-length vectors past the memory equality gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedEmbeddingDimension {
    Known(usize),
    MissingForCustomModel,
}

/// Resolve the effective dimension: explicit config wins, else known-native
/// dimension, else `MissingForCustomModel`.
fn resolve_embedding_dimension(
    model: &str,
    explicit_dim: Option<usize>,
) -> ResolvedEmbeddingDimension {
    if let Some(dim) = explicit_dim {
        return ResolvedEmbeddingDimension::Known(dim);
    }
    match embedding_model_capabilities(model) {
        Some(caps) => ResolvedEmbeddingDimension::Known(caps.native_dim),
        None => ResolvedEmbeddingDimension::MissingForCustomModel,
    }
}

/// Canonicalize the embeddings API base URL into the single string used for both
/// request construction and vector identity.
///
/// `None` selects the OpenAI default. Otherwise the input must be an absolute
/// `http`/`https` URL with a host and no userinfo, query, or fragment (any of
/// which is a configuration error). Scheme and host are lowercased, the scheme's
/// default port (80/443) is stripped, and redundant trailing path slashes are
/// trimmed (root becomes `/`). Non-default ports and interior/case-sensitive
/// path segments are preserved, so two distinct gateway paths on one host stay
/// distinguishable.
fn normalize_embedding_base_url(raw: Option<&str>) -> Result<String> {
    let raw = match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => return Ok(OPENAI_EMBEDDINGS_BASE.to_string()),
        Some(s) => s,
    };
    let parsed = url::Url::parse(raw)
        .map_err(|e| anyhow::anyhow!("invalid memory_embedding_base_url `{raw}`: {e}"))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        anyhow::bail!("memory_embedding_base_url `{raw}` must use http or https, not `{scheme}`");
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("memory_embedding_base_url `{raw}` has no host"))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        anyhow::bail!("memory_embedding_base_url `{raw}` must not contain userinfo");
    }
    if parsed.query().is_some() {
        anyhow::bail!("memory_embedding_base_url `{raw}` must not contain a query string");
    }
    if parsed.fragment().is_some() {
        anyhow::bail!("memory_embedding_base_url `{raw}` must not contain a fragment");
    }
    // `url` already lowercases scheme and host, and omits the port when it is the
    // scheme default, so `port()` (not `port_or_known_default()`) yields the
    // non-default-port stripping the spec requires.
    let scheme = scheme.to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    let mut out = format!("{scheme}://{host}");
    if let Some(port) = parsed.port() {
        out.push_str(&format!(":{port}"));
    }
    let path = parsed.path().trim_end_matches('/');
    if path.is_empty() {
        out.push('/');
    } else {
        out.push_str(path);
    }
    Ok(out)
}

/// Build the canonical persisted vector-space identity. Endpoint, bare model,
/// and effective dimension all participate; the credential never does.
fn embedding_model_id(base_url: &str, model: &str, dim: usize) -> String {
    format!("openai:{base_url}|{model}|dim={dim}")
}

impl OpenAiEmbeddingBackend {
    /// Construct a backend for `model` against `base_url` with an optional
    /// explicit dimension.
    ///
    /// Returns `Err` when the base URL is not canonicalizable or when a custom
    /// model has no explicit dimension (both refuse the remote backend so the
    /// caller can fall back to local rather than persist an unsafe identity).
    pub fn new(
        model: impl Into<String>,
        api_key: impl Into<String>,
        base_url: Option<String>,
        dim: Option<usize>,
    ) -> Result<Self> {
        let model = model.into();
        let base_url = normalize_embedding_base_url(base_url.as_deref())?;
        let effective_dim = match resolve_embedding_dimension(&model, dim) {
            ResolvedEmbeddingDimension::Known(d) => d,
            ResolvedEmbeddingDimension::MissingForCustomModel => anyhow::bail!(
                "memory_embedding_model `{model}` is not a known OpenAI model, so its embedding \
                 dimension cannot be inferred; set agents.memory_embedding_dim to the model's \
                 output width (e.g. 1024 for bge-m3)"
            ),
        };
        // An explicit dimension on a model that does not support the request
        // `dimensions` field is an identity/sanity DECLARATION, not a truncation
        // request the server will honor. Make that non-obvious semantics visible
        // once so a user does not assume server-side truncation.
        if dim.is_some()
            && !embedding_model_capabilities(&model)
                .map(|c| c.supports_dimensions)
                .unwrap_or(false)
        {
            warn_once_identity_only_dimension(&model);
        }
        Ok(Self {
            model_id: embedding_model_id(&base_url, &model, effective_dim),
            model,
            base_url,
            api_key: api_key.into(),
            dim: effective_dim,
            explicit_dim: dim,
        })
    }

    /// Build the `/v1/embeddings` request body. Pure so it is unit-testable
    /// without a network. A `"dimensions"` field is included ONLY when the user
    /// explicitly set a dimension AND the model documents support for it; for an
    /// explicit dimension on a non-capable/unknown model the field is omitted
    /// (and the caller warns once that the value is an identity declaration, not
    /// a server-side truncation request).
    fn request_body(&self, inputs: &[String]) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.model,
            "input": inputs,
        });
        let capable = embedding_model_capabilities(&self.model)
            .map(|c| c.supports_dimensions)
            .unwrap_or(false);
        if let Some(dim) = self.explicit_dim
            && capable
        {
            body["dimensions"] = serde_json::json!(dim);
        }
        body
    }

    /// POST a batch of inputs to `/embeddings` and return their vectors in order.
    ///
    /// Runs the blocking HTTP call on a dedicated thread so it is safe to call
    /// from a synchronous context (the bench) AND from inside a tokio runtime
    /// worker (the live memory path) without the nested-runtime panic that
    /// `reqwest::blocking` would otherwise trigger.
    fn embed_inputs(&self, inputs: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/embeddings", self.base_url);
        let api_key = self.api_key.clone();
        let want = inputs.len();
        let expected_dim = self.dim;
        let body = self.request_body(&inputs);

        let vectors = std::thread::scope(|scope| {
            scope
                .spawn(move || -> Result<Vec<Vec<f32>>> {
                    let client = reqwest::blocking::Client::builder()
                        .timeout(std::time::Duration::from_secs(60))
                        .build()?;
                    let resp = client
                        .post(&url)
                        .header("Authorization", format!("Bearer {api_key}"))
                        .header("Content-Type", "application/json")
                        .json(&body)
                        .send()?;
                    let status = resp.status();
                    let text = resp.text()?;
                    if !status.is_success() {
                        anyhow::bail!(
                            "OpenAI embeddings request failed ({status}): {}",
                            text.chars().take(400).collect::<String>()
                        );
                    }
                    let parsed: EmbeddingsResponse = serde_json::from_str(&text)
                        .map_err(|e| anyhow::anyhow!("parse embeddings response: {e}"))?;
                    let mut data = parsed.data;
                    // The API returns items with an `index` field; sort to be safe.
                    data.sort_by_key(|d| d.index);
                    Ok(data.into_iter().map(|d| d.embedding).collect())
                })
                .join()
                .map_err(|_| anyhow::anyhow!("OpenAI embeddings worker thread panicked"))?
        })?;

        if vectors.len() != want {
            anyhow::bail!(
                "OpenAI embeddings returned {} vectors for {} inputs",
                vectors.len(),
                want
            );
        }
        // The declared dimension is part of this backend's persisted vector
        // identity. A response vector of a different length would be stored
        // under a false `dim=N` tag and poison the memory store, so any mismatch
        // is a HARD error rather than a warning.
        validate_response_dims(&vectors, expected_dim, &self.model)?;
        Ok(vectors)
    }
}

/// Reject any batch containing a vector whose length differs from the declared
/// `expected_dim`. Pure so response safety is unit-testable without a network.
/// Every vector is checked, not just the first, so a heterogeneous batch cannot
/// smuggle a mismatched-length vector past the identity gate.
fn validate_response_dims(vectors: &[Vec<f32>], expected_dim: usize, model: &str) -> Result<()> {
    for (i, v) in vectors.iter().enumerate() {
        if v.len() != expected_dim {
            anyhow::bail!(
                "OpenAI embeddings model {model} returned a vector of length {} at index {i}, \
                 but the declared dimension is {expected_dim}; refusing to persist a \
                 mismatched-length embedding under a false identity",
                v.len()
            );
        }
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingsDatum>,
}

#[derive(serde::Deserialize)]
struct EmbeddingsDatum {
    index: usize,
    embedding: Vec<f32>,
}

impl EmbeddingBackend for OpenAiEmbeddingBackend {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed_raw(&self, text: &str) -> Result<Vec<f32>> {
        let mut out = self.embed_inputs(vec![text.to_string()])?;
        out.pop()
            .ok_or_else(|| anyhow::anyhow!("OpenAI embeddings returned no vector"))
    }

    fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let inputs: Vec<String> = texts.iter().map(|t| self.format_passage(t)).collect();
        self.embed_inputs(inputs)
    }

    // OpenAI embeddings are symmetric and prefix-free: identity formatting.
}

/// Resolve the active embedding backend.
///
/// Selects the OpenAI/openai-compatible remote backend when the user has
/// opted in (`agents.memory_embedding_backend = "openai"`) AND a valid remote
/// configuration with a resolvable credential exists; otherwise falls back to
/// the always-available local ONNX backend. Selection is conservative but
/// OBSERVABLE: a misconfigured or keyless remote setting degrades to local and
/// emits a process-once warning naming the cause, so it does not silently
/// masquerade as an intentional local choice.
pub fn active_backend() -> Box<dyn EmbeddingBackend> {
    match openai_backend_from_config() {
        Ok(Some(remote)) => return Box::new(remote),
        Ok(None) => {
            // Remote either not selected, or selected but no credential resolved.
            // Only the selected-but-keyless case is worth a warning.
            if remote_backend_selected() {
                warn_once_remote_selected_without_credential();
            }
        }
        Err(err) => warn_once_remote_config_invalid(&err),
    }
    Box::new(LocalOnnxBackend)
}

/// Build an [`OpenAiEmbeddingBackend`] from config + resolved credentials.
///
/// Returns:
/// - `Ok(None)` when remote embeddings are not selected, or are selected but no
///   credential resolves (the ordinary conservative fallback);
/// - `Err` when remote IS selected with a credential but the configuration is
///   invalid (uncanonicalizable base URL, or a custom model with no explicit
///   dimension). An invalid remote config must not silently look like a local
///   selection, and must never construct a backend that would persist an unsafe
///   identity.
///
/// By default the credential comes from `OPENAI_API_KEY`, but
/// `agents.memory_embedding_api_key_env` can point at a different bearer key for
/// OpenAI-compatible local gateways such as oMLX without making the normal
/// OpenAI provider appear configured.
pub fn openai_backend_from_config() -> Result<Option<OpenAiEmbeddingBackend>> {
    let agents = &crate::config::config().agents;
    if !agents
        .memory_embedding_backend
        .eq_ignore_ascii_case("openai")
    {
        return Ok(None);
    }
    let api_key_env = agents
        .memory_embedding_api_key_env
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("OPENAI_API_KEY");
    let Some(api_key) = crate::provider_catalog::load_api_key(
        &crate::provider_catalog::ApiKeyCredentialSource::primary_only(api_key_env, "openai.env"),
    ) else {
        // Selected but no credential: conservative local fallback (warned once
        // by the caller), not a hard error.
        return Ok(None);
    };
    let model = agents
        .memory_embedding_model
        .clone()
        .unwrap_or_else(|| DEFAULT_OPENAI_EMBEDDING_MODEL.to_string());
    let base_url = agents.memory_embedding_base_url.clone();
    let dim = agents.memory_embedding_dim;
    Ok(Some(OpenAiEmbeddingBackend::new(
        model, api_key, base_url, dim,
    )?))
}

/// Whether config selects the remote (`openai`) embedding backend. WI-4
/// guarantees the value is already exactly lowercase `local`/`openai`, but the
/// ASCII-case-insensitive check is retained defensively and matches the
/// selection check in [`openai_backend_from_config`].
fn remote_backend_selected() -> bool {
    crate::config::config()
        .agents
        .memory_embedding_backend
        .eq_ignore_ascii_case("openai")
}

/// The credential env name the remote embedding backend looks for, for use in
/// the missing-credential warning.
fn configured_embedding_key_env() -> String {
    crate::config::config()
        .agents
        .memory_embedding_api_key_env
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("OPENAI_API_KEY")
        .to_string()
}

static WARNED_MISSING_CREDENTIAL: crate::config::WarnOnce = crate::config::WarnOnce::new();
static WARNED_INVALID_CONFIG: crate::config::WarnOnce = crate::config::WarnOnce::new();
static WARNED_IDENTITY_ONLY_DIM: crate::config::WarnOnce = crate::config::WarnOnce::new();

/// Warn once when the remote backend is selected but no credential resolves, so
/// the local fallback is visible rather than silent. Names the exact env looked
/// for so the fix is obvious. Returns whether the warning fired (for tests).
fn warn_once_remote_selected_without_credential() -> bool {
    if !WARNED_MISSING_CREDENTIAL.should_fire() {
        return false;
    }
    crate::logging::warn(&format!(
        "memory_embedding_backend = \"openai\" but no credential was found in {}; \
         falling back to the local embedding backend",
        configured_embedding_key_env()
    ));
    true
}

/// Warn once when the remote backend is selected with a credential but its
/// configuration is invalid (bad base URL, or custom model missing a dimension),
/// so the local fallback is visible.
fn warn_once_remote_config_invalid(err: &anyhow::Error) {
    if !WARNED_INVALID_CONFIG.should_fire() {
        return;
    }
    crate::logging::warn(&format!(
        "memory_embedding_backend = \"openai\" is misconfigured ({err}); \
         falling back to the local embedding backend"
    ));
}

/// Warn once that an explicit `memory_embedding_dim` on a model that does not
/// support the request `dimensions` field is an identity declaration, not a
/// server-side truncation request.
fn warn_once_identity_only_dimension(model: &str) {
    if !WARNED_IDENTITY_ONLY_DIM.should_fire() {
        return;
    }
    crate::logging::warn(&format!(
        "memory_embedding_dim is set for model {model}, which does not support server-side \
         dimension truncation; the value is used only as a vector-identity/sanity declaration, \
         not sent as a truncation request"
    ));
}

#[cfg(test)]
fn reset_warn_once_guards() {
    WARNED_MISSING_CREDENTIAL.reset();
    WARNED_INVALID_CONFIG.reset();
    WARNED_IDENTITY_ONLY_DIM.reset();
}

/// The model id (vector-space tag) of the currently active backend.
///
/// Persisted on freshly embedded memories and used to gate cross-model dense
/// comparisons. When remote embeddings are not active this is the local
/// MiniLM tag, matching legacy untagged memories.
pub fn active_model_id() -> String {
    active_backend().model_id().to_string()
}

/// Whether `entry_model` (an entry's `effective_embedding_model()`) shares a
/// vector space with the active backend, so their dense cosine is meaningful.
pub fn model_matches_active(entry_model: &str) -> bool {
    entry_model == active_model_id()
}

/// Embed a retrieval QUERY with the active backend, returning the vector and the
/// backend's model id. The local backend round-trips through the cached
/// `crate::embedding` facade; remote backends call their API directly.
pub fn embed_query_active(text: &str) -> anyhow::Result<(Vec<f32>, String)> {
    let backend = active_backend();
    let vec = backend.embed_query(text)?;
    Ok((vec, backend.model_id().to_string()))
}

/// Embed a stored PASSAGE/memory with the active backend, returning the vector
/// and the backend's model id (to persist on the entry for space-gating).
pub fn embed_passage_active(text: &str) -> anyhow::Result<(Vec<f32>, String)> {
    let backend = active_backend();
    let vec = backend.embed_passage(text)?;
    Ok((vec, backend.model_id().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construct a remote backend, panicking on the refuse-remote error paths so
    /// the happy-path tests read cleanly.
    fn backend(model: &str, base_url: Option<&str>, dim: Option<usize>) -> OpenAiEmbeddingBackend {
        OpenAiEmbeddingBackend::new(model, "sk-x", base_url.map(str::to_string), dim)
            .expect("backend should construct")
    }

    #[test]
    fn local_backend_model_id_matches_legacy_tag() {
        // Critical for backward compatibility: the local backend's model id must
        // equal the legacy tag so pre-tagging memories stay in the same space.
        assert_eq!(LocalOnnxBackend.model_id(), LEGACY_EMBEDDING_MODEL);
    }

    #[test]
    fn default_formatting_is_identity() {
        let b = LocalOnnxBackend;
        assert_eq!(b.format_query("hello"), "hello");
        assert_eq!(b.format_passage("world"), "world");
    }

    // --- dimension resolution ------------------------------------------------

    #[test]
    fn known_models_infer_native_dim() {
        assert_eq!(backend("text-embedding-3-small", None, None).dim(), 1536);
        assert_eq!(backend("text-embedding-3-large", None, None).dim(), 3072);
        assert_eq!(backend("text-embedding-ada-002", None, None).dim(), 1536);
    }

    #[test]
    fn explicit_dim_wins_over_native() {
        let b = backend("text-embedding-3-large", None, Some(256));
        assert_eq!(b.dim(), 256);
    }

    #[test]
    fn custom_model_with_explicit_dim_is_tagged_with_that_dim() {
        let b = backend("bge-m3", None, Some(1024));
        assert_eq!(b.dim(), 1024);
        assert!(
            b.model_id().ends_with("|bge-m3|dim=1024"),
            "unexpected model_id: {}",
            b.model_id()
        );
    }

    #[test]
    fn custom_model_without_dim_refuses_remote_construction() {
        let err = OpenAiEmbeddingBackend::new("bge-m3", "sk-x", None, None)
            .expect_err("custom model with no dim must refuse remote construction");
        let msg = err.to_string();
        assert!(
            msg.contains("memory_embedding_dim"),
            "unhelpful error: {msg}"
        );
        assert!(msg.contains("bge-m3"), "error should name the model: {msg}");
    }

    // --- request JSON --------------------------------------------------------

    #[test]
    fn request_includes_dimensions_only_for_capable_model_with_explicit_dim() {
        // v3-small + explicit 256 -> dimensions: 256
        let b = backend("text-embedding-3-small", None, Some(256));
        let body = b.request_body(&["hi".to_string()]);
        assert_eq!(body["dimensions"], serde_json::json!(256));
        assert_eq!(body["model"], serde_json::json!("text-embedding-3-small"));

        // v3-small, no override -> omitted
        let b = backend("text-embedding-3-small", None, None);
        let body = b.request_body(&["hi".to_string()]);
        assert!(body.get("dimensions").is_none());

        // ada-002 + explicit -> omitted (not dimensions-capable)
        let b = backend("text-embedding-ada-002", None, Some(256));
        let body = b.request_body(&["hi".to_string()]);
        assert!(body.get("dimensions").is_none());

        // custom + explicit -> omitted (unknown model)
        let b = backend("bge-m3", None, Some(1024));
        let body = b.request_body(&["hi".to_string()]);
        assert!(body.get("dimensions").is_none());
    }

    // --- model_id identity ---------------------------------------------------

    #[test]
    fn model_id_encodes_endpoint_model_and_dim_and_never_a_key() {
        let b = backend("text-embedding-3-small", None, None);
        assert_eq!(
            b.model_id(),
            "openai:https://api.openai.com/v1|text-embedding-3-small|dim=1536"
        );
        assert!(!b.model_id().contains("sk-x"));
        assert_ne!(b.model_id(), LocalOnnxBackend.model_id());
    }

    #[test]
    fn model_id_differs_across_endpoint_dim_and_backend() {
        let default_ep = backend("text-embedding-3-small", None, None);
        let other_ep = backend(
            "text-embedding-3-small",
            Some("https://gw.example.com/v1"),
            None,
        );
        assert_ne!(default_ep.model_id(), other_ep.model_id());

        let dim_256 = backend("text-embedding-3-large", None, Some(256));
        let dim_native = backend("text-embedding-3-large", None, None);
        assert_ne!(dim_256.model_id(), dim_native.model_id());

        // Two gateway paths on one host are distinct vector services.
        let path_a = backend("m", Some("https://h.example.com/a"), Some(8));
        let path_b = backend("m", Some("https://h.example.com/b"), Some(8));
        assert_ne!(path_a.model_id(), path_b.model_id());

        assert_ne!(default_ep.model_id(), LocalOnnxBackend.model_id());
    }

    // --- URL canonicalization ------------------------------------------------

    #[test]
    fn url_default_and_none_are_openai() {
        assert_eq!(
            normalize_embedding_base_url(None).unwrap(),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_embedding_base_url(Some("   ")).unwrap(),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn url_case_default_port_and_trailing_slash_normalize_equally() {
        let a = normalize_embedding_base_url(Some("HTTPS://API.OpenAI.com:443/v1/")).unwrap();
        let b = normalize_embedding_base_url(Some("https://api.openai.com/v1")).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, "https://api.openai.com/v1");
        // Root path collapses to "/".
        assert_eq!(
            normalize_embedding_base_url(Some("https://h.example.com///")).unwrap(),
            "https://h.example.com/"
        );
        // Multiple trailing slashes trimmed but interior preserved.
        assert_eq!(
            normalize_embedding_base_url(Some("https://h.example.com/v1///")).unwrap(),
            "https://h.example.com/v1"
        );
    }

    #[test]
    fn url_non_default_port_is_kept() {
        assert_eq!(
            normalize_embedding_base_url(Some("http://localhost:8080/v1")).unwrap(),
            "http://localhost:8080/v1"
        );
    }

    #[test]
    fn url_rejects_userinfo_query_fragment_scheme_and_missing_host() {
        assert!(normalize_embedding_base_url(Some("https://user:pw@h.example.com/v1")).is_err());
        assert!(normalize_embedding_base_url(Some("https://h.example.com/v1?a=b")).is_err());
        assert!(normalize_embedding_base_url(Some("https://h.example.com/v1#f")).is_err());
        assert!(normalize_embedding_base_url(Some("ftp://h.example.com/v1")).is_err());
        assert!(normalize_embedding_base_url(Some("not a url")).is_err());
    }

    #[test]
    fn invalid_url_refuses_remote_construction() {
        let err = OpenAiEmbeddingBackend::new(
            "text-embedding-3-small",
            "sk-x",
            Some("https://user:pw@h.example.com/v1".to_string()),
            None,
        )
        .expect_err("userinfo URL must refuse remote construction");
        assert!(err.to_string().contains("userinfo"));
    }

    // --- response validation -------------------------------------------------

    #[test]
    fn response_validator_accepts_exact_length_and_rejects_mismatch() {
        let ok = vec![vec![0.0f32; 4], vec![1.0f32; 4]];
        assert!(validate_response_dims(&ok, 4, "m").is_ok());

        // First vector wrong length.
        let bad_first = vec![vec![0.0f32; 3], vec![1.0f32; 4]];
        assert!(validate_response_dims(&bad_first, 4, "m").is_err());

        // A later heterogeneous batch vector is also caught.
        let bad_later = vec![vec![0.0f32; 4], vec![1.0f32; 5]];
        assert!(validate_response_dims(&bad_later, 4, "m").is_err());
    }

    // --- warn-once fallback --------------------------------------------------

    #[test]
    fn missing_credential_fallback_warns_exactly_once() {
        reset_warn_once_guards();
        assert!(
            warn_once_remote_selected_without_credential(),
            "first call should warn"
        );
        assert!(
            !warn_once_remote_selected_without_credential(),
            "second call must be suppressed"
        );
        reset_warn_once_guards();
    }
}
