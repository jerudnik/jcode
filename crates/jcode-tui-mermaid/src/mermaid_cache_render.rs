use super::*;

/// Metadata capacity, sized to avoid cache re-materialization while scrolling.
pub(super) const RENDER_CACHE_MAX: usize = 512;
/// Minimum cached width as a percentage of requested width, limiting blurry upscaling.
pub(super) const CACHE_WIDTH_MATCH_PERCENT: u32 = 85;
/// Width quantization prevents tiny pane changes from creating distinct renders.
pub(super) const RENDER_WIDTH_BUCKET_CELLS: u32 = 4;
/// Layout capacity; 32 representative 100-node/99-edge entries occupy roughly 2.4 MB.
/// Layouts are width-independent, so retaining them makes resize misses raster-only.
pub(super) const LAYOUT_CACHE_MAX: usize = 32;

pub(super) struct MermaidCache {
    pub(super) entries: HashMap<(u64, RenderProfile), CachedDiagram>,
    pub(super) order: VecDeque<(u64, RenderProfile)>,
    pub(super) cache_dir: PathBuf,
}

#[derive(Clone)]
pub(super) struct CachedDiagram {
    pub(super) path: PathBuf,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl MermaidCache {
    pub(super) fn new() -> Self {
        // `app_cache_dir` honors the test harness's storage redirect.
        let cache_dir = jcode_storage::app_cache_dir()
            .unwrap_or_else(|_| std::env::temp_dir().join("jcode"))
            .join("mermaid");

        let _ = fs::create_dir_all(&cache_dir);

        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            cache_dir,
        }
    }

    fn touch(&mut self, key: (u64, RenderProfile)) {
        if let Some(pos) = self.order.iter().position(|entry| *entry == key) {
            self.order.remove(pos);
        }
        self.order.push_back(key);
    }

    pub(super) fn get(
        &mut self,
        hash: u64,
        min_width: Option<u32>,
        profile: Option<RenderProfile>,
    ) -> Option<CachedDiagram> {
        if let Some(profile) = profile {
            return self.get_exact_profile(hash, min_width, profile);
        }

        if let Some((key, existing)) = self.order.iter().rev().find_map(|key| {
            let (entry_hash, _) = *key;
            let existing = self.entries.get(key)?;
            if entry_hash == hash && cached_width_satisfies(existing.width, min_width) {
                Some((*key, existing.clone()))
            } else {
                None
            }
        }) {
            if existing.path.exists() {
                super::record_cache_stat_syscall();
                self.touch(key);
                return Some(existing);
            }
            super::record_cache_stat_syscall();
            self.entries.remove(&key);
            if let Some(pos) = self.order.iter().position(|entry| *entry == key) {
                self.order.remove(pos);
            }
        }

        if let Some(found) = self.discover_on_disk(hash, min_width, None) {
            self.insert(hash, RenderProfile::default(), found.clone());
            return Some(found);
        }

        None
    }

    fn get_exact_profile(
        &mut self,
        hash: u64,
        min_width: Option<u32>,
        profile: RenderProfile,
    ) -> Option<CachedDiagram> {
        let key = (hash, profile);
        if let Some(existing) = self.entries.get(&key).cloned() {
            super::record_cache_stat_syscall();
            if existing.path.exists() && cached_width_satisfies(existing.width, min_width) {
                self.touch(key);
                return Some(existing);
            }
            self.entries.remove(&key);
            if let Some(pos) = self.order.iter().position(|entry| *entry == key) {
                self.order.remove(pos);
            }
        }

        if let Some(found) = self.discover_on_disk(hash, min_width, Some(profile)) {
            self.insert(hash, profile, found.clone());
            return Some(found);
        }

        None
    }

    /// LRU-promoting lookup that intentionally skips filesystem validation.
    fn get_in_memory(&mut self, hash: u64, profile: RenderProfile) -> Option<CachedDiagram> {
        let key = (hash, profile);
        let existing = self.entries.get(&key).cloned()?;
        self.touch(key);
        Some(existing)
    }

    /// Finds the most recently used profile without filesystem access.
    /// The draw thread may run outside the aspect scope used to render the diagram.
    fn get_in_memory_any_profile(&mut self, hash: u64) -> Option<CachedDiagram> {
        let key = self
            .order
            .iter()
            .rev()
            .find(|(entry_hash, _)| *entry_hash == hash)
            .copied()?;
        let existing = self.entries.get(&key).cloned()?;
        self.touch(key);
        Some(existing)
    }

    pub(super) fn insert(&mut self, hash: u64, profile: RenderProfile, diagram: CachedDiagram) {
        let key = (hash, profile);
        if let std::collections::hash_map::Entry::Occupied(mut entry) = self.entries.entry(key) {
            entry.insert(diagram);
            self.touch(key);
        } else {
            self.entries.insert(key, diagram);
            self.order.push_back(key);
            while self.order.len() > RENDER_CACHE_MAX {
                if let Some(old) = self.order.pop_front() {
                    self.entries.remove(&old);
                }
            }
        }
    }

