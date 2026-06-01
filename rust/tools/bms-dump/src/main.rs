//! bms-dump — CLI tool to parse a BMS file and dump the result as JSON.
//!
//! This is the Rust-side counterpart to Phase 0's "legacy C++ dump tool".
//! Usage: `bms-dump <path-to-bms>`
//!
//! Output format is designed to match `fixtures/golden/bms/*.json`.

use openlr2_bms::{BmsParseOptions, Chart};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: bms-dump <path-to-bms>");
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    if !path.exists() {
        eprintln!("Error: file not found: {}", path.display());
        std::process::exit(1);
    }

    let options = BmsParseOptions::default();
    match Chart::from_file(&path, options) {
        Ok(chart) => {
            let json = serde_json::to_string_pretty(&chart).unwrap_or_else(|e| {
                eprintln!("Error serializing chart: {}", e);
                std::process::exit(1);
            });
            println!("{}", json);
        }
        Err(e) => {
            eprintln!("Error parsing BMS: {}", e);
            std::process::exit(1);
        }
    }
}
