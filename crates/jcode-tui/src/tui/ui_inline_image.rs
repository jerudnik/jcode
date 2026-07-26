//! Inline image transcript section.
//!
//! Images attached to the conversation (pasted screenshots, `read` of an image
//! file, generated images) render directly in the chat flow, sized to fit the
//! chat width with a capped height. This replaces the old "pinned image side
//! panel" surface.
//!
//! Design goals:
//! * **Lazy.** Prepare only needs each image's `(id, width, height)`, obtained
//!   from a cheap header parse (no full decode, no disk write, no retained
//!   bytes). The full decode + terminal transmit happens at draw time, and only
//!   for images currently on screen.
//! * **Single source of pixels.** The base64 payloads stay in their existing
//!   owner (`App::side_pane_images()`); this section keeps only ids and a small
//!   ingest-time payload registry so the draw step can materialize on demand.
//! * **Correct fit.** Images scale to fit width (preserving aspect) and cap at a
//!   fraction of the viewport so a tall screenshot never buries the transcript.

use crate::tui::mermaid;
use jcode_tui_messages::{ImageRegion, ImageRegionRender, PreparedMessages};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, OnceLock, mpsc};

/// One image to render inline, resolved from a `RenderedImage`.
#[derive(Clone)]
pub(crate) struct InlineImageItem {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

/// Cap an inline image at this fraction of the chat viewport height so a tall
/// image cannot push the rest of the transcript off-screen.
const MAX_VIEWPORT_FRACTION_PERCENT: u16 = 55;
/// Never shrink an inline image below this many rows.
const MIN_IMAGE_ROWS: u16 = 3;
/// Fixed row cap for images anchored inside the transcript body. The body is
/// prepared and cached independently of the viewport height, so anchored
/// placeholder geometry must not depend on it; a fixed cap keeps tall
/// screenshots from dominating the flow while staying resize-stable.
const ANCHORED_MAX_ROWS: u16 = 16;

/// Discrete per-image expand levels. Clicking the `expand` badge cycles an
/// image through these caps. The caps are *fixed row counts* (not a fraction of
/// the viewport) on purpose: anchored placeholder geometry feeds the body cache
/// which is keyed by width only, so the expand level must stay viewport
/// independent or it would break that invariant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ImageExpandLevel {
    /// Default fit-to-flow size (`ANCHORED_MAX_ROWS`).
    #[default]
    Fit,
    /// Roughly 2.5x taller, for a closer look without leaving the transcript.
    Large,
    /// Effectively uncapped height: tall enough that virtually every image is
    /// width-bound, i.e. rendered at the largest size the chat pane allows.
    Full,
}

impl ImageExpandLevel {
    /// Next level in the click cycle (Fit -> Large -> Full -> Fit).
    pub(crate) fn next(self) -> Self {
        match self {
            ImageExpandLevel::Fit => ImageExpandLevel::Large,
            ImageExpandLevel::Large => ImageExpandLevel::Full,
            ImageExpandLevel::Full => ImageExpandLevel::Fit,
        }
    }

    /// Anchored row cap for this level. Stays viewport independent so the
    /// width-keyed body cache remains valid across resizes. The `Full` cap is
    /// bounded by kitty's virtual-placement row limit (296 diacritic slots),
    /// with margin, so stable fit rendering keeps working at every level.
    fn anchored_cap_rows(self) -> u16 {
        match self {
            ImageExpandLevel::Fit => ANCHORED_MAX_ROWS,
            ImageExpandLevel::Large => 40,
            ImageExpandLevel::Full => 200,
        }
    }
}

/// Resolve the expand level for an image id. Implemented by `App` so the lookup
/// stays close to the persisted/live state, while this module owns the geometry.
pub(crate) trait ImageExpandLevels {
    fn expand_level(&self, id: u64) -> ImageExpandLevel;
}

/// A levels source that reports every image as `Fit`. Used by tests that
/// exercise section/line building without an `App` to resolve expand state.
#[cfg(test)]
pub(crate) struct AllFit;
#[cfg(test)]
impl ImageExpandLevels for AllFit {
    fn expand_level(&self, _id: u64) -> ImageExpandLevel {
        ImageExpandLevel::Fit
    }
}