    #[cfg(feature = "renderer")]
    pub(super) fn cache_path(
        &self,
        hash: u64,
        target_width: u32,
        profile: RenderProfile,
    ) -> PathBuf {
        let suffix = profile.cache_suffix().unwrap_or_default();
        self.cache_dir
            .join(format!("{:016x}_w{}{}.png", hash, target_width, suffix))
    }

    pub(super) fn discover_on_disk(
        &self,
        hash: u64,
        min_width: Option<u32>,
        profile: Option<RenderProfile>,
    ) -> Option<CachedDiagram> {
        let mut candidates: Vec<(PathBuf, u32, RenderProfile)> = Vec::new();
        super::record_cache_stat_syscall();
        let entries = fs::read_dir(&self.cache_dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("png") {
                continue;
            }
            let Some((file_hash, width_hint, file_profile)) = parse_cache_filename(&path) else {
                continue;
            };
            if file_hash == hash && profile.is_none_or(|profile| profile == file_profile) {
                candidates.push((path, width_hint, file_profile));
            }
        }
        if candidates.is_empty() {
            return None;
        }

        let selected = if let Some(min_w) = min_width {
            if let Some(candidate) = candidates
                .iter()
                .filter(|(_, w, _)| cached_width_satisfies(*w, Some(min_w)))
                .min_by_key(|(_, w, _)| *w)
            {
                candidate.clone()
            } else {
                return None;
            }
        } else {
            candidates
                .iter()
                .max_by_key(|(_, w, _)| *w)
                .cloned()
                .unwrap_or_else(|| candidates[0].clone())
        };

        let (path, width_hint, _) = selected;
        let (width, height) = get_png_dimensions(&path).unwrap_or((width_hint, width_hint));
        Some(CachedDiagram {
            path,
            width,
            height,
        })
    }
}

pub(super) fn cached_width_satisfies(width: u32, min_width: Option<u32>) -> bool {
    let Some(min_width) = min_width else {
        return true;
    };
    if min_width == 0 {
        return true;
    }
    width.saturating_mul(100) >= min_width.saturating_mul(CACHE_WIDTH_MATCH_PERCENT)
}

pub(super) fn parse_cache_filename(path: &Path) -> Option<(u64, u32, RenderProfile)> {
    let stem = path.file_stem()?.to_str()?;
    let (hash_hex, width_part) = stem.split_once("_w")?;
    let hash = u64::from_str_radix(hash_hex, 16).ok()?;
    let (width_text, profile) = if let Some((width, aspect)) = width_part.split_once("_a") {
        let aspect = aspect.parse::<u16>().ok()?;
        (
            width,
            RenderProfile {
                preferred_aspect_per_mille: Some(aspect),
            },
        )
    } else {
        (width_part, RenderProfile::default())
    };
    let width = width_text.parse::<u32>().ok()?;
    Some((hash, width, profile))
}

/// Everything that affects layout geometry; terminal width affects only rasterization.
#[cfg(feature = "renderer")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct LayoutCacheKey {
    pub(super) source_hash: u64,
    pub(super) theme_fingerprint: u64,
    pub(super) profile: RenderProfile,
    pub(super) layout_config_fingerprint: u64,
}

/// Computed layouts reusable across output dimensions.
#[cfg(feature = "renderer")]
pub(super) struct LayoutCache {
    pub(super) entries: HashMap<LayoutCacheKey, Arc<Layout>>,
    pub(super) order: VecDeque<LayoutCacheKey>,
    /// Theme shared by resident entries; changing it invalidates the whole cache.
    pub(super) theme_fingerprint: Option<u64>,
}

#[cfg(feature = "renderer")]
impl LayoutCache {
    pub(super) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            theme_fingerprint: None,
        }
    }

    fn enforce_theme(&mut self, theme_fingerprint: u64) {
        if self.theme_fingerprint != Some(theme_fingerprint) {
            self.entries.clear();
            self.order.clear();
            self.theme_fingerprint = Some(theme_fingerprint);
        }
    }

    fn touch(&mut self, key: LayoutCacheKey) {
        if let Some(pos) = self.order.iter().position(|entry| *entry == key) {
            self.order.remove(pos);
        }
        self.order.push_back(key);
    }

    pub(super) fn get(&mut self, key: &LayoutCacheKey) -> Option<Arc<Layout>> {
        self.enforce_theme(key.theme_fingerprint);
        let layout = self.entries.get(key).cloned()?;
        self.touch(*key);
        Some(layout)
    }

    pub(super) fn insert(&mut self, key: LayoutCacheKey, layout: Arc<Layout>) {
        self.enforce_theme(key.theme_fingerprint);
        if let std::collections::hash_map::Entry::Occupied(mut entry) = self.entries.entry(key) {
            entry.insert(layout);
            self.touch(key);
        } else {
            self.entries.insert(key, layout);
            self.order.push_back(key);
            while self.order.len() > LAYOUT_CACHE_MAX {
                if let Some(old) = self.order.pop_front() {
                    self.entries.remove(&old);
                }
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.theme_fingerprint = None;
    }
}

#[cfg(feature = "renderer")]
pub(super) static LAYOUT_CACHE: LazyLock<Mutex<LayoutCache>> =
    LazyLock::new(|| Mutex::new(LayoutCache::new()));

/// Hashes an in-memory-only cache key; cross-process stability is unnecessary.
#[cfg(feature = "renderer")]
fn serialize_fingerprint<T: serde::Serialize>(value: &T) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match serde_json::to_string(value) {
        Ok(text) => text.hash(&mut hasher),
        Err(_) => std::any::type_name::<T>().hash(&mut hasher),
    }
    hasher.finish()
}

