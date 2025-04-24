use clap::Parser;
use glob::glob;

use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use indicatif::{ProgressBar, ProgressStyle};

/// Scan CSS/SCSS files for unique Hex colors and output JSON report
#[derive(Parser)]
#[command(name = "hexvar")]
#[command(about = "Scan stylesheets for hex colors", long_about = None)]
struct Cli {
    /// Output CSS file with variables for each hex code
    #[arg(long, value_name = "FILE")]
    css_vars: Option<String>,
    /// Glob patterns to include (e.g., "src/**/*.css")
    #[arg(value_name = "GLOB", required = true)]
    patterns: Vec<String>,

    /// Glob patterns or directories to ignore
    #[arg(short, long, value_name = "IGNORE")]
    ignore: Vec<String>,

    /// Output file for JSON report (default: stdout)
    #[arg(short, long, value_name = "FILE")]
    out: Option<String>,
}

#[derive(Serialize)]
struct ColorReport(HashMap<String, u32>);

fn main() {
    let cli = Cli::parse();
    // Regex to match 8, 6, or 3 digit hex codes (longest first, not 4)
    let re = Regex::new(r"#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3})").unwrap();

    // Collect all file paths matching patterns (ignoring ignores)
    let mut paths: Vec<PathBuf> = Vec::new();
    // Default file extensions to scan
    let default_exts = ["css", "scss", "sass", "vue", "astro", "svelte"];
    let _use_default_exts = cli.patterns.len() == 1 && cli.patterns[0] == "/**/*";

    for pat in &cli.patterns {
        for entry in glob(pat).expect("Invalid glob pattern") {
            if let Ok(path) = entry {
                // skip if matches any ignore pattern
                if cli.ignore.iter().any(|ig| path.to_string_lossy().contains(ig)) {
                    continue;
                }
                // Always ignore anything in common output directories
                const OUTPUT_DIRS: &[&str] = &[
                    "node_modules", "dist", "build", "out", ".next", ".vercel", ".cache", "coverage", "target"
                ];
                if path.components().any(|c| {
                    let s = c.as_os_str().to_string_lossy();
                    OUTPUT_DIRS.contains(&s.as_ref())
                }) {
                    continue;
                }
                // Only include files with allowed extensions
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if !default_exts.contains(&ext) {
                        continue;
                    }
                } else {
                    continue;
                }
                paths.push(path);
            }
        }
    }
    // Set up progress bar
    let file_count = paths.len();
    let pb = ProgressBar::new(file_count as u64);
    pb.set_style(ProgressStyle::with_template("{spinner} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
        .unwrap()
        .progress_chars("|/-\\ "));

    // Scan files and count hex codes (sequential, for progress bar UX)
    let mut counts: HashMap<String, u32> = HashMap::new();
    for path in &paths {
        let fullpath = path.display().to_string();
        pb.set_message(fullpath);
        pb.inc(1);
        if let Ok(content) = fs::read_to_string(path) {
            for m in re.find_iter(&content) {
                let hex = m.as_str().to_string();
                *counts.entry(hex).or_insert(0) += 1;
            }
        }
    }
    pb.finish_and_clear();

    let total: u32 = counts.values().sum();
    let unique = counts.len();

    println!("\n==== HEXVAR SUMMARY ====");
    if unique == 0 {
        println!("No hex codes found in {} files.", file_count);
    } else {
        println!("Files scanned:      {}", file_count);
        println!("Unique hex codes:   {}", unique);
        println!("Total occurrences:  {}", total);
    }
    println!("=======================\n");

    // If requested, generate CSS variables file
    if let Some(css_path) = &cli.css_vars {
        use std::io::Write;
        let mut css = String::from(":root {\n");
        for hex in counts.keys() {
            let var = format!("--color-{}", hex.trim_start_matches('#').to_lowercase());
            css.push_str(&format!("    {}: {};", var, hex));
            css.push('\n');
        }
        css.push_str("}\n");
        match std::fs::File::create(css_path) {
            Ok(mut file) => {
                if let Err(e) = file.write_all(css.as_bytes()) {
                    eprintln!("Failed to write CSS vars file {}: {}", css_path, e);
                } else {
                    println!("Wrote CSS variables to {}", css_path);
                }
            }
            Err(e) => eprintln!("Failed to create CSS vars file {}: {}", css_path, e),
        }
    }

    // Output JSON to file or stdout
    let report = ColorReport(counts.clone());
    let json = serde_json::to_string_pretty(&report).unwrap();
    match cli.out {
        Some(ref out_path) => {
            if let Err(e) = std::fs::write(out_path, json) {
                eprintln!("Failed to write output file {}: {}", out_path, e);
                std::process::exit(1);
            }
        },
        None => {
            println!("{}", json);
        }
    }

}