/// Adapter so prepare code can resolve per-image expand levels straight from the
/// app state without copying the whole map into this module.
pub(crate) struct AppExpandLevels<'a>(pub &'a dyn crate::tui::TuiState);
impl ImageExpandLevels for AppExpandLevels<'_> {
    fn expand_level(&self, id: u64) -> ImageExpandLevel {
        self.0.image_expand_level(id)
    }
}

/// Ingest-time registry mapping image id -> (media_type, base64) so the draw
/// step can materialize bytes without threading payloads through the cached
/// prepared-frame model. Bounded; entries are cheap (two `String`s + id).
static PAYLOAD_REGISTRY: LazyLock<Mutex<PayloadRegistry>> =
    LazyLock::new(|| Mutex::new(PayloadRegistry::new()));

const PAYLOAD_REGISTRY_MAX: usize = 512;
/// Byte budget for the payload registry. Entries hold the *full base64
/// payload* (a 5 MB screenshot is ~6.7 MB of base64), so a pure entry-count
/// bound could still pin gigabytes of RAM across a screenshot-heavy session.
/// Evicted payloads are re-registered by the next prepare pass that resolves
/// the image, so the only cost of a tight budget is a string clone later.
const PAYLOAD_REGISTRY_MAX_BYTES: usize = 64 * 1024 * 1024;

struct PayloadRegistry {
    map: std::collections::HashMap<u64, (String, String)>,
    order: std::collections::VecDeque<u64>,
    total_bytes: usize,
}

impl PayloadRegistry {
    fn new() -> Self {
        Self {
            map: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            total_bytes: 0,
        }
    }

    /// Insert a payload. Returns true when the id was newly inserted (false
    /// when it was already registered).
    fn insert(&mut self, id: u64, media_type: &str, data_b64: &str) -> bool {
        if self.map.contains_key(&id) {
            return false;
        }
        self.total_bytes = self
            .total_bytes
            .saturating_add(media_type.len() + data_b64.len());
        self.map
            .insert(id, (media_type.to_string(), data_b64.to_string()));
        self.order.push_back(id);
        // Evict oldest-first past either bound, but never the entry just
        // inserted: a single over-budget payload must stay resident or its
        // image could never materialize.
        while (self.order.len() > PAYLOAD_REGISTRY_MAX
            || self.total_bytes > PAYLOAD_REGISTRY_MAX_BYTES)
            && self.order.len() > 1
        {
            if let Some(old) = self.order.pop_front()
                && let Some((media_type, data_b64)) = self.map.remove(&old)
            {
                self.total_bytes = self
                    .total_bytes
                    .saturating_sub(media_type.len() + data_b64.len());
            }
        }
        true
    }

    fn get(&self, id: u64) -> Option<(String, String)> {
        self.map.get(&id).cloned()
    }

    fn remove(&mut self, id: u64) {
        if let Some((media_type, data_b64)) = self.map.remove(&id) {
            self.total_bytes = self
                .total_bytes
                .saturating_sub(media_type.len() + data_b64.len());
            if let Some(pos) = self.order.iter().position(|entry| *entry == id) {
                self.order.remove(pos);
            }
        }
    }
}

/// Record an image payload so [`materialize_visible`] can decode it on demand.
///
/// Skipped entirely for images that are already materialized: their decoded
/// bytes live in the render cache (memory) and cache dir (disk), so staging
/// the base64 copy again would just hold multi-megabyte payloads resident
/// twice. [`materialize_visible`] rediscovers evicted entries from disk.
pub(crate) fn register_payload(id: u64, media_type: &str, data_b64: &str) {
    if mermaid::inline_image_is_materialized(id) {
        return;
    }
    let newly_inserted = match PAYLOAD_REGISTRY.lock() {
        Ok(mut reg) => reg.insert(id, media_type, data_b64),
        Err(_) => false,
    };
    // A fresh payload may succeed where a previously evicted/corrupt one
    // failed, so give the prewarm pipeline its retries back.
    if newly_inserted && let Ok(mut failures) = PREWARM_FAILURES.lock() {
        failures.remove(&id);
    }
}

/// Drop the staged base64 payload for an image whose decoded bytes are now
/// persisted in the render cache; see [`register_payload`].
fn release_payload(id: u64) {
    if let Ok(mut reg) = PAYLOAD_REGISTRY.lock() {
        reg.remove(id);
    }
}

