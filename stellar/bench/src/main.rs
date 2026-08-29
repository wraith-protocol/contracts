use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use wraith_stellar_bench::{collect_rows, print_markdown, to_json, BenchMeta};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut format = "markdown".to_string();
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => {
                i += 1;
                format = args
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| usage_exit("--format requires a value"));
            }
            "--out" => {
                i += 1;
                out_path = Some(resolve_out_path(args.get(i).cloned().unwrap_or_else(
                    || {
                        usage_exit("--out requires a path");
                    },
                )));
            }
            "-h" | "--help" => usage_exit(""),
            other => usage_exit(&format!("unknown argument: {other}")),
        }
        i += 1;
    }

    let rows = collect_rows();
    let meta = BenchMeta::default();

    let rendered = match format.as_str() {
        "markdown" | "md" => wraith_stellar_bench::markdown_table(&rows),
        "json" => to_json(&rows, &meta),
        other => usage_exit(&format!("unsupported --format: {other}")),
    };

    if let Some(path) = out_path {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).unwrap_or_else(|e| {
                    eprintln!("failed to create {}: {e}", parent.display());
                    process::exit(1);
                });
            }
        }
        fs::write(&path, &rendered).unwrap_or_else(|e| {
            eprintln!("failed to write {}: {e}", path.display());
            process::exit(1);
        });
        eprintln!("wrote {}", path.display());
    }

    // Always print to stdout for CI logs / PERF.md piping.
    if format == "markdown" || format == "md" {
        print_markdown(&rows);
    } else {
        print!("{rendered}");
    }
}

/// Resolve `--out` against the package directory so CI always writes
/// `stellar/bench/results.json` regardless of the process CWD.
fn resolve_out_path(raw: String) -> PathBuf {
    let path = PathBuf::from(&raw);
    if path.is_absolute() {
        return path;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn usage_exit(msg: &str) -> ! {
    if !msg.is_empty() {
        eprintln!("error: {msg}");
    }
    eprintln!(
        "usage: wraith-stellar-bench [--format markdown|json] [--out path]\n\
         \n\
         Measures per-op Soroban resources. JSON output is used for CI baselines.\n\
         Relative --out paths are resolved against the bench package directory."
    );
    process::exit(if msg.is_empty() { 0 } else { 2 });
}
