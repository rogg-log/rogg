//! The cache directory (spec §4.2): UTF-8 conversions, extracted archive members and decompressed `.gz` files, keyed
//! by their origin, written atomically, and evicted least-recently-used above a size limit. Entries in use are leased
//! and never evicted. Captured stdin and FIFO data lives in `streams/`.

use crate::error::CoreError;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Default size limit (spec §4.2: 20 GB).
pub const DEFAULT_LIMIT_BYTES: u64 = 20 * 1024 * 1024 * 1024;
/// Captured stdin and FIFO data, one `stream-<pid>-<n>.log` per stream.
pub const STREAMS_DIR: &str = "streams";

/// FNV-1a over `parts`, with a separator after each part so `["ab", "c"]` and `["a", "bc"]` differ.
pub fn fnv1a(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for part in parts {
        for &b in *part {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The key of a cache entry derived from the file at `path`: its canonical path, the member chain inside it (empty for
/// the file itself), its size and modification time, and `purpose` (what the entry holds). A changed origin never
/// reuses a stale entry, and different products of one origin never collide.
pub fn origin_key(path: &Path, members: &[&str], purpose: &str) -> std::io::Result<u64> {
    let meta = std::fs::metadata(path)?;
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    let mut parts: Vec<&[u8]> = vec![canonical.as_os_str().as_encoded_bytes()];
    parts.extend(members.iter().map(|m| m.as_bytes()));
    let (len, mtime) = (meta.len().to_le_bytes(), mtime.to_le_bytes());
    parts.extend([&len[..], &mtime[..], purpose.as_bytes()]);
    Ok(fnv1a(&parts))
}

/// A writer-unique temporary name next to `dst`: `<dst>.<pid>.<n>.partial`. Concurrent writers of one entry never
/// share or delete each other's output, and a sweep can tell whether the writer is still running.
pub fn partial_path(dst: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut name = dst.as_os_str().to_owned();
    name.push(format!(".{}.{}.partial", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
    PathBuf::from(name)
}

/// The writer of a `<name>.<pid>.<n>.partial` file.
fn partial_owner(name: &str) -> Option<i32> {
    let mut parts = name.strip_suffix(".partial")?.rsplitn(3, '.');
    parts.next()?.parse::<u64>().ok()?;
    let pid = parts.next()?.parse().ok()?;
    parts.next().map(|_| pid)
}

/// The writer of a `stream-<pid>-<n>.log` capture.
fn stream_owner(name: &str) -> Option<i32> {
    name.strip_prefix("stream-")?.split('-').next()?.parse().ok()
}

fn process_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    // SAFETY: signal 0 delivers nothing; it only checks that the process exists.
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    alive || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Marks a file as recently used (the LRU order is the modification time).
fn touch(path: &Path) {
    if let Ok(file) = File::options().append(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Evicted {
    pub removed: usize,
    pub freed: u64,
    /// Bytes left in the cache directory (leased entries and partial files included).
    pub remaining: u64,
}

pub struct Cache {
    dir: PathBuf,
    limit: u64,
    leases: Mutex<HashMap<PathBuf, usize>>,
}

/// Keeps a cache entry from being evicted while a document reads it.
pub struct CacheLease {
    cache: Arc<Cache>,
    path: PathBuf,
}

impl CacheLease {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CacheLease {
    fn drop(&mut self) {
        let mut leases = self.cache.leases.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(n) = leases.get_mut(&self.path) {
            *n -= 1;
            if *n == 0 {
                leases.remove(&self.path);
            }
        }
    }
}

impl Cache {
    pub fn new(dir: impl Into<PathBuf>, limit: u64) -> Arc<Cache> {
        Arc::new(Cache { dir: dir.into(), limit, leases: Mutex::new(HashMap::new()) })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// `<dir>/<key as 16 hex digits>.<ext>`.
    pub fn entry(&self, key: u64, ext: &str) -> PathBuf {
        self.dir.join(format!("{key:016x}.{ext}"))
    }

    pub fn streams_dir(&self) -> PathBuf {
        self.dir.join(STREAMS_DIR)
    }

    /// Marks `path` in use until the lease drops (eviction skips it) and refreshes its LRU time. An entry may be
    /// leased before it is written, so eviction cannot remove it between its creation and its first use.
    pub fn lease(self: &Arc<Self>, path: &Path) -> CacheLease {
        touch(path);
        *self.leases.lock().unwrap_or_else(|e| e.into_inner()).entry(path.to_path_buf()).or_insert(0) += 1;
        CacheLease { cache: self.clone(), path: path.to_path_buf() }
    }

    pub fn is_leased(&self, path: &Path) -> bool {
        self.leases.lock().unwrap_or_else(|e| e.into_inner()).contains_key(path)
    }

    /// Writes an entry atomically: `write` fills a writer-unique partial file, which is renamed over `dst` on success
    /// and removed on error or cancellation, so `dst` is either complete or absent.
    pub fn write_entry(&self, dst: &Path, write: impl FnOnce(&mut File) -> Result<(), CoreError>) -> Result<(), CoreError> {
        std::fs::create_dir_all(&self.dir).map_err(CoreError::io(&self.dir))?;
        let partial = partial_path(dst);
        let result = File::create(&partial).map_err(CoreError::io(&partial)).and_then(|mut file| {
            write(&mut file)?;
            file.flush().map_err(CoreError::io(&partial))
        });
        let result = result.and_then(|()| std::fs::rename(&partial, dst).map_err(CoreError::io(dst)));
        if result.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        result
    }

    /// Removes partial files whose writer is no longer running (a crash or kill mid-write). Run at startup.
    pub fn sweep_partials(&self) -> usize {
        sweep(&self.dir, partial_owner)
    }

    /// Removes stdin / FIFO captures whose process is no longer running (stream tabs are never restored).
    pub fn sweep_streams(&self) -> usize {
        sweep(&self.streams_dir(), stream_owner)
    }

    /// Deletes least-recently-used entries until the directory holds at most the limit. Leased entries, partial files
    /// and the streams folder are never removed.
    pub fn evict(&self) -> Evicted {
        self.evict_to(self.limit)
    }

    pub fn evict_to(&self, limit: u64) -> Evicted {
        let (candidates, total) = self.eviction_candidates();
        self.remove_candidates(candidates, total, limit)
    }

    /// The unleased, non-partial files of the directory (least recently used first) and the bytes the directory holds.
    fn eviction_candidates(&self) -> (Vec<(SystemTime, u64, PathBuf)>, u64) {
        let Ok(read) = std::fs::read_dir(&self.dir) else { return (Vec::new(), 0) };
        let mut candidates = Vec::new();
        let mut total = 0;
        for entry in read.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            total += meta.len();
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".partial") || self.is_leased(&path) {
                continue;
            }
            candidates.push((meta.modified().unwrap_or(UNIX_EPOCH), meta.len(), path));
        }
        candidates.sort();
        (candidates, total)
    }

    /// Removes `candidates` in order until `total` bytes are down to `limit`. The lease check and the removal happen
    /// under the lease lock, so a lease taken after the candidates were collected still protects its entry.
    fn remove_candidates(&self, candidates: Vec<(SystemTime, u64, PathBuf)>, total: u64, limit: u64) -> Evicted {
        let mut out = Evicted { remaining: total, ..Evicted::default() };
        for (_, len, path) in candidates {
            if out.remaining <= limit {
                break;
            }
            let leases = self.leases.lock().unwrap_or_else(|e| e.into_inner());
            if leases.contains_key(&path) {
                continue;
            }
            if std::fs::remove_file(&path).is_ok() {
                out.removed += 1;
                out.freed += len;
                out.remaining -= len;
            }
        }
        out
    }
}

fn sweep(dir: &Path, owner: fn(&str) -> Option<i32>) -> usize {
    let Ok(read) = std::fs::read_dir(dir) else { return 0 };
    read.flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            owner(&name).is_some_and(|pid| !process_alive(pid)) && std::fs::remove_file(e.path()).is_ok()
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn set_mtime(path: &Path, t: SystemTime) {
        File::options().append(true).open(path).unwrap().set_modified(t).unwrap();
    }

    fn dead_pid() -> i32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id() as i32;
        child.wait().unwrap();
        pid
    }

    #[test]
    fn keys_follow_origin_members_and_purpose() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.log");
        std::fs::write(&a, "one").unwrap();
        let base = origin_key(&a, &[], "utf8").unwrap();
        assert_eq!(base, origin_key(&a, &[], "utf8").unwrap(), "stable");
        assert_ne!(base, origin_key(&a, &["x.log"], "utf8").unwrap(), "member");
        assert_ne!(base, origin_key(&a, &[], "member").unwrap(), "purpose");
        assert_ne!(origin_key(&a, &["ab", "c"], "m").unwrap(), origin_key(&a, &["a", "bc"], "m").unwrap(), "chain parts stay apart");
        std::fs::write(&a, "one two").unwrap();
        assert_ne!(base, origin_key(&a, &[], "utf8").unwrap(), "a changed origin gets a new key");
        assert!(origin_key(&dir.path().join("missing"), &[], "utf8").is_err());
    }

    #[test]
    fn write_entry_renames_on_success_and_leaves_nothing_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join("cache"), DEFAULT_LIMIT_BYTES);
        let ok = cache.entry(1, "member");
        cache.write_entry(&ok, |f| f.write_all(b"abc").map_err(CoreError::io(&ok))).unwrap();
        assert_eq!(std::fs::read(&ok).unwrap(), b"abc");
        let bad = cache.entry(2, "member");
        let err = cache.write_entry(&bad, |f| {
            f.write_all(b"half").unwrap();
            Err(CoreError::Cancelled)
        });
        assert!(matches!(err, Err(CoreError::Cancelled)));
        assert!(!bad.exists());
        let names: Vec<String> = std::fs::read_dir(cache.dir()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| !n.ends_with(".partial")), "{names:?}");
        assert_eq!(cache.entry(0xab, "log").file_name().unwrap(), "00000000000000ab.log");
    }

    #[test]
    fn evicts_oldest_unleased_first() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path(), 250);
        let t0 = SystemTime::now() - Duration::from_secs(3600);
        let paths: Vec<PathBuf> = ["a", "b", "c"].iter().map(|n| dir.path().join(format!("{n}.member"))).collect();
        for (i, p) in paths.iter().enumerate() {
            std::fs::write(p, [0u8; 100]).unwrap();
            set_mtime(p, t0 + Duration::from_secs(10 * i as u64));
        }
        std::fs::write(dir.path().join("x.member.1.0.partial"), [0u8; 100]).unwrap();
        // 400 bytes in total (the partial counts but is never removed): b and c go, oldest first; a is leased.
        let lease = cache.lease(&paths[0]);
        assert!(cache.is_leased(&paths[0]));
        let out = cache.evict();
        assert_eq!(out, Evicted { removed: 2, freed: 200, remaining: 200 });
        assert!(paths[0].exists() && !paths[1].exists() && !paths[2].exists());
        assert!(dir.path().join("x.member.1.0.partial").exists());
        drop(lease);
        assert!(!cache.is_leased(&paths[0]));
        assert_eq!(cache.evict_to(0).removed, 1, "released entries are evictable");
        assert!(!paths[0].exists());
    }

    #[test]
    fn leases_count_and_refresh_the_lru_time() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path(), DEFAULT_LIMIT_BYTES);
        let p = dir.path().join("e.log");
        std::fs::write(&p, "x").unwrap();
        set_mtime(&p, SystemTime::now() - Duration::from_secs(3600));
        let one = cache.lease(&p);
        let two = cache.lease(&p);
        let age = SystemTime::now().duration_since(std::fs::metadata(&p).unwrap().modified().unwrap()).unwrap();
        assert!(age < Duration::from_secs(60), "a lease marks the entry as recently used");
        drop(one);
        assert!(cache.is_leased(&p), "still leased by the second holder");
        drop(two);
        assert!(!cache.is_leased(&p));
        assert_eq!(cache.lease(&dir.path().join("not-yet.log")).path(), dir.path().join("not-yet.log"), "an entry may be leased before it exists");
    }

    #[test]
    fn sweeps_files_of_dead_processes_only() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path(), DEFAULT_LIMIT_BYTES);
        let (dead, me) = (dead_pid(), std::process::id());
        std::fs::write(dir.path().join(format!("k.utf8.{dead}.0.partial")), "x").unwrap();
        std::fs::write(dir.path().join(format!("k.utf8.{me}.1.partial")), "x").unwrap();
        std::fs::write(dir.path().join("k.utf8"), "x").unwrap();
        assert_eq!(cache.sweep_partials(), 1);
        assert!(dir.path().join(format!("k.utf8.{me}.1.partial")).exists(), "a running writer's partial survives");
        assert!(dir.path().join("k.utf8").exists());
        std::fs::create_dir_all(cache.streams_dir()).unwrap();
        std::fs::write(cache.streams_dir().join(format!("stream-{dead}-0.log")), "x").unwrap();
        std::fs::write(cache.streams_dir().join(format!("stream-{me}-0.log")), "x").unwrap();
        assert_eq!(cache.sweep_streams(), 1);
        assert!(cache.streams_dir().join(format!("stream-{me}-0.log")).exists());
    }

    #[test]
    fn partial_paths_are_unique_and_name_their_writer() {
        let dst = Path::new("/c/k.utf8");
        let (a, b) = (partial_path(dst), partial_path(dst));
        assert_ne!(a, b);
        let name = a.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(partial_owner(&name), Some(std::process::id() as i32));
        assert_eq!(stream_owner("stream-42-7.log"), Some(42));
        assert_eq!(partial_owner("k.utf8"), None);
    }

    #[test]
    fn a_lease_taken_after_the_scan_still_protects_its_entry() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path(), 0);
        let (a, b) = (dir.path().join("a.member"), dir.path().join("b.member"));
        std::fs::write(&a, [0u8; 100]).unwrap();
        std::fs::write(&b, [0u8; 100]).unwrap();
        set_mtime(&a, SystemTime::now() - Duration::from_secs(3600));
        let (candidates, total) = cache.eviction_candidates();
        assert_eq!((candidates.len(), total), (2, 200));
        let _lease = cache.lease(&a);
        let out = cache.remove_candidates(candidates, total, 0);
        assert!(a.exists(), "leased after the scan, so it survives");
        assert!(!b.exists());
        assert_eq!(out, Evicted { removed: 1, freed: 100, remaining: 100 });
    }

    #[test]
    fn a_failed_rename_leaves_no_partial() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path(), DEFAULT_LIMIT_BYTES);
        let dst = cache.entry(7, "member");
        std::fs::create_dir_all(dst.join("occupied")).unwrap();
        assert!(cache.write_entry(&dst, |f| f.write_all(b"x").map_err(CoreError::io(&dst))).is_err());
        let names: Vec<String> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| !n.ends_with(".partial")), "{names:?}");
    }
}