/// Ensure the image with `id` is materialized (decoded + cached) so it can be
/// drawn. Returns true on success.
///
/// Steady-state frames hit a cheap in-memory presence probe (no payload clone,
/// no payload hash); only the first visible frame for an image pays the decode
/// + cache cost.
pub(crate) fn materialize_visible(id: u64) -> bool {
    if mermaid::inline_image_is_materialized(id) {
        return true;
    }
    if let Some((media_type, data_b64)) = PAYLOAD_REGISTRY.lock().ok().and_then(|reg| reg.get(id)) {
        let materialized = mermaid::materialize_inline_image_by_id(id, &media_type, &data_b64);
        if materialized.is_some() {
            // The decoded bytes now live in the render cache and cache dir;
            // holding the base64 copy too would double-count every image.
            release_payload(id);
            return true;
        }
        return false;
    }
    // No staged payload: either it was dropped after a successful
    // materialization and the render-cache entry has since been LRU-evicted
    // (restore it from the persisted cache file), or this is a mermaid
    // diagram whose PNG lives in the shared render cache/disk.
    if mermaid::rediscover_inline_image(id).is_some() {
        return true;
    }
    mermaid::get_cached_path(id).is_some()
}

/// One pending prewarm request: build everything needed to draw image `id`
/// at the given placeholder geometry (decode payload, write cache file, scale
/// to the target box, escape-encode for Kitty).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PrewarmRequest {
    id: u64,
    target_cols: u16,
    target_rows: u16,
}

static PREWARM_TX: OnceLock<mpsc::Sender<PrewarmRequest>> = OnceLock::new();
/// Requests queued or in flight, so a 60fps scroll doesn't enqueue the same
/// image dozens of times before the worker finishes it.
static PREWARM_INFLIGHT: LazyLock<Mutex<HashSet<PrewarmRequest>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Per-image count of failed materialize attempts. Without this memo an
/// undecodable payload (or one evicted from the registry) would loop forever:
/// draw schedules a prewarm, the worker fails and nudges a repaint, the
/// repaint reschedules the same prewarm. After
/// [`PREWARM_FAILURE_MAX_ATTEMPTS`] failures the id is skipped until
/// [`register_payload`] sees a fresh payload for it.
static PREWARM_FAILURES: LazyLock<Mutex<HashMap<u64, u8>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const PREWARM_FAILURE_MAX_ATTEMPTS: u8 = 3;
/// Bound the failure memo; it only holds pathological ids, so if it fills up
/// something is systemically wrong and starting over is harmless.
const PREWARM_FAILURES_MAX: usize = 512;

fn prewarm_failures_exhausted(id: u64) -> bool {
    PREWARM_FAILURES
        .lock()
        .ok()
        .and_then(|failures| failures.get(&id).copied())
        .is_some_and(|count| count >= PREWARM_FAILURE_MAX_ATTEMPTS)
}

fn record_prewarm_failure(id: u64) {
    if let Ok(mut failures) = PREWARM_FAILURES.lock() {
        if failures.len() >= PREWARM_FAILURES_MAX && !failures.contains_key(&id) {
            failures.clear();
        }
        let count = failures.entry(id).or_insert(0);
        *count = count.saturating_add(1);
        if *count == PREWARM_FAILURE_MAX_ATTEMPTS {
            crate::logging::warn(&format!(
                "inline image {id:#018x} failed to materialize {PREWARM_FAILURE_MAX_ATTEMPTS} times; \
                 suspending prewarm until its payload is re-registered"
            ));
        }
    }
}

fn prewarm_sender() -> &'static mpsc::Sender<PrewarmRequest> {
    PREWARM_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<PrewarmRequest>();
        if let Err(err) = std::thread::Builder::new()
            .name("jcode-inline-image-prewarm".to_string())
            .spawn(move || prewarm_worker(rx))
        {
            crate::logging::warn(&format!(
                "Failed to spawn inline-image prewarm worker; first view will decode on the UI thread: {}",
                err
            ));
        }
        tx
    })
}

fn prewarm_worker(rx: mpsc::Receiver<PrewarmRequest>) {
    for req in rx {
        let materialized = materialize_visible(req.id);
        if materialized {
            mermaid::prewarm_inline_fit_state(req.id, req.target_cols, req.target_rows, true);
            if let Ok(mut failures) = PREWARM_FAILURES.lock() {
                failures.remove(&req.id);
            }
        } else {
            record_prewarm_failure(req.id);
        }
        if let Ok(mut inflight) = PREWARM_INFLIGHT.lock() {
            inflight.remove(&req);
        }
        if !materialized {
            // Nothing new to draw; nudging a repaint would only make the
            // draw path reschedule this same failed request.
            continue;
        }
        // Nudge the UI exactly like a finished deferred Mermaid render so the
        // placeholder fills in on the next frame without user input. The
        // prepared placeholder geometry is unchanged, so no prepare-cache
        // invalidation is needed - just a repaint.
        crate::bus::Bus::global().publish(crate::bus::BusEvent::MermaidRenderCompleted);
    }
}

