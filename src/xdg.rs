use std::{env, path::PathBuf};

pub fn build_scan_roots(extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = Vec::<PathBuf>::new();

    // XDG_DATA_HOME (default ~/.local/share)
    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".local/share")
        });
    roots.push(data_home.join("applications"));

    // XDG_DATA_DIRS (default /usr/local/share:/usr/share)
    let data_dirs =
        env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());

    for part in data_dirs
        .split(':')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        roots.push(PathBuf::from(part).join("applications"));
    }

    // user -p paths (scan as-is + /applications variant)
    for p in extra {
        roots.push(p.clone());
        if p.file_name().map(|n| n == "applications").unwrap_or(false) {
            // already applications dir
        } else {
            roots.push(p.join("applications"));
        }
    }

    // Dedup while preserving precedence order.
    let mut out: Vec<PathBuf> = Vec::with_capacity(roots.len());
    for r in roots {
        if !out.contains(&r) {
            out.push(r);
        }
    }
    out
}

pub fn cache_dir() -> PathBuf {
    // XDG_CACHE_HOME (default ~/.cache)
    let base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".cache")
        });

    base.join("desktop-indexer")
}

pub fn data_dir() -> PathBuf {
    // XDG_DATA_HOME (default ~/.local/share)
    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".local/share")
        });

    base.join("desktop-indexer")
}

pub fn socket_path() -> PathBuf {
    // Prefer XDG_RUNTIME_DIR for per-session sockets.
    if let Some(dir) = env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("desktop-indexer.sock");
    }

    // Fallback: per-user-ish file in /tmp.
    let user = env::var("USER").unwrap_or_else(|_| "user".to_string());
    PathBuf::from("/tmp").join(format!("desktop-indexer-{user}.sock"))
}
