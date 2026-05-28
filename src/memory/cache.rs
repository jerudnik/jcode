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
}