/// Make sure image `id` can be drawn cheaply this frame.
///
/// Returns true when the draw path can run now without heavy work (image
/// decoded and, on Kitty, scale+transmit state matches the placeholder
/// geometry). Returns false after scheduling background preparation; the
/// caller should skip drawing this frame and rely on the completion nudge to
/// repaint.
pub(crate) fn ensure_drawable(id: u64, target_cols: u16, target_rows: u16) -> bool {
    let materialized = mermaid::inline_image_is_materialized(id);
    let readiness = if materialized {
        mermaid::inline_fit_readiness(id, target_cols, target_rows, true)
    } else {
        // Not decoded yet. On any protocol the first draw would block on a
        // full decode, so prewarm regardless of protocol support.
        mermaid::InlineFitReadiness::NeedsPrewarm
    };

    match draw_action(materialized, readiness) {
        DrawAction::DrawNow => true,
        DrawAction::Prewarm => {
            schedule_prewarm(id, target_cols, target_rows);
            false
        }
    }
}

/// What [`ensure_drawable`] should do for a given image state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrawAction {
    /// The draw path can run cheaply this frame.
    DrawNow,
    /// Schedule background preparation and skip drawing this frame.
    Prewarm,
}

/// The decision half of [`ensure_drawable`], split out from the global image
/// caches and the process-wide terminal picker.
///
/// The picker is a process-global `OnceLock` that any other test can
/// initialize (and never un-initialize), so a test that reached this logic
/// through `ensure_drawable` silently asserted a different branch depending on
/// which tests ran before it in the same process.
pub(crate) fn draw_action(
    materialized: bool,
    readiness: mermaid::InlineFitReadiness,
) -> DrawAction {
    match readiness {
        mermaid::InlineFitReadiness::Ready => DrawAction::DrawNow,
        // Non-Kitty fallback renderers manage their own protocol state; the
        // only requirement is that the bytes are already decoded.
        mermaid::InlineFitReadiness::Unsupported if materialized => DrawAction::DrawNow,
        mermaid::InlineFitReadiness::Unsupported | mermaid::InlineFitReadiness::NeedsPrewarm => {
            DrawAction::Prewarm
        }
    }
}

fn schedule_prewarm(id: u64, target_cols: u16, target_rows: u16) {
    if prewarm_failures_exhausted(id) {
        return;
    }
    let req = PrewarmRequest {
        id,
        target_cols,
        target_rows,
    };
    if let Ok(mut inflight) = PREWARM_INFLIGHT.lock()
        && !inflight.insert(req)
    {
        return;
    }
    if prewarm_sender().send(req).is_err() {
        // Worker unavailable: fall back to synchronous work on next frame.
        if let Ok(mut inflight) = PREWARM_INFLIGHT.lock() {
            inflight.remove(&req);
        }
        materialize_visible(id);
    }
}

/// Warm an inline image that is *not* on screen yet so it is ready to draw the
/// instant it scrolls into view. Unlike [`ensure_drawable`], this never blocks
/// and never draws: if the image still needs decode/scale/transmit work it is
/// scheduled on the background prewarm worker (deduped against in-flight and
/// already-warm state), otherwise it is a cheap no-op.
///
/// Callers pass the same `(target_cols, target_rows)` placeholder geometry the
/// draw path will use, so the prewarmed Kitty fit-state matches exactly and the
/// first on-screen frame hits the `Ready` fast path with no rescale.
pub(crate) fn prefetch(id: u64, target_cols: u16, target_rows: u16) {
    let readiness = if mermaid::inline_image_is_materialized(id) {
        mermaid::inline_fit_readiness(id, target_cols, target_rows, true)
    } else {
        mermaid::InlineFitReadiness::NeedsPrewarm
    };
    match readiness {
        // Already drawable, or a protocol that builds its state synchronously
        // at draw time (nothing useful to prewarm ahead).
        mermaid::InlineFitReadiness::Ready | mermaid::InlineFitReadiness::Unsupported => {}
        mermaid::InlineFitReadiness::NeedsPrewarm => {
            schedule_prewarm(id, target_cols, target_rows);
        }
    }
}

