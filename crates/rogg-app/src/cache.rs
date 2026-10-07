//! The app's cache (spec §4.2): one `rogg_core::cache::Cache` shared by every document, so leases from all tabs keep
//! their entries; swept and trimmed on background threads.

use gpui_kit::{App, Global};
use rogg_core::cache::{Cache, DEFAULT_LIMIT_BYTES};
use std::sync::Arc;

pub struct CacheStore(pub Arc<Cache>);

impl Global for CacheStore {}

impl CacheStore {
    /// The shared cache (a fresh one under the default directory when the app globals are not installed).
    pub fn get(cx: &App) -> Arc<Cache> {
        cx.try_global::<CacheStore>().map(|s| s.0.clone()).unwrap_or_else(|| Cache::new(rogg_core::paths::cache_dir(), DEFAULT_LIMIT_BYTES))
    }
}

/// Startup maintenance: removes partial files and stream captures left by processes that died, then trims the cache
/// to its limit (M1 carry-over: stale `*.partial` sweep).
pub fn start_maintenance(cache: Arc<Cache>) {
    let _detached = std::thread::Builder::new().name("rogg-cache".into()).spawn(move || {
        cache.sweep_partials();
        cache.sweep_streams();
        cache.evict();
    });
}

/// Trims the cache on a background thread (after a document wrote new entries).
pub fn evict_soon(cx: &App) {
    let cache = CacheStore::get(cx);
    let _detached = std::thread::Builder::new().name("rogg-cache-evict".into()).spawn(move || {
        cache.evict();
    });
}
