use crate::cache;
use crate::models::{
    DesktopActionOut, DesktopEntryIndexed, DesktopEntryOut, ParsedScanResult, ScanResult,
};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use walkdir::WalkDir;

fn timing_enabled() -> bool {
    matches!(
        std::env::var("DESKTOP_INDEXER_TIMING").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

pub fn scan_desktop_files(scan_roots: &[PathBuf], limit: Option<usize>) -> ScanResult {
    let (found_count, paths) = scan_desktop_paths(scan_roots, limit);
    let files = paths
        .into_iter()
        .map(|(_root, p)| p.to_string_lossy().to_string())
        .collect();

    ScanResult {
        scanned_roots: scan_roots
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        found_count,
        files,
    }
}

pub fn scan_and_parse_desktop_files(
    scan_roots: &[PathBuf],
    limit: Option<usize>,
) -> ParsedScanResult {
    let t_scan = Instant::now();
    let (found_count, paths) = scan_desktop_paths(scan_roots, limit);
    let dur_scan = t_scan.elapsed();

    let roots_key: Vec<String> = scan_roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // Cache only when we are building a full index.
    if limit.is_none() {
        let t_load = Instant::now();
        let cache_index = cache::load(&roots_key);
        let dur_load = t_load.elapsed();
        let cache_path = cache::cache_file_path(&roots_key);

        let mut entries: Vec<DesktopEntryIndexed> = Vec::with_capacity(paths.len());
        let mut parse_failed: usize = 0;
        let mut new_cache_entries: Vec<cache::CachedEntry> = Vec::with_capacity(paths.len());

        let mut cache_hits: usize = 0;
        let mut reparsed: usize = 0;
        let mut meta_missing: usize = 0;

        let t_work = Instant::now();

        let mut seen_ids: HashSet<String> = HashSet::new();

        for (root, p) in &paths {
            let id = compute_desktop_id(root, p);
            if !seen_ids.insert(id.clone()) {
                continue;
            }

            let Some((size, mtime_sec)) = cache::meta_for(p) else {
                meta_missing += 1;
                match parse_desktop_file_with_id(p, id) {
                    Some(entry) => {
                        entries.push(entry.clone());
                        // No metadata => don't cache
                    }
                    None => parse_failed += 1,
                }
                continue;
            };

            let p_str = p.to_string_lossy().to_string();
            if let Some(ce) = cache_index.by_path.get(&p_str)
                && cache::is_fresh(ce, size, mtime_sec)
            {
                entries.push(ce.entry.clone());
                new_cache_entries.push(ce.clone());
                cache_hits += 1;
                continue;
            }

            match parse_desktop_file_with_id(p, id) {
                Some(entry) => {
                    entries.push(entry.clone());
                    let ce = cache::cached_entry(p, entry, size, mtime_sec);
                    new_cache_entries.push(ce);
                    reparsed += 1;
                }
                None => parse_failed += 1,
            }
        }

        let dur_work = t_work.elapsed();

        // Persist updated cache (best-effort), but avoid rewriting if nothing changed.
        // In the steady state this removes a few ms of JSON serialize+write per command.
        let prev_cached_paths = cache_index.by_path.len();
        let new_cached_paths = new_cache_entries.len();
        let should_save_cache = cache_index.needs_save
            || reparsed > 0
            || (meta_missing == 0 && parse_failed == 0 && prev_cached_paths != new_cached_paths);

        let dur_save = if should_save_cache {
            let t_save = Instant::now();
            cache::save(&roots_key, new_cache_entries);
            t_save.elapsed()
        } else {
            Duration::ZERO
        };

        if timing_enabled() {
            eprintln!(
                "desktop-indexer timing: scan={:?} load_cache={:?} work={:?} save_cache={:?} paths={} found_count={} cache_hits={} reparsed={} meta_missing={} parse_failed={} cache_file={}",
                dur_scan,
                dur_load,
                dur_work,
                dur_save,
                paths.len(),
                found_count,
                cache_hits,
                reparsed,
                meta_missing,
                parse_failed,
                cache_path.display()
            );
        }

        return ParsedScanResult {
            scanned_roots: roots_key,
            found_count,
            parsed_count: entries.len(),
            parse_failed,
            entries,
        };
    }

    let mut entries: Vec<DesktopEntryIndexed> = Vec::new();
    let mut parse_failed: usize = 0;

    let t_parse = Instant::now();

    let mut seen_ids: HashSet<String> = HashSet::new();

    for (root, p) in &paths {
        let id = compute_desktop_id(root, p);
        if !seen_ids.insert(id.clone()) {
            continue;
        }

        match parse_desktop_file_with_id(p, id) {
            Some(entry) => entries.push(entry),
            None => parse_failed += 1,
        }
    }

    if timing_enabled() {
        eprintln!(
            "desktop-indexer timing: scan={:?} parse={:?} paths={} found_count={} parsed={} parse_failed={} (cache disabled due to limit)",
            dur_scan,
            t_parse.elapsed(),
            paths.len(),
            found_count,
            entries.len(),
            parse_failed
        );
    }

    ParsedScanResult {
        scanned_roots: roots_key,
        found_count,
        parsed_count: entries.len(),
        parse_failed,
        entries,
    }
}

pub fn parse_desktop_file_using_roots(
    path: &Path,
    applications_roots: &[PathBuf],
) -> Option<DesktopEntryIndexed> {
    let id = desktop_file_id_using_roots(path, applications_roots);
    parse_desktop_file_with_id(path, id)
}

pub fn desktop_file_id_using_roots(path: &Path, applications_roots: &[PathBuf]) -> String {
    for root in applications_roots {
        if path.starts_with(root) {
            return compute_desktop_id(root, path);
        }
    }

    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn parse_desktop_file_with_id(path: &Path, id: String) -> Option<DesktopEntryIndexed> {
    let data = fs::read_to_string(path).ok()?;

    #[derive(Default)]
    struct LocalizedField {
        default: Option<String>,
        best_rank: Option<usize>,
        best_value: Option<String>,
    }

    impl LocalizedField {
        fn set(&mut self, locale: Option<&str>, value: &str, prefs: &[String]) {
            match locale {
                None => {
                    self.default = Some(value.to_string());
                }
                Some(loc) => {
                    if let Some(rank) = prefs.iter().position(|p| p == loc)
                        && self.best_rank.map(|r| rank < r).unwrap_or(true)
                    {
                        self.best_rank = Some(rank);
                        self.best_value = Some(value.to_string());
                    }
                }
            }
        }

        fn resolve(&self) -> Option<String> {
            self.best_value.clone().or_else(|| self.default.clone())
        }
    }

    fn parse_bool(v: &str) -> Option<bool> {
        match v.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        }
    }

    fn split_list(v: &str) -> Vec<String> {
        // Spec uses ';' separated lists, often ending with ';'
        v.split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    fn preferred_locales() -> Vec<String> {
        // Prefer LC_ALL > LC_MESSAGES > LANG
        fn clean_locale(s: &str) -> Option<String> {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            // drop encoding and modifiers: fr_FR.UTF-8@euro => fr_FR
            let s = s.split('.').next().unwrap_or(s);
            let s = s.split('@').next().unwrap_or(s);
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        }

        let raw = std::env::var("LC_ALL")
            .ok()
            .and_then(|s| clean_locale(&s))
            .or_else(|| {
                std::env::var("LC_MESSAGES")
                    .ok()
                    .and_then(|s| clean_locale(&s))
            })
            .or_else(|| std::env::var("LANG").ok().and_then(|s| clean_locale(&s)));

        let Some(loc) = raw else {
            return Vec::new();
        };

        let mut prefs = Vec::new();
        // Exact locale match first.
        prefs.push(loc.clone());
        // language part fallback: fr_FR -> fr, pt_BR -> pt
        if let Some((lang, _)) = loc.split_once('_')
            && !lang.is_empty()
        {
            prefs.push(lang.to_string());
        }
        // hyphen variant fallback too: fr-FR -> fr
        if let Some((lang, _)) = loc.split_once('-')
            && !lang.is_empty()
        {
            prefs.push(lang.to_string());
        }

        prefs.sort();
        prefs.dedup();
        // Keep determinism (sort+dedup) but ensure the exact match stays first.
        let mut ordered = Vec::new();
        ordered.push(loc);
        for p in prefs {
            if !ordered.contains(&p) {
                ordered.push(p);
            }
        }
        ordered
    }

    fn split_key_locale(key: &str) -> (&str, Option<&str>) {
        // "Name[fr_FR]" => ("Name", Some("fr_FR"))
        let Some((base, rest)) = key.split_once('[') else {
            return (key, None);
        };
        let locale = rest.strip_suffix(']');
        match locale {
            Some(loc) if !loc.is_empty() => (base, Some(loc)),
            _ => (key, None),
        }
    }

    enum Section {
        None,
        DesktopEntry,
        DesktopAction(String),
        Other,
    }

    let locale_prefs = preferred_locales();

    let mut section = Section::None;

    let mut name = LocalizedField::default();
    let mut generic_name = LocalizedField::default();
    let mut comment = LocalizedField::default();
    let mut icon: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut try_exec: Option<String> = None;
    let mut terminal: bool = false;
    let mut categories: Vec<String> = Vec::new();
    let mut keywords = LocalizedField::default();
    let mut mime_types: Vec<String> = Vec::new();
    let mut actions_list: Vec<String> = Vec::new();
    let mut type_: Option<String> = None;
    let mut startup_wm_class: Option<String> = None;
    let mut startup_notify: Option<bool> = None;
    let mut nodisplay: Option<bool> = None;
    let mut hidden: Option<bool> = None;
    let mut only_show_in: Vec<String> = Vec::new();
    let mut not_show_in: Vec<String> = Vec::new();
    let mut extra: BTreeMap<String, String> = BTreeMap::new();

    type DesktopAction = (
        LocalizedField,
        Option<String>,
        Option<String>,
        BTreeMap<String, String>,
    );

    // Desktop actions keyed by action id
    let mut actions: BTreeMap<String, DesktopAction> = BTreeMap::new();

    for raw_line in data.lines() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            if line == "[Desktop Entry]" {
                section = Section::DesktopEntry;
            } else if let Some(rest) = line.strip_prefix("[Desktop Action ") {
                if let Some(action_id) = rest.strip_suffix(']') {
                    section = Section::DesktopAction(action_id.trim().to_string());
                    actions
                        .entry(action_id.trim().to_string())
                        .or_insert_with(|| {
                            (LocalizedField::default(), None, None, BTreeMap::new())
                        });
                } else {
                    section = Section::Other;
                }
            } else {
                section = Section::Other;
            }
            continue;
        }

        let Some((key_raw, value_raw)) = line.split_once('=') else {
            continue;
        };

        let key_raw = key_raw.trim();
        let value = value_raw.trim();
        if key_raw.is_empty() {
            continue;
        }

        let (key, locale) = split_key_locale(key_raw);

        match &mut section {
            Section::DesktopEntry => {
                match key {
                    "Name" => name.set(locale, value, &locale_prefs),
                    "GenericName" => generic_name.set(locale, value, &locale_prefs),
                    "Comment" => comment.set(locale, value, &locale_prefs),
                    "Icon" => {
                        if locale.is_none() {
                            icon = Some(value.to_string())
                        }
                    }
                    "Exec" => {
                        if locale.is_none() {
                            exec = Some(value.to_string())
                        }
                    }
                    "TryExec" => {
                        if locale.is_none() {
                            try_exec = Some(value.to_string())
                        }
                    }
                    "Terminal" => {
                        if locale.is_none() {
                            terminal = parse_bool(value).unwrap_or(false)
                        }
                    }
                    "Categories" => {
                        if locale.is_none() {
                            categories = split_list(value)
                        }
                    }
                    "Keywords" => keywords.set(locale, value, &locale_prefs),
                    "MimeType" => {
                        if locale.is_none() {
                            mime_types = split_list(value)
                        }
                    }
                    "Actions" => {
                        if locale.is_none() {
                            actions_list = split_list(value)
                        }
                    }
                    "Type" => {
                        if locale.is_none() {
                            type_ = Some(value.to_string())
                        }
                    }
                    "StartupWMClass" => {
                        if locale.is_none() {
                            startup_wm_class = Some(value.to_string())
                        }
                    }
                    "StartupNotify" => {
                        if locale.is_none() {
                            startup_notify = parse_bool(value)
                        }
                    }
                    "NoDisplay" => {
                        if locale.is_none() {
                            nodisplay = parse_bool(value)
                        }
                    }
                    "Hidden" => {
                        if locale.is_none() {
                            hidden = parse_bool(value)
                        }
                    }
                    "OnlyShowIn" => {
                        if locale.is_none() {
                            only_show_in = split_list(value)
                        }
                    }
                    "NotShowIn" => {
                        if locale.is_none() {
                            not_show_in = split_list(value)
                        }
                    }
                    _ => {
                        // Keep unknown keys only for base (non-localized) values to avoid huge maps.
                        if locale.is_none() {
                            extra.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            }

            Section::DesktopAction(action_id) => {
                let entry = actions
                    .entry(action_id.clone())
                    .or_insert_with(|| (LocalizedField::default(), None, None, BTreeMap::new()));

                match key {
                    "Name" => entry.0.set(locale, value, &locale_prefs),
                    "Icon" => {
                        if locale.is_none() {
                            entry.1 = Some(value.to_string());
                        }
                    }
                    "Exec" => {
                        if locale.is_none() {
                            entry.2 = Some(value.to_string());
                        }
                    }
                    _ => {
                        if locale.is_none() {
                            entry.3.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            }

            Section::None | Section::Other => {
                // Ignore keys outside known sections.
            }
        }
    }

    let resolved_keywords = keywords
        .resolve()
        .map(|s| split_list(&s))
        .unwrap_or_default();

    // Build actions vector; if Actions= exists, keep that order first, then others.
    let mut action_out: Vec<DesktopActionOut> = Vec::new();
    let mut seen = std::collections::BTreeSet::<String>::new();

    for aid in &actions_list {
        if let Some((lname, aicon, aexec, _extra)) = actions.get(aid) {
            action_out.push(DesktopActionOut {
                id: aid.clone(),
                name: lname.resolve(),
                icon: aicon.clone(),
                exec: aexec.clone(),
            });
            seen.insert(aid.clone());
        }
    }

    for (aid, (lname, aicon, aexec, _extra)) in &actions {
        if seen.contains(aid) {
            continue;
        }
        action_out.push(DesktopActionOut {
            id: aid.clone(),
            name: lname.resolve(),
            icon: aicon.clone(),
            exec: aexec.clone(),
        });
    }

    let out = DesktopEntryOut {
        id,
        name: name.resolve(),
        generic_name: generic_name.resolve(),
        comment: comment.resolve(),
        icon,
        exec,
        try_exec,
        terminal,
        categories,
        keywords: resolved_keywords,
        mime_types,
        actions: action_out,
        type_,
        startup_wm_class,
        startup_notify,
        nodisplay,
        hidden,
        only_show_in,
        not_show_in,
    };

    let id_lc = out.id.to_lowercase();
    let name_lc = out.name.as_deref().map(|s| s.to_lowercase());
    let norm = make_norm(&out);

    Some(DesktopEntryIndexed {
        out,
        norm,
        id_lc,
        name_lc,
    })
}

fn is_desktop_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("desktop"))
        .unwrap_or(false)
}

fn scan_desktop_paths(
    scan_roots: &[PathBuf],
    limit: Option<usize>,
) -> (usize, Vec<(PathBuf, PathBuf)>) {
    let mut found_count: usize = 0;
    let mut paths: Vec<(PathBuf, PathBuf)> = Vec::new();

    for root in scan_roots {
        if !root.is_dir() {
            continue;
        }

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            if is_desktop_file(path) {
                found_count += 1;

                // Limit only the returned list (useful for `scan --limit`),
                // but keep counting the total number of matches.
                if limit.map(|limit| paths.len() < limit).unwrap_or(true) {
                    paths.push((root.clone(), path.to_path_buf()));
                }
            }
        }
    }

    (found_count, paths)
}

fn compute_desktop_id(applications_root: &Path, desktop_path: &Path) -> String {
    // Per Desktop Entry spec:
    // desktop file id = relative path under "applications" with '/' replaced by '-'
    // and without the ".desktop" suffix.
    let rel = desktop_path
        .strip_prefix(applications_root)
        .unwrap_or(desktop_path);

    let mut s = rel.to_string_lossy().to_string();
    if let Some(stripped) = s.strip_suffix(".desktop") {
        s = stripped.to_string();
    }

    s = s.replace('/', "-");
    s
}

fn make_norm(out: &DesktopEntryOut) -> String {
    let mut s = String::new();

    push_norm(&mut s, Some(&out.id));
    push_norm(&mut s, out.name.as_deref());
    push_norm(&mut s, out.generic_name.as_deref());
    push_norm(&mut s, out.comment.as_deref());
    push_norm(&mut s, out.exec.as_deref());
    push_norm(&mut s, out.try_exec.as_deref());
    push_norm(&mut s, out.icon.as_deref());

    for c in &out.categories {
        push_norm(&mut s, Some(c));
    }
    for k in &out.keywords {
        push_norm(&mut s, Some(k));
    }
    for m in &out.mime_types {
        push_norm(&mut s, Some(m));
    }
    for a in &out.actions {
        push_norm(&mut s, Some(&a.id));
        push_norm(&mut s, a.name.as_deref());
        push_norm(&mut s, a.exec.as_deref());
        push_norm(&mut s, a.icon.as_deref());
    }

    push_norm(&mut s, out.type_.as_deref());
    push_norm(&mut s, out.startup_wm_class.as_deref());

    s
}

fn push_norm(dst: &mut String, v: Option<&str>) {
    let Some(x) = v else {
        return;
    };

    if !dst.is_empty() {
        dst.push(' ');
    }

    // Avoid allocating a temporary lowercased String.
    for ch in x.chars() {
        for lc in ch.to_lowercase() {
            dst.push(lc);
        }
    }
}