fn resolve_item(image: &crate::session::RenderedImage) -> Option<InlineImageItem> {
    let (id, width, height) = mermaid::inline_image_dims(&image.media_type, &image.data)?;
    register_payload(id, &image.media_type, &image.data);
    let label = image
        .label
        .clone()
        .unwrap_or_else(|| image.media_type.clone());
    Some(InlineImageItem {
        id,
        width,
        height,
        label,
    })
}

/// Inline images split by their transcript anchor so the body renderer can
/// place each one at the message that produced it.
#[derive(Default)]
pub(crate) struct AnchoredInlineImages {
    /// Images anchored to a tool result, keyed by tool call id.
    pub by_tool: HashMap<String, Vec<InlineImageItem>>,
    /// Images anchored to the nth (0-based) rendered user prompt.
    pub by_prompt: HashMap<usize, Vec<InlineImageItem>>,
    /// Images with no usable anchor; rendered at the end of the transcript.
    pub unanchored: Vec<InlineImageItem>,
}

impl AnchoredInlineImages {
    #[cfg(test)]
    pub(crate) fn has_anchored(&self) -> bool {
        !self.by_tool.is_empty() || !self.by_prompt.is_empty()
    }

    /// Items that will NOT be placed inside the transcript body: unanchored
    /// images plus anchored images whose anchor target does not exist among
    /// `messages` (e.g. live images for a tool whose transcript entry was
    /// replaced). These fall back to the bottom inline-images section so no
    /// image ever silently disappears.
    pub(crate) fn unplaced_items(
        &self,
        messages: &[jcode_tui_messages::DisplayMessage],
    ) -> Vec<InlineImageItem> {
        let mut items: Vec<InlineImageItem> = self.unanchored.clone();
        if self.by_tool.is_empty() && self.by_prompt.is_empty() {
            return items;
        }

        let mut tool_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut prompt_count = 0usize;
        for msg in messages {
            use crate::tui::DisplayMessageRoleExt as _;
            match msg.effective_role() {
                "tool" => {
                    if let Some(tool) = &msg.tool_data {
                        tool_ids.insert(tool.id.as_str());
                    }
                }
                "user" => {
                    if !crate::session::is_attached_image_label_text(&msg.content) {
                        prompt_count += 1;
                    }
                }
                _ => {}
            }
        }

        for (id, bucket) in &self.by_tool {
            if !tool_ids.contains(id.as_str()) {
                items.extend(bucket.iter().cloned());
            }
        }
        for (ordinal, bucket) in &self.by_prompt {
            if *ordinal >= prompt_count {
                items.extend(bucket.iter().cloned());
            }
        }
        items
    }
}

/// Resolve rendered images into anchored buckets (tool call / user prompt /
/// unanchored). Same lazy header-only cost profile as [`resolve_item`].
pub(crate) fn resolve_anchored_items(
    images: &[crate::session::RenderedImage],
) -> AnchoredInlineImages {
    let mut result = AnchoredInlineImages::default();
    for image in images {
        let Some(item) = resolve_item(image) else {
            continue;
        };
        match &image.anchor {
            Some(crate::session::RenderedImageAnchor::ToolCall { id }) => {
                result.by_tool.entry(id.clone()).or_default().push(item);
            }
            Some(crate::session::RenderedImageAnchor::UserPrompt { ordinal }) => {
                result.by_prompt.entry(*ordinal).or_default().push(item);
            }
            None => result.unanchored.push(item),
        }
    }
    result
}

/// One-slot cache for [`resolve_anchored_items`], keyed by the image-set
/// signature. Resolving hashes every image payload (for ids), so body
/// preparation must not redo it per rebuild; the signature is already cached
/// per transcript version on the app side.
type AnchoredCache = Mutex<Option<((usize, u64), std::sync::Arc<AnchoredInlineImages>)>>;
static ANCHORED_CACHE: LazyLock<AnchoredCache> = LazyLock::new(|| Mutex::new(None));

