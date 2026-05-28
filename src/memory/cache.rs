use crate::memory_graph::MemoryGraph;
use jcode_cache_isolation::SCHEMA_VERSION;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

// === Graph Cache ===

/// Composite key for the in-memory graph cache.
///
/// `path` (absolute, per-workspace) is the primary axis — different workspaces
/// derive different paths via `project_memory_path()` so cross-workspace
/// confusion is already prevented at the path level. `schema_version`
/// (`jcode_cache_isolation::SCHEMA_VERSION`) is folded in as a defensive
/// second axis so a one-knob bump of the cache-isolation contract atomically
/// invalidates every cached graph in the long-lived process without requiring
/// a path-level migration.
///
/// See TASK-89: GraphCache is the LOW-risk defensive layer; the mtime check
/// in `cached_graph` is the primary correctness guard.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GraphCacheKey {
    path: PathBuf,
    schema_version: u32,
}

impl GraphCacheKey {
    fn for_path(path: PathBuf) -> Self {
        Self {
            path,
            schema_version: SCHEMA_VERSION,
        }
    }
}

struct GraphCacheEntry {
    graph: MemoryGraph,
    modified: Option<SystemTime>,
}

struct GraphCache {
    entries: HashMap<GraphCacheKey, GraphCacheEntry>,
}

impl GraphCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

static GRAPH_CACHE: OnceLock<Mutex<GraphCache>> = OnceLock::new();

fn graph_cache() -> &'static Mutex<GraphCache> {
    GRAPH_CACHE.get_or_init(|| Mutex::new(GraphCache::new()))
}

fn graph_mtime(path: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

pub(super) fn cached_graph(path: &PathBuf) -> Option<MemoryGraph> {
    let modified = graph_mtime(path);
    let cache = graph_cache().lock().ok()?;
    let key = GraphCacheKey::for_path(path.clone());
    let entry = cache.entries.get(&key)?;
    if entry.modified == modified {
        Some(entry.graph.clone())
    } else {
        None
    }
}

pub(super) fn cache_graph(path: PathBuf, graph: &MemoryGraph) {
    let modified = graph_mtime(&path);
    if let Ok(mut cache) = graph_cache().lock() {
        cache.entries.insert(
            GraphCacheKey::for_path(path),
            GraphCacheEntry {
                graph: graph.clone(),
                modified,
            },
        );
    }
}

/// Drop every cached `MemoryGraph` from the process-wide `GRAPH_CACHE`.
///
/// Explicit invalidation hook (TASK-89 AC#3) for session-resume,
/// workspace-switch, and provider/model-change events. The mtime check in
/// `cached_graph` already keeps individual entries fresh against on-disk
/// mutation, and `SCHEMA_VERSION` in `GraphCacheKey` already invalidates
/// the whole cache on a contract bump; this function exists so the TUI
/// backend can eagerly reclaim memory and signal intent at the event
/// boundary instead of waiting for natural eviction.
pub fn clear_graph_cache() {
    if let Ok(mut cache) = graph_cache().lock() {
        cache.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TASK-89 AC#2/AC#4 (GraphCache defensive layer):
    /// the same on-disk path with a different `schema_version` must produce
    /// a distinct cache key, so a bump of `jcode_cache_isolation::SCHEMA_VERSION`
    /// atomically invalidates every cached graph entry without requiring a
    /// path-level migration. Equal `(path, schema_version)` pairs must hit.
    #[test]
    fn graph_cache_key_isolates_by_schema_version() {
        let p = PathBuf::from("/tmp/jcode-test-graph.json");
        let a = GraphCacheKey {
            path: p.clone(),
            schema_version: SCHEMA_VERSION,
        };
        let b = GraphCacheKey {
            path: p.clone(),
            schema_version: SCHEMA_VERSION.wrapping_add(1),
        };
        let a2 = GraphCacheKey::for_path(p);
        assert_eq!(a, a2);
        assert_ne!(a, b);
    }

    /// TASK-89 AC#3: `clear_graph_cache` must drop every entry from the
    /// process-wide `GRAPH_CACHE` so a session-resume hook reliably
    /// reclaims memory instead of waiting for natural mtime-driven
    /// invalidation on the next read of each path.
    #[test]
    fn clear_graph_cache_drops_all_entries() {
        // Serialize against any other test mutating GRAPH_CACHE.
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());

        clear_graph_cache();
        {
            let mut cache = graph_cache().lock().unwrap();
            cache.entries.insert(
                GraphCacheKey::for_path(PathBuf::from("/tmp/jcode-test-graph-a.json")),
                GraphCacheEntry {
                    graph: MemoryGraph::default(),
                    modified: None,
                },
            );
            cache.entries.insert(
                GraphCacheKey::for_path(PathBuf::from("/tmp/jcode-test-graph-b.json")),
                GraphCacheEntry {
                    graph: MemoryGraph::default(),
                    modified: None,
                },
            );
            assert_eq!(cache.entries.len(), 2);
        }
        clear_graph_cache();
        let cache = graph_cache().lock().unwrap();
        assert!(cache.entries.is_empty(), "GRAPH_CACHE must be empty");
    }
}