/// Builds the shared layout configuration used by rendering and cache keys.
#[cfg(feature = "renderer")]
pub(super) fn build_layout_config(
    complexity: usize,
    render_profile: RenderProfile,
) -> LayoutConfig {
    let spacing_factor = if complexity > 30 { 1.2 } else { 1.0 };
    LayoutConfig {
        node_spacing: 80.0 * spacing_factor,
        rank_spacing: 80.0 * spacing_factor,
        node_padding_x: 40.0,
        node_padding_y: 20.0,
        preferred_aspect_ratio: render_profile.preferred_aspect_ratio(),
        ..Default::default()
    }
}

#[cfg(feature = "renderer")]
pub(super) fn layout_cache_key(
    source_hash: u64,
    theme: &Theme,
    layout_config: &LayoutConfig,
    profile: RenderProfile,
) -> LayoutCacheKey {
    LayoutCacheKey {
        source_hash,
        theme_fingerprint: serialize_fingerprint(theme),
        profile,
        layout_config_fingerprint: serialize_fingerprint(layout_config),
    }
}

#[cfg(feature = "renderer")]
fn layout_cache_get(key: &LayoutCacheKey) -> Option<Arc<Layout>> {
    let cached = LAYOUT_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(key);
    if let Ok(mut state) = MERMAID_DEBUG.lock() {
        if cached.is_some() {
            state.stats.layout_cache_hits += 1;
        } else {
            state.stats.layout_cache_misses += 1;
        }
    }
    cached
}

#[cfg(feature = "renderer")]
fn layout_cache_insert(key: LayoutCacheKey, layout: Arc<Layout>) {
    LAYOUT_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(key, layout);
}

pub(super) fn clear_layout_cache() {
    #[cfg(feature = "renderer")]
    LAYOUT_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

/// Returns resident layout count and approximate bytes.
pub(super) fn layout_cache_usage() -> (usize, u64) {
    #[cfg(feature = "renderer")]
    {
        let cache = LAYOUT_CACHE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let bytes = cache
            .entries
            .values()
            .map(|layout| approx_layout_bytes(layout))
            .sum();
        (cache.entries.len(), bytes)
    }
    #[cfg(not(feature = "renderer"))]
    {
        (0, 0)
    }
}

#[cfg(feature = "renderer")]
fn text_block_bytes(block: &mermaid_rs_renderer::layout::TextBlock) -> u64 {
    std::mem::size_of::<mermaid_rs_renderer::layout::TextBlock>() as u64
        + block
            .lines
            .iter()
            .map(|line| line.len() as u64 + std::mem::size_of::<String>() as u64)
            .sum::<u64>()
}

/// Estimates owned layout data. Diagram-specific payloads are counted only at
/// enum size, making this accurate for flowcharts and a lower bound otherwise.
#[cfg(feature = "renderer")]
pub(super) fn approx_layout_bytes(layout: &Layout) -> u64 {
    use mermaid_rs_renderer::layout::{EdgeLayout, NodeLayout, SubgraphLayout};
    let mut bytes = std::mem::size_of::<Layout>() as u64;
    for (id, node) in &layout.nodes {
        bytes += std::mem::size_of::<NodeLayout>() as u64
            + id.len() as u64
            + node.id.len() as u64
            + text_block_bytes(&node.label);
    }
    for edge in &layout.edges {
        bytes += std::mem::size_of::<EdgeLayout>() as u64
            + edge.from.len() as u64
            + edge.to.len() as u64
            + (edge.points.len() as u64) * (std::mem::size_of::<(f32, f32)>() as u64);
        for label in [&edge.label, &edge.start_label, &edge.end_label]
            .into_iter()
            .flatten()
        {
            bytes += text_block_bytes(label);
        }
    }
    for subgraph in &layout.subgraphs {
        bytes += std::mem::size_of::<SubgraphLayout>() as u64
            + subgraph.label.len() as u64
            + text_block_bytes(&subgraph.label_block)
            + subgraph
                .nodes
                .iter()
                .map(|node| node.len() as u64 + std::mem::size_of::<String>() as u64)
                .sum::<u64>();
    }
    bytes
}

/// Per-content counter lets parallel tests avoid races on global hit/miss stats.
#[cfg(test)]
pub(super) static LAYOUT_COMPUTATIONS: LazyLock<Mutex<HashMap<u64, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(all(test, feature = "renderer"))]
fn record_layout_computation_for_test(hash: u64) {
    if let Ok(mut counts) = LAYOUT_COMPUTATIONS.lock() {
        *counts.entry(hash).or_insert(0) += 1;
    }
}

