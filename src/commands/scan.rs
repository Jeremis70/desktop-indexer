use crate::desktop::{scan_and_parse_desktop_files, scan_desktop_files};
use crate::models::DesktopEntryOut;
use crate::output::print_json;

pub fn scan(
    scan_roots: &[std::path::PathBuf],
    limit: Option<usize>,
    parse: bool,
    json: bool,
    respect_try_exec: bool,
) -> i32 {
    if parse {
        let result = scan_and_parse_desktop_files(scan_roots, limit, respect_try_exec);

        if json {
            let entries: Vec<DesktopEntryOut> =
                result.entries.iter().map(|e| e.out.clone()).collect();

            #[derive(serde::Serialize)]
            struct ScanParseOut {
                scanned_roots: Vec<String>,
                found_count: usize,
                parsed_count: usize,
                parse_failed: usize,
                entries: Vec<DesktopEntryOut>,
            }

            let out = ScanParseOut {
                scanned_roots: result.scanned_roots,
                found_count: result.found_count,
                parsed_count: result.parsed_count,
                parse_failed: result.parse_failed,
                entries,
            };

            print_json(&out);
        } else {
            println!("roots:");
            for r in &result.scanned_roots {
                println!("  {r}");
            }
            println!("found_count={}", result.found_count);
            println!("parsed_count={}", result.parsed_count);
            println!("parse_failed={}", result.parse_failed);
            for e in &result.entries {
                let name = e.out.name.as_deref().unwrap_or("");
                if name.is_empty() {
                    println!("{}", e.out.id);
                } else {
                    println!("{}\t{}", e.out.id, name);
                }
            }
        }
        return 0;
    }

    let result = scan_desktop_files(scan_roots, limit);
    if json {
        print_json(&result);
    } else {
        println!("roots:");
        for r in &result.scanned_roots {
            println!("  {r}");
        }
        println!("found_count={}", result.found_count);
        println!("showing={}", result.files.len());
        for f in &result.files {
            println!("{f}");
        }
    }

    0
}