/// Resolve the app's images into anchored buckets, cached by the image-set
/// signature. Returns an empty result without touching payloads when the app
/// has no images or inline images are hidden.
pub(crate) fn resolve_anchored_items_cached(
    app: &dyn crate::tui::TuiState,
) -> std::sync::Arc<AnchoredInlineImages> {
    if !app.pin_images() {
        return std::sync::Arc::new(AnchoredInlineImages::default());
    }
    let signature = app.side_pane_images_signature();
    if signature.0 == 0 {
        return std::sync::Arc::new(AnchoredInlineImages::default());
    }
    if let Ok(cache) = ANCHORED_CACHE.lock()
        && let Some((cached_sig, cached)) = cache.as_ref()
        && *cached_sig == signature
    {
        return cached.clone();
    }
    let resolved = std::sync::Arc::new(resolve_anchored_items(&app.side_pane_images()));
    if let Ok(mut cache) = ANCHORED_CACHE.lock() {
        *cache = Some((signature, resolved.clone()));
    }
    resolved
}

/// Compute how many `(rows, cols)` an inline image occupies at `chat_width`,
/// capped at `cap_rows`. `cols` includes the 2-cell left border, matching what
/// the draw step actually paints, so layout (e.g. info widget placement) can
/// know the real horizontal extent.
fn fit_geometry_with_cap(width: u32, height: u32, chat_width: u16, cap_rows: u16) -> (u16, u16) {
    // Single source of truth for inline-fit placeholder geometry, shared with
    // the mermaid crate so diagrams and raster images stay in lockstep with
    // the draw-time fit math.
    mermaid::inline_fit_geometry(width, height, chat_width, cap_rows)
}

/// Compute `(rows, cols)` for an inline image at `chat_width`, given a viewport
/// height to cap against.
fn fit_geometry(width: u32, height: u32, chat_width: u16, viewport_height: u16) -> (u16, u16) {
    let cap_rows = ((viewport_height as u32 * MAX_VIEWPORT_FRACTION_PERCENT as u32) / 100)
        .clamp(MIN_IMAGE_ROWS as u32, u16::MAX as u32) as u16;
    fit_geometry_with_cap(width, height, chat_width, cap_rows)
}

/// Compute `(rows, cols)` for an image anchored inside the transcript body at a
/// given expand level. Uses a fixed (viewport-independent) row cap so the body's
/// prepared lines stay valid across resizes (the body cache is keyed by width
/// only); the expand level only swaps which fixed cap applies.
pub(crate) fn fit_geometry_anchored(
    width: u32,
    height: u32,
    chat_width: u16,
    level: ImageExpandLevel,
) -> (u16, u16) {
    fit_geometry_with_cap(width, height, chat_width, level.anchored_cap_rows())
}

/// Compute how many rows an inline image should occupy at `chat_width`, given a
/// viewport height to cap against.
#[cfg(test)]
fn fit_rows(width: u32, height: u32, chat_width: u16, viewport_height: u16) -> u16 {
    fit_geometry(width, height, chat_width, viewport_height).0
}

/// Build the dim label line shown above an inline image, e.g.
/// `  screenshot.png  1920×1080`, with a trailing show/hide badge
/// (`[Alt] [⇧] [I] hide` / `[Alt] [⇧] [I] show image`) so the toggle is
/// discoverable right where the image renders. The line is kept deliberately
/// short so it fits on one row; there is no visible expand badge, but clicking
/// the rendered image body still cycles the per-image size
/// (see `inline_image_body_target_from_screen`).
pub(crate) fn image_label_line(
    item: &InlineImageItem,
    width: u16,
    images_visible: bool,
    _level: ImageExpandLevel,
) -> Line<'static> {
    let dims = format!("{}×{}", item.width, item.height);
    let label = if item.label.is_empty() {
        dims
    } else {
        format!("{}  {}", item.label, dims)
    };
    let dim = Style::default().add_modifier(Modifier::DIM);
    let prefix = Line::from(vec![Span::styled("  ", dim), Span::styled(label, dim)]);
    let mut suffix = vec![
        Span::raw("  "),
        Span::styled(super::viewport::copy_badge_alt_badge(), dim),
        Span::styled(" [⇧] [I] ", dim),
    ];
    if images_visible {
        suffix.push(Span::styled("hide", dim));
    } else {
        suffix.push(Span::styled(
            "show image",
            Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC),
        ));
    }
    jcode_tui_render::truncate_line_preserving_suffix_to_width(
        &prefix,
        &Line::from(suffix),
        width as usize,
    )
}

