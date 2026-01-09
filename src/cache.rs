use crate::models::DesktopEntryIndexed;
use crate::xdg::cache_dir;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const CACHE_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntry {
    pub path: String,
    pub size: u64,
    pub mtime_sec: u64,
    pub entry: DesktopEntryIndexed,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    roots: Vec<String>,
    entries: Vec<CachedEntry>,
}

pub struct CacheIndex {
    pub by_path: HashMap<String, CachedEntry>,
    pub needs_save: bool,
}

impl CacheIndex {
    pub fn empty() -> Self {
        Self {
            by_path: HashMap::new(),
            needs_save: false,
        }
    }
}

pub fn load(scan_roots: &[String]) -> CacheIndex {
    // Preferred: binary cache (fast to parse).
    let bin_path = cache_bin_path(scan_roots, CACHE_VERSION);
    if let Ok(data) = fs::read(&bin_path)
        && let Ok(cache) = postcard::from_bytes::<CacheFile>(&data)
        && cache.version == CACHE_VERSION
        && cache.roots == scan_roots
    {
        let mut by_path = HashMap::with_capacity(cache.entries.len());
        for ce in cache.entries {
            by_path.insert(ce.path.clone(), ce);
        }
        return CacheIndex {
            by_path,
            needs_save: false,
        };
    }

    CacheIndex::empty()
}

pub fn save(scan_roots: &[String], entries: Vec<CachedEntry>) {
    let dir = cache_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }

    let path = cache_bin_path(scan_roots, CACHE_VERSION);
    let cache = CacheFile {
        version: CACHE_VERSION,
        roots: scan_roots.to_vec(),
        entries,
    };

    let Ok(data) = postcard::to_stdvec(&cache) else {
        return;
    };

    // Best-effort write: atomic-ish via temp file + rename.
    let tmp = path.with_extension("bin.tmp");
    if fs::write(&tmp, data).is_ok() {
        let _ = fs::rename(tmp, path);
    }
}

pub fn meta_for(path: &Path) -> Option<(u64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let size = meta.len();
    let mtime = meta.modified().ok()?;
    let mtime_sec = system_time_to_secs(mtime)?;
    Some((size, mtime_sec))
}

pub fn cached_entry(
    path: &Path,
    entry: DesktopEntryIndexed,
    size: u64,
    mtime_sec: u64,
) -> CachedEntry {
    CachedEntry {
        path: path.to_string_lossy().to_string(),
        size,
        mtime_sec,
        entry,
    }
}

pub fn is_fresh(cached: &CachedEntry, size: u64, mtime_sec: u64) -> bool {
    cached.size == size && cached.mtime_sec == mtime_sec
}

pub fn cache_file_path(scan_roots: &[String]) -> PathBuf {
    cache_bin_path(scan_roots, CACHE_VERSION)
}

fn cache_bin_path(scan_roots: &[String], version: u32) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    scan_roots.hash(&mut hasher);
    let h = hasher.finish();

    cache_dir().join(format!("index-{h:x}.v{version}.bin"))
}

fn system_time_to_secs(t: SystemTime) -> Option<u64> {
    let d = t.duration_since(UNIX_EPOCH).ok()?;
    Some(d.as_secs())
}