#[cfg(test)]
pub(super) fn layout_computations_for_test(hash: u64) -> u64 {
    LAYOUT_COMPUTATIONS
        .lock()
        .map(|counts| counts.get(&hash).copied().unwrap_or(0))
        .unwrap_or(0)
}

/// Evicts every PNG for `content`, forcing rasterization at the next requested width.
pub fn evict_render_cache_for_content(content: &str) {
    evict_render_cache_by_hash(hash_content(content));
}

/// Evicts PNGs while leaving the layout tier warm.
#[cfg(test)]
pub(super) fn evict_render_cache_for_test(hash: u64) {
    evict_render_cache_by_hash(hash);
}

/// Inserts a test entry under the ambient render profile.
#[cfg(test)]
pub(super) fn insert_render_cache_entry_for_test(
    hash: u64,
    path: PathBuf,
    width: u32,
    height: u32,
) {
    if let Ok(mut cache) = RENDER_CACHE.lock() {
        cache.insert(
            hash,
            current_render_profile(),
            CachedDiagram {
                path,
                width,
                height,
            },
        );
    }
}

#[cfg(test)]
pub(super) fn get_cached_diagram_in_memory_for_test(hash: u64) -> Option<CachedDiagram> {
    get_cached_diagram_in_memory(hash)
}

fn evict_render_cache_by_hash(hash: u64) {
    let mut cache = RENDER_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let keys: Vec<(u64, RenderProfile)> = cache
        .entries
        .keys()
        .filter(|(entry_hash, _)| *entry_hash == hash)
        .copied()
        .collect();
    for key in keys {
        if let Some(entry) = cache.entries.remove(&key) {
            let _ = fs::remove_file(&entry.path);
        }
        if let Some(pos) = cache.order.iter().position(|entry| *entry == key) {
            cache.order.remove(pos);
        }
    }
    // Remove non-resident files before discovery can resurrect them.
    if let Ok(entries) = fs::read_dir(&cache.cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some((file_hash, _, _)) = parse_cache_filename(&path)
                && file_hash == hash
            {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

pub(super) fn get_cached_diagram(hash: u64, min_width: Option<u32>) -> Option<CachedDiagram> {
    let profile = current_render_profile();
    let mut cache = RENDER_CACHE.lock().ok()?;
    if let Some(diagram) = cache.get(hash, min_width, Some(profile)) {
        return Some(diagram);
    }
    cache.get(hash, min_width, None)
}

/// Hot-path lookup that avoids per-frame filesystem calls. Missing files still
/// fail safely when decoded, so eager existence checks add latency, not safety.
pub(super) fn get_cached_diagram_in_memory(hash: u64) -> Option<CachedDiagram> {
    let profile = current_render_profile();
    let mut cache = RENDER_CACHE.lock().ok()?;
    cache
        .get_in_memory(hash, profile)
        .or_else(|| cache.get_in_memory(hash, RenderProfile::default()))
        // The draw path may not carry the aspect profile used during rendering.
        .or_else(|| cache.get_in_memory_any_profile(hash))
}

fn get_cached_diagram_for_profile(
    hash: u64,
    min_width: Option<u32>,
    profile: RenderProfile,
) -> Option<CachedDiagram> {
    let mut cache = RENDER_CACHE.lock().ok()?;
    cache.get(hash, min_width, Some(profile))
}

pub fn get_cached_path(hash: u64) -> Option<PathBuf> {
    get_cached_diagram(hash, None).map(|c| c.path)
}

#[cfg(feature = "renderer")]
fn invalidate_cached_image(hash: u64) {
    if let Ok(mut state) = IMAGE_STATE.lock() {
        state.remove(&hash);
    }
    if let Ok(mut kitty) = KITTY_VIEWPORT_STATE.lock() {
        kitty.remove(&hash);
    }
    if let Ok(mut source) = SOURCE_CACHE.lock() {
        source.remove(hash);
    }
}

/// Result of rendering a Mermaid diagram.
pub enum RenderResult {
    /// Rendered image and its state-lookup hash.
    Image {
        hash: u64,
        path: PathBuf,
        width: u32,
        height: u32,
    },
    /// Rendering error.
    Error(String),
}

/// Returns whether a code-block language denotes Mermaid.
pub fn is_mermaid_lang(lang: &str) -> bool {
    let lang_lower = lang.to_lowercase();
    let is_mermaid = lang_lower == "mermaid" || lang_lower.starts_with("mermaid");
    if is_mermaid {
        // Prewarm lazily so diagram-free sessions avoid the font scan.
        // The callee's OnceLock ensures only the first detection spawns work.
        super::runtime::prewarm_svg_font_db_async();
    }
    is_mermaid
}

// Complexity caps bound renderer memory use.
const MAX_NODES: usize = 100;
const MAX_EDGES: usize = 200;

pub(super) fn estimate_diagram_size(content: &str) -> (usize, usize) {
    svg::estimate_diagram_size(content)
}

pub(super) fn calculate_render_size(
    node_count: usize,
    edge_count: usize,
    terminal_width: Option<u16>,
) -> (f64, f64) {
    let (width, height) = svg::calculate_render_size(node_count, edge_count, terminal_width);
    if let Some(aspect) = current_render_profile().preferred_aspect_ratio() {
        let profile_height = (width / aspect as f64).clamp(300.0, DEFAULT_RENDER_HEIGHT as f64);
        (width, profile_height)
    } else {
        (width, height)
    }
}

#[cfg(feature = "renderer")]
fn svg_dimension_to_u32(value: f32) -> u32 {
    if value.is_finite() && value > 0.0 {
        value.round().clamp(1.0, u32::MAX as f32) as u32
    } else {
        1
    }
}

#[cfg(feature = "renderer")]
fn write_output_png_cached_fonts(
    svg: &str,
    output: &Path,
    render_cfg: &RenderConfig,
    theme: &Theme,
) -> anyhow::Result<()> {
    svg::write_output_png_cached_fonts(svg, output, render_cfg, theme)
}

/// Renders a Mermaid code block to a cached PNG.
pub fn render_mermaid(content: &str) -> RenderResult {
    render_mermaid_sized(content, None)
}

/// Renders with an explicit terminal width for adaptive sizing.
pub fn render_mermaid_sized(content: &str, terminal_width: Option<u16>) -> RenderResult {
    render_mermaid_sized_internal(content, terminal_width, true)
}

/// Renders without exposing the diagram in the user-visible diagram pane.
pub fn render_mermaid_untracked(content: &str, terminal_width: Option<u16>) -> RenderResult {
    render_mermaid_sized_internal(content, terminal_width, false)
}

pub(super) fn bump_deferred_render_epoch() {
    DEFERRED_RENDER_EPOCH.fetch_add(1, Ordering::Relaxed);
    bump_debug_stats(|s| s.deferred_epoch_bumps += 1);
}

fn bump_debug_stats(f: impl FnOnce(&mut MermaidDebugStats)) {
    if let Ok(mut state) = MERMAID_DEBUG.lock() {
        f(&mut state.stats);
    }
}

pub fn deferred_render_epoch() -> u64 {
    DEFERRED_RENDER_EPOCH.load(Ordering::Relaxed)
}

/// Simulates deferred completion so tests need not race the worker thread.
pub fn debug_bump_deferred_render_epoch_for_tests() {
    bump_deferred_render_epoch();
}

fn deferred_render_sender() -> &'static mpsc::Sender<DeferredRenderTask> {
    DEFERRED_RENDER_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<DeferredRenderTask>();
        if let Err(err) = std::thread::Builder::new()
            .name("jcode-mermaid-deferred".to_string())
            .spawn(move || deferred_render_worker(rx))
        {
            crate::log_warn(&format!(
                "Failed to spawn mermaid deferred worker, falling back to synchronous rendering: {}",
                err
            ));
        }
        tx
    })
}

