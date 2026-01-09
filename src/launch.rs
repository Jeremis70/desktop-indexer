use std::{env, path::Path};

#[derive(Debug, Clone, Copy)]
pub enum Terminal {
    Foot,
    Kitty,
    Alacritty,
    WezTerm,
}

pub fn pick_terminal() -> Option<Terminal> {
    // Keep this deterministic and simple.
    if is_executable_in_path("foot") {
        return Some(Terminal::Foot);
    }
    if is_executable_in_path("kitty") {
        return Some(Terminal::Kitty);
    }
    if is_executable_in_path("alacritty") {
        return Some(Terminal::Alacritty);
    }
    if is_executable_in_path("wezterm") {
        return Some(Terminal::WezTerm);
    }

    None
}

pub fn exec_to_argv(exec_line: &str) -> Vec<String> {
    // Desktop Entry spec allows field codes like %u, %U, %f, %F, etc.
    // For now we drop them (we're launching without file/url args).
    let Some(tokens) = shlex::split(exec_line) else {
        return Vec::new();
    };

    tokens
        .into_iter()
        .filter_map(|t| {
            // Remove known field codes
            if is_field_code_token(&t) {
                return None;
            }

            // Best-effort: strip field codes embedded in an arg
            // Example: "--foo=%u" -> "--foo="
            if t.contains('%') {
                return Some(strip_field_codes(&t));
            }

            Some(t)
        })
        .filter(|t| !t.is_empty())
        .collect()
}

fn is_field_code_token(t: &str) -> bool {
    matches!(
        t,
        "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%i" | "%c" | "%k" | "%v" | "%m"
    )
}

fn strip_field_codes(s: &str) -> String {
    // Minimal: remove any occurrences of %<char>.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            // Skip next char if present (the code), or keep '%' if it's the end.
            if chars.peek().is_some() {
                chars.next();
                continue;
            }
        }
        out.push(ch);
    }

    out
}

fn is_executable_in_path(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let Some(path_os) = env::var_os("PATH") else {
        return false;
    };

    for dir in env::split_paths(&path_os) {
        if dir.as_os_str().is_empty() {
            continue;
        }

        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return true;
        }
    }

    false
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };

    if !meta.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        mode & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        // Best-effort for non-unix.
        true
    }
}
