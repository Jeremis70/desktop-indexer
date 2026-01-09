use crate::empty_query::EmptyQueryMode;
use crate::frequency::Usage;
use crate::models::{DesktopEntryIndexed, DesktopEntryOut};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{cmp::Reverse, collections::BinaryHeap};

pub fn normalize_query(query: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();

    let mut buf = String::new();
    for ch in query.trim().chars() {
        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                buf.push(lc);
            }
        } else if !buf.is_empty() {
            tokens.push(std::mem::take(&mut buf));
        }
    }
    if !buf.is_empty() {
        tokens.push(buf);
    }

    // Most selective first => fail faster.
    tokens.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    tokens.dedup();

    tokens
}

pub fn norm_has_token_prefix(norm: &str, token: &str) -> bool {
    if token.is_empty() {
        return true;
    }

    if norm.starts_with(token) {
        return true;
    }

    let bytes = norm.as_bytes();
    for (idx, _) in norm.match_indices(token) {
        if idx > 0 && bytes[idx - 1] == b' ' {
            return true;
        }
    }

    false
}

pub fn search_entries_with_usage_map_and_empty_mode(
    entries: &[DesktopEntryIndexed],
    query: &str,
    limit: usize,
    usage: &HashMap<String, Usage>,
    empty_mode: EmptyQueryMode,
) -> Vec<DesktopEntryOut> {
    if limit == 0 {
        return Vec::new();
    }

    let tokens = normalize_query(query);
    if tokens.is_empty() {
        return empty_query_entries(entries, limit, usage, empty_mode);
    }

    // Keep only top-K scored candidates.
    let mut heap: BinaryHeap<Reverse<(i32, usize)>> = BinaryHeap::new();

    let now_sec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    'outer: for (idx, e) in entries.iter().enumerate() {
        for t in &tokens {
            if !norm_has_token_prefix(&e.norm, t) {
                continue 'outer;
            }
        }

        let u = usage.get(&e.out.id).copied().unwrap_or_default();
        let score = score_entry(e, &tokens, u, now_sec);

        heap.push(Reverse((score, idx)));
        if heap.len() > limit {
            heap.pop();
        }
    }

    // heap is min-heap via Reverse; drain then sort by score desc.
    let mut picked: Vec<(i32, usize)> = heap.into_iter().map(|Reverse(x)| x).collect();
    picked.sort_by(|a, b| b.0.cmp(&a.0));

    picked
        .into_iter()
        .map(|(_, idx)| entries[idx].out.clone())
        .collect()
}

fn empty_query_entries(
    entries: &[DesktopEntryIndexed],
    limit: usize,
    usage: &HashMap<String, Usage>,
    empty_mode: EmptyQueryMode,
) -> Vec<DesktopEntryOut> {
    let mut picked: Vec<(usize, Usage)> = entries
        .iter()
        .enumerate()
        .filter_map(|(idx, e)| usage.get(&e.out.id).copied().map(|u| (idx, u)))
        .filter(|(_idx, u)| match empty_mode {
            EmptyQueryMode::Recency => u.last_used != 0,
            EmptyQueryMode::Frequency => u.freq != 0,
        })
        .collect();

    picked.sort_by(|(a_idx, a_u), (b_idx, b_u)| match empty_mode {
        EmptyQueryMode::Recency => b_u
            .last_used
            .cmp(&a_u.last_used)
            .then_with(|| b_u.freq.cmp(&a_u.freq))
            .then_with(|| {
                let a_name = entries[*a_idx].out.name.as_deref().unwrap_or("");
                let b_name = entries[*b_idx].out.name.as_deref().unwrap_or("");
                a_name.cmp(b_name)
            })
            .then_with(|| entries[*a_idx].out.id.cmp(&entries[*b_idx].out.id)),
        EmptyQueryMode::Frequency => (b_u.freq)
            .cmp(&a_u.freq)
            .then_with(|| b_u.last_used.cmp(&a_u.last_used))
            .then_with(|| {
                let a_name = entries[*a_idx].out.name.as_deref().unwrap_or("");
                let b_name = entries[*b_idx].out.name.as_deref().unwrap_or("");
                a_name.cmp(b_name)
            })
            .then_with(|| entries[*a_idx].out.id.cmp(&entries[*b_idx].out.id)),
    });

    picked
        .into_iter()
        .take(limit)
        .map(|(idx, _)| entries[idx].out.clone())
        .collect()
}

pub fn score_entry(e: &DesktopEntryIndexed, tokens: &[String], usage: Usage, now_sec: u64) -> i32 {
    let mut score: i32 = 10;

    // Frequency boost (bounded so it doesn't dominate relevance).
    // 0..20 => +0..80, 20+ saturates.
    score += (usage.freq.min(20) as i32) * 4;

    // Recency boost (bounded).
    // The goal is to break ties between similarly relevant results.
    score += recency_bonus(usage.last_used, now_sec);

    // Name boosts (avoid per-candidate allocations)
    if let Some(name_lc) = e.name_lc.as_deref() {
        let all_in_name = tokens.iter().all(|t| name_lc.contains(t));
        if all_in_name {
            score += 40;
        }

        if let Some(first) = tokens.first()
            && name_lc.starts_with(first)
        {
            score += 30;
        }
    }

    // id boost
    if let Some(first) = tokens.first()
        && e.id_lc.contains(first)
    {
        score += 15;
    }

    score
}

fn recency_bonus(last_used: u64, now_sec: u64) -> i32 {
    if last_used == 0 || now_sec == 0 {
        return 0;
    }

    let age = now_sec.saturating_sub(last_used);

    // Simple step function, avoids floats and is easy to reason about.
    const HOUR: u64 = 60 * 60;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    const MONTH: u64 = 30 * DAY;

    if age < HOUR {
        30
    } else if age < DAY {
        20
    } else if age < WEEK {
        10
    } else if age < MONTH {
        5
    } else {
        0
    }
}