fn deferred_render_worker(rx: mpsc::Receiver<DeferredRenderTask>) {
    for task in rx {
        let pending_request = match PENDING_RENDER_REQUESTS.lock() {
            Ok(pending) => pending
                .get(&task.render_key)
                .map(|request| (request.register_active, request.diagram_scope)),
            Err(poisoned) => poisoned
                .into_inner()
                .get(&task.render_key)
                .map(|request| (request.register_active, request.diagram_scope)),
        };

        let Some((register_active, diagram_scope)) = pending_request else {
            bump_debug_stats(|s| s.deferred_worker_skips += 1);
            continue;
        };

        bump_debug_stats(|s| s.deferred_worker_renders += 1);

        let _scope = RegistrationScopeGuard::new(diagram_scope);
        let profile = task.render_key.2;
        let _ = with_preferred_aspect_ratio(profile.preferred_aspect_ratio(), || {
            render_mermaid_sized_internal(&task.content, task.terminal_width, register_active)
        });

        if let Ok(mut pending) = PENDING_RENDER_REQUESTS.lock() {
            pending.remove(&task.render_key);
        }
        bump_deferred_render_epoch();
        crate::notify_render_completed();
    }
}

pub(crate) fn is_likely_stream_update(previous: &str, next: &str) -> bool {
    let previous = previous.trim_end();
    let next = next.trim_end();
    if previous == next || previous.len().min(next.len()) < 16 {
        return false;
    }
    next.starts_with(previous) || previous.starts_with(next)
}

/// Returns cached output immediately or queues rendering and returns `None`.
pub fn render_mermaid_deferred(content: &str, terminal_width: Option<u16>) -> Option<RenderResult> {
    render_mermaid_deferred_with_registration(content, terminal_width, false)
}

