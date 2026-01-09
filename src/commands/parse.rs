use crate::desktop::parse_desktop_file_using_roots;
use crate::output::print_json;
use std::path::Path;

pub fn parse(scan_roots: &[std::path::PathBuf], path: &Path, json: bool) -> i32 {
    let Some(entry) = parse_desktop_file_using_roots(path, scan_roots) else {
        eprintln!("Failed to parse {}", path.display());
        return 1;
    };

    if json {
        print_json(&entry.out);
    } else {
        println!("{:#?}", entry.out);
        eprintln!("norm={}", entry.norm);
    }

    0
}