/// Lines for images anchored at a transcript message: per image, a leading
/// blank, a dim label, a geometry-encoding marker line plus blank placeholder
/// rows (recognized by the image-region scan), and a trailing blank. When
/// `images_visible` is false the image collapses to just its label stub (with
/// a `show image` badge) and no placeholder rows are emitted, so nothing is
/// painted.
pub(crate) fn anchored_image_lines(
    items: &[InlineImageItem],
    width: u16,
    images_visible: bool,
    levels: &dyn ImageExpandLevels,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for item in items {
        let level = levels.expand_level(item.id);
        lines.push(Line::from(""));
        lines.push(image_label_line(item, width, images_visible, level));
        if images_visible {
            let (rows, cols) = fit_geometry_anchored(item.width, item.height, width, level);
            lines.extend(mermaid::inline_image_placeholder_lines(item.id, rows, cols));
        }
        lines.push(Line::from(""));
    }
    lines
}

/// Build the inline-images prepared section: a heading + correctly-sized
/// placeholder per image, with explicit `image_regions` (render = Fit) that the
/// viewport draws lazily. When `images_visible` is false each image collapses
/// to its label stub and no regions are emitted.
pub(crate) fn build_section(
    items: &[InlineImageItem],
    width: u16,
    viewport_height: u16,
    prefix_blank: bool,
    images_visible: bool,
    levels: &dyn ImageExpandLevels,
) -> PreparedMessages {
    use std::sync::Arc;

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut image_regions: Vec<ImageRegion> = Vec::new();

    if items.is_empty() {
        return empty();
    }

    if prefix_blank {
        lines.push(Line::from(""));
    }

    for item in items {
        let level = levels.expand_level(item.id);
        lines.push(image_label_line(item, width, images_visible, level));

        if images_visible {
            // The bottom (unanchored) section is rebuilt every frame, not body
            // cached, so a viewport-relative default fit is fine here. Expanded
            // levels use the discrete fixed caps so they grow predictably.
            let (rows, cols) = match level {
                ImageExpandLevel::Fit => {
                    fit_geometry(item.width, item.height, width, viewport_height)
                }
                _ => {
                    fit_geometry_with_cap(item.width, item.height, width, level.anchored_cap_rows())
                }
            };
            let region_start = lines.len();
            for _ in 0..rows {
                lines.push(Line::from(""));
            }
            image_regions.push(ImageRegion {
                abs_line_idx: region_start,
                end_line: region_start + rows as usize,
                hash: item.id,
                height: rows,
                width: cols,
                render: ImageRegionRender::Fit,
            });
        }
        // Trailing spacer between images.
        lines.push(Line::from(""));
    }

    let line_count = lines.len();
    let plain: Vec<String> = lines
        .iter()
        .map(jcode_tui_render::line_plain_text)
        .collect();

    PreparedMessages {
        wrapped_lines: lines,
        wrapped_plain_lines: Arc::new(plain),
        wrapped_copy_offsets: Arc::new(vec![0; line_count]),
        raw_plain_lines: Arc::new(Vec::new()),
        wrapped_line_map: Arc::new(Vec::new()),
        wrapped_user_indices: Vec::new(),
        wrapped_user_prompt_starts: Vec::new(),
        wrapped_user_prompt_ends: Vec::new(),
        user_prompt_texts: Vec::new(),
        image_regions,
        edit_tool_ranges: Vec::new(),
        copy_targets: Vec::new(),
        message_boundaries: Vec::new(),
        mermaid_pending_epoch: None,
    }
}

fn empty() -> PreparedMessages {
    use std::sync::Arc;
    PreparedMessages {
        wrapped_lines: Vec::new(),
        wrapped_plain_lines: Arc::new(Vec::new()),
        wrapped_copy_offsets: Arc::new(Vec::new()),
        raw_plain_lines: Arc::new(Vec::new()),
        wrapped_line_map: Arc::new(Vec::new()),
        wrapped_user_indices: Vec::new(),
        wrapped_user_prompt_starts: Vec::new(),
        wrapped_user_prompt_ends: Vec::new(),
        user_prompt_texts: Vec::new(),
        image_regions: Vec::new(),
        edit_tool_ranges: Vec::new(),
        copy_targets: Vec::new(),
        message_boundaries: Vec::new(),
        mermaid_pending_epoch: None,
    }
}

#[cfg(test)]
#[path = "ui_inline_image_tests.rs"]
mod tests;