pub fn render_mermaid_deferred_with_registration(
    content: &str,
    terminal_width: Option<u16>,
    register_active: bool,
) -> Option<RenderResult> {
    render_mermaid_deferred_inner(content, terminal_width, register_active, None)
}

pub fn render_mermaid_deferred_with_stream_scope(
    content: &str,
    terminal_width: Option<u16>,
    stream_scope: u64,
) -> Option<RenderResult> {
    render_mermaid_deferred_inner(content, terminal_width, false, Some(stream_scope))
}

fn render_mermaid_deferred_inner(
    content: &str,
    terminal_width: Option<u16>,
    register_active: bool,
    stream_scope: Option<u64>,
) -> Option<RenderResult> {
    let hash = hash_content(content);
    let (node_count, edge_count) = estimate_diagram_size(content);

    // Bypass queue bookkeeping so tests cannot register diagrams after reset.
    if is_synchronous_render_mode() {
        let result = render_mermaid_sized_internal(content, terminal_width, register_active);
        return Some(result);
    }

    if node_count > MAX_NODES || edge_count > MAX_EDGES {
        return Some(RenderResult::Error(format!(
            "Diagram too complex ({} nodes, {} edges). Max: {} nodes, {} edges.",
            node_count, edge_count, MAX_NODES, MAX_EDGES
        )));
    }

    let (target_width, _) = calculate_render_size(node_count, edge_count, terminal_width);
    let target_width_u32 = target_width as u32;
    let render_profile = current_render_profile();

    if let Some(cached) =
        get_cached_diagram_for_profile(hash, Some(target_width_u32), render_profile)
    {
        if register_active {
            register_active_diagram(hash, cached.width, cached.height, None);
        }
        return Some(RenderResult::Image {
            hash,
            path: cached.path,
            width: cached.width,
            height: cached.height,
        });
    }

    if let Some(err) = RENDER_ERRORS
        .lock()
        .ok()
        .and_then(|errors| errors.get(&hash).cloned())
    {
        return Some(RenderResult::Error(err));
    }

    let render_key = (hash, target_width_u32, render_profile);
    let should_enqueue =
        match PENDING_RENDER_REQUESTS.lock() {
            Ok(mut pending) => {
                let mut superseded = 0u64;
                pending.retain(|(_, pending_width, pending_profile), request| {
                    let same_stream_scope =
                        request.stream_scope.is_some() && request.stream_scope == stream_scope;
                    let same_profile = *pending_profile == render_profile;
                    let same_terminal_width = request.terminal_width == terminal_width;
                    let compatible_width =
                        cached_width_satisfies(*pending_width, Some(target_width_u32))
                            || cached_width_satisfies(target_width_u32, Some(*pending_width));
                    let supersede = same_stream_scope
                        && same_profile
                        && same_terminal_width
                        && compatible_width
                        && is_likely_stream_update(&request.content, content);
                    if supersede {
                        superseded = superseded.saturating_add(1);
                    }
                    !supersede
                });
                if superseded > 0
                    && let Ok(mut state) = MERMAID_DEBUG.lock()
                {
                    state.stats.deferred_superseded =
                        state.stats.deferred_superseded.saturating_add(superseded);
                }

                if let Some((_, existing_request)) = pending.iter_mut().find(
                    |((pending_hash, pending_width, pending_profile), _)| {
                        *pending_hash == hash
                            && *pending_profile == render_profile
                            && cached_width_satisfies(*pending_width, Some(target_width_u32))
                    },
                ) {
                    if register_active {
                        existing_request.register_active = true;
                    }
                    bump_debug_stats(|s| s.deferred_deduped += 1);
                    false
                } else {
                    match pending.entry(render_key) {
                        Entry::Occupied(mut occupied) => {
                            if register_active {
                                occupied.get_mut().register_active = true;
                            }
                            bump_debug_stats(|s| s.deferred_deduped += 1);
                            false
                        }
                        Entry::Vacant(vacant) => {
                            vacant.insert(PendingDeferredRender {
                                register_active,
                                terminal_width,
                                content: content.to_string(),
                                stream_scope,
                                diagram_scope: current_diagram_scope(),
                            });
                            bump_debug_stats(|s| s.deferred_enqueued += 1);
                            true
                        }
                    }
                }
            }
            Err(_) => {
                return Some(render_mermaid_sized_internal(
                    content,
                    terminal_width,
                    register_active,
                ));
            }
        };

    if should_enqueue {
        let task = DeferredRenderTask {
            content: content.to_string(),
            terminal_width,
            render_key,
        };
        if deferred_render_sender().send(task).is_err() {
            if let Ok(mut pending) = PENDING_RENDER_REQUESTS.lock() {
                pending.remove(&render_key);
            }
            return Some(render_mermaid_sized_internal(
                content,
                terminal_width,
                register_active,
            ));
        }
    }

    None
}

fn render_mermaid_sized_internal(
    content: &str,
    terminal_width: Option<u16>,
    register_active: bool,
) -> RenderResult {
    if let Ok(mut state) = MERMAID_DEBUG.lock() {
        state.stats.total_requests += 1;
        state.stats.last_content_len = Some(content.len());
        state.stats.last_error = None;
        state.stats.last_parse_ms = None;
        state.stats.last_layout_ms = None;
        state.stats.last_svg_ms = None;
        state.stats.last_png_ms = None;
    }

    let hash = hash_content(content);
    let render_profile = current_render_profile();

    let (node_count, edge_count) = estimate_diagram_size(content);
    #[cfg(feature = "renderer")]
    let complexity = node_count + edge_count;

    if let Ok(mut state) = MERMAID_DEBUG.lock() {
        state.stats.last_nodes = Some(node_count);
        state.stats.last_edges = Some(edge_count);
    }

    if node_count > MAX_NODES || edge_count > MAX_EDGES {
        let msg = format!(
            "Diagram too complex ({} nodes, {} edges). Max: {} nodes, {} edges.",
            node_count, edge_count, MAX_NODES, MAX_EDGES
        );
        if let Ok(mut state) = MERMAID_DEBUG.lock() {
            state.stats.render_errors += 1;
            state.stats.last_error = Some(msg.clone());
        }
        return RenderResult::Error(msg);
    }

    let (target_width, target_height) =
        calculate_render_size(node_count, edge_count, terminal_width);
    let target_width_u32 = target_width as u32;
    let target_height_u32 = target_height as u32;

    if let Ok(mut state) = MERMAID_DEBUG.lock() {
        state.stats.last_target_width = Some(target_width_u32);
        state.stats.last_target_height = Some(target_height_u32);
    }

    if let Some(cached) =
        get_cached_diagram_for_profile(hash, Some(target_width_u32), render_profile)
    {
        if let Ok(mut state) = MERMAID_DEBUG.lock() {
            state.stats.cache_hits += 1;
            state.stats.last_hash = Some(format!("{:016x}", hash));
        }
        if register_active {
            register_active_diagram(hash, cached.width, cached.height, None);
        }
        return RenderResult::Image {
            hash,
            path: cached.path,
            width: cached.width,
            height: cached.height,
        };
    }

    if let Ok(mut state) = MERMAID_DEBUG.lock() {
        state.stats.cache_misses += 1;
        state.stats.last_hash = Some(format!("{:016x}", hash));
    }

    #[cfg(not(feature = "renderer"))]
    {
        let msg = "Mermaid rendering is disabled in this build".to_string();
        if let Ok(mut errors) = RENDER_ERRORS.lock() {
            super::bounded_bookkeeping_insert(&mut errors, hash, msg.clone());
        }
        if let Ok(mut state) = MERMAID_DEBUG.lock() {
            state.stats.render_errors += 1;
            state.stats.last_error = Some(msg.clone());
        }
        RenderResult::Error(msg)
    }

    #[cfg(feature = "renderer")]
    {
        let png_path = {
            let cache = RENDER_CACHE
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            cache.cache_path(hash, target_width_u32, render_profile)
        };
        let png_path_clone = png_path.clone();

        let _render_guard = RENDER_WORK_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // A worker may have populated the cache while this thread waited for the lock.
        if let Some(cached) =
            get_cached_diagram_for_profile(hash, Some(target_width_u32), render_profile)
        {
            if let Ok(mut errors) = RENDER_ERRORS.lock() {
                errors.remove(&hash);
            }
            if let Ok(mut state) = MERMAID_DEBUG.lock() {
                state.stats.cache_hits += 1;
                state.stats.last_hash = Some(format!("{:016x}", hash));
            }
            if register_active {
                register_active_diagram(hash, cached.width, cached.height, None);
            }
            return RenderResult::Image {
                hash,
                path: cached.path,
                width: cached.width,
                height: cached.height,
            };
        }

        // Catch renderer panics without emitting the library's panic-hook output.
        let content_owned = content.to_string();

        let prev_hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));

        let render_start = Instant::now();
        let render_result = panic::catch_unwind(move || -> Result<RenderStageBreakdown, String> {
            let theme = terminal_theme();
            let layout_config = build_layout_config(complexity, render_profile);

            // Width-bucket misses can reuse the width-independent layout tier.
            let cache_key = layout_cache_key(hash, &theme, &layout_config, render_profile);
            let (layout, parse_ms, layout_ms) = if let Some(layout) = layout_cache_get(&cache_key) {
                (layout, 0.0, 0.0)
            } else {
                let parse_start = Instant::now();
                let parsed =
                    parse_mermaid(&content_owned).map_err(|e| format!("Parse error: {}", e))?;
                let parse_ms = parse_start.elapsed().as_secs_f32() * 1000.0;

                let layout_start = Instant::now();
                let layout = Arc::new(compute_layout(&parsed.graph, &theme, &layout_config));
                let layout_ms = layout_start.elapsed().as_secs_f32() * 1000.0;
                #[cfg(test)]
                record_layout_computation_for_test(hash);
                layout_cache_insert(cache_key, Arc::clone(&layout));
                (layout, parse_ms, layout_ms)
            };

            let svg_start = Instant::now();
            let output_dimensions = Some((target_width as f32, target_height as f32));
            // The compatibility path derives dimensions by retargeting the SVG;
            // the mmdr size API returns them directly when enabled.
            let (svg, dimensions) =
                render_svg_for_png(&layout, &theme, &layout_config, output_dimensions);
            let svg_ms = svg_start.elapsed().as_secs_f32() * 1000.0;

            let render_config = RenderConfig {
                width: dimensions.width,
                height: dimensions.height,
                background: theme.background.clone(),
            };

            if let Some(parent) = png_path_clone.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create cache directory: {}", e))?;
            }

            let png_start = Instant::now();
            write_output_png_cached_fonts(&svg, &png_path_clone, &render_config, &theme)
                .map_err(|e| format!("Render error: {}", e))?;
            let png_ms = png_start.elapsed().as_secs_f32() * 1000.0;

            Ok(RenderStageBreakdown {
                parse_ms,
                layout_ms,
                svg_ms,
                png_ms,
                measured_width: svg_dimension_to_u32(dimensions.width),
                measured_height: svg_dimension_to_u32(dimensions.height),
                viewbox_width: svg_dimension_to_u32(dimensions.viewbox_width),
                viewbox_height: svg_dimension_to_u32(dimensions.viewbox_height),
            })
        });

        panic::set_hook(prev_hook);

        let render_ms = render_start.elapsed().as_secs_f32() * 1000.0;
        let stage_breakdown = match render_result {
            Ok(Ok(stage_breakdown)) => {
                if let Ok(mut errors) = RENDER_ERRORS.lock() {
                    errors.remove(&hash);
                }
                if let Ok(mut state) = MERMAID_DEBUG.lock() {
                    state.stats.render_success += 1;
                    state.stats.last_render_ms = Some(render_ms);
                    state.stats.last_parse_ms = Some(stage_breakdown.parse_ms);
                    state.stats.last_layout_ms = Some(stage_breakdown.layout_ms);
                    state.stats.last_svg_ms = Some(stage_breakdown.svg_ms);
                    state.stats.last_png_ms = Some(stage_breakdown.png_ms);
                    state.stats.last_measured_width = Some(stage_breakdown.measured_width);
                    state.stats.last_measured_height = Some(stage_breakdown.measured_height);
                    state.stats.last_viewbox_width = Some(stage_breakdown.viewbox_width);
                    state.stats.last_viewbox_height = Some(stage_breakdown.viewbox_height);
                }
                stage_breakdown
            }
            Ok(Err(e)) => {
                if let Ok(mut errors) = RENDER_ERRORS.lock() {
                    super::bounded_bookkeeping_insert(&mut errors, hash, e.clone());
                }
                if let Ok(mut state) = MERMAID_DEBUG.lock() {
                    state.stats.render_errors += 1;
                    state.stats.last_render_ms = Some(render_ms);
                    state.stats.last_error = Some(e.clone());
                }
                return RenderResult::Error(e);
            }
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic in mermaid renderer".to_string()
                };
                if let Ok(mut errors) = RENDER_ERRORS.lock() {
                    super::bounded_bookkeeping_insert(
                        &mut errors,
                        hash,
                        format!("Renderer panic: {}", msg),
                    );
                }
                if let Ok(mut state) = MERMAID_DEBUG.lock() {
                    state.stats.render_errors += 1;
                    state.stats.last_render_ms = Some(render_ms);
                    state.stats.last_error = Some(format!("Renderer panic: {}", msg));
                }
                return RenderResult::Error(format!("Renderer panic: {}", msg));
            }
        };

        let (width, height) = get_png_dimensions(&png_path).unwrap_or((
            stage_breakdown.measured_width,
            stage_breakdown.measured_height,
        ));

        if let Ok(mut state) = MERMAID_DEBUG.lock() {
            state.stats.last_png_width = Some(width);
            state.stats.last_png_height = Some(height);
        }

        {
            let mut cache = RENDER_CACHE
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            cache.insert(
                hash,
                render_profile,
                CachedDiagram {
                    path: png_path.clone(),
                    width,
                    height,
                },
            );
        }
        // A new size/path invalidates decoded widget state.
        invalidate_cached_image(hash);

        if register_active {
            register_active_diagram(hash, width, height, None);
        }

        RenderResult::Image {
            hash,
            path: png_path,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod font_prewarm_tests {
    #[test]
    fn mermaid_detection_triggers_font_db_prewarm() {
        assert!(!super::is_mermaid_lang("rust"), "sanity: non-mermaid");
        // Parallel tests may already have initialized the OnceLock.
        assert!(super::is_mermaid_lang("mermaid"));
        assert!(
            crate::SVG_FONT_DB_PREWARM_STARTED.get().is_some(),
            "first mermaid sighting must kick off the font-DB prewarm"
        );
    }
}
