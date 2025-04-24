use clap::{Parser, Subcommand};
use glob::glob;

use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use indicatif::{ProgressBar, ProgressStyle};
mod css_color_names;

/// Scan CSS/SCSS files for unique Hex colors and output JSON report
#[derive(Subcommand)]
enum Commands {
    /// Scan and report hex colors (existing logic)
    Scan {
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
    },
    /// Replace hex codes in files with CSS variables using colours_map.json
    Replace {
        /// Glob patterns to include (e.g., "src/**/*.css")
        #[arg(value_name = "GLOB", required = true)]
        patterns: Vec<String>,
        /// Glob patterns or directories to ignore
        #[arg(short, long, value_name = "IGNORE")]
        ignore: Vec<String>,
    },
}

#[derive(Parser)]
#[command(name = "hexvar")]
#[command(about = "Scan stylesheets for hex colors", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Serialize)]
struct ColorReport(HashMap<String, u32>);

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Scan { patterns, css_vars, out, ignore } => {
            // Regex to match 8, 6, or 3 digit hex codes (longest first, not 4)
            let re = Regex::new(r"#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3})").unwrap();

            // Collect all file paths matching patterns (ignoring ignores)
            let mut paths: Vec<PathBuf> = Vec::new();
            // Default file extensions to scan
            let default_exts = ["css", "scss", "sass", "vue", "astro", "svelte"];
            let _use_default_exts = patterns.len() == 1 && patterns[0] == "/**/*";

            for pat in patterns {
                for entry in glob(pat).expect("Invalid glob pattern") {
                    if let Ok(path) = entry {
                        // skip if matches any ignore pattern
                        if ignore.iter().any(|ig| path.to_string_lossy().contains(ig)) {
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
            if let Some(css_path) = &css_vars {
                use std::io::Write;
                use palette::{Srgb, Lab, FromColor};
                use palette::color_difference::DeltaE;
                let mut css = String::from(":root {\n");
                let delta_e_threshold = 10.0;
                let mut clusters: Vec<(String, Lab)> = Vec::new(); // (canonical hex, Lab)
                let mut hex_to_canonical: std::collections::HashMap<String, String> = std::collections::HashMap::new();
                // Precompute LAB for all hexes
                let mut hex_lab: std::collections::HashMap<&String, Lab> = std::collections::HashMap::new();
                for hex in counts.keys() {
                    let rgb = hex.trim_start_matches('#');
                    let (r, g, b) = match rgb.len() {
                        3 => {
                            let r = u8::from_str_radix(&rgb[0..1].repeat(2), 16).unwrap_or(0);
                            let g = u8::from_str_radix(&rgb[1..2].repeat(2), 16).unwrap_or(0);
                            let b = u8::from_str_radix(&rgb[2..3].repeat(2), 16).unwrap_or(0);
                            (r, g, b)
                        },
                        6 => {
                            let r = u8::from_str_radix(&rgb[0..2], 16).unwrap_or(0);
                            let g = u8::from_str_radix(&rgb[2..4], 16).unwrap_or(0);
                            let b = u8::from_str_radix(&rgb[4..6], 16).unwrap_or(0);
                            (r, g, b)
                        },
                        _ => { continue; }
                    };
                    let lab: Lab = Lab::from_color(Srgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0));
                    hex_lab.insert(hex, lab);
                }
                // Clustering
                for hex in counts.keys() {
                    if let Some(&lab) = hex_lab.get(hex) {
                        let mut canonical: Option<String> = None;
                        for (canon_hex, canon_lab) in &clusters {
                            if lab.delta_e(*canon_lab) < delta_e_threshold {
                                canonical = Some(canon_hex.clone());
                                break;
                            }
                        }
                        if let Some(canon_hex) = canonical {
                            hex_to_canonical.insert(hex.clone(), canon_hex);
                        } else {
                            clusters.push((hex.clone(), lab));
                            hex_to_canonical.insert(hex.clone(), hex.clone());
                        }
                    }
                }
                // Output CSS vars for canonical colors only
                for (canon_hex, _) in &clusters {
                    // Try to find a CSS color name for this hex
                    let mut var = None;
                    for (name, css_hex) in css_color_names::CSS_COLOR_NAMES.iter() {
                        if css_hex.eq_ignore_ascii_case(canon_hex) {
                            var = Some(format!("--color-{}", name.replace('_', "-")));
                            break;
                        }
                    }
                    // If no exact match, find closest CSS color by Euclidean RGB distance
                    let var = var.unwrap_or_else(|| {
                        fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
                            let hex = hex.trim_start_matches('#');
                            match hex.len() {
                                3 => {
                                    let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                                    let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                                    let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                                    Some((r, g, b))
                                }
                                6 => {
                                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                                    Some((r, g, b))
                                }
                                _ => None
                            }
                        }
                        let (r, g, b) = match hex_to_rgb(canon_hex) {
                            Some(rgb) => rgb,
                            None => return format!("--color-{}", canon_hex.trim_start_matches('#').to_lowercase()),
                        };
                        let mut min_dist = u32::MAX;
                        let mut closest = None;
                        for (name, css_hex) in css_color_names::CSS_COLOR_NAMES.iter() {
                            if let Some((cr, cg, cb)) = hex_to_rgb(css_hex) {
                                let dist = (r as i32 - cr as i32).pow(2) as u32
                                 + (g as i32 - cg as i32).pow(2) as u32
                                 + (b as i32 - cb as i32).pow(2) as u32;
                                if dist < min_dist {
                                    min_dist = dist;
                                    closest = Some(name);
                                }
                            }
                        }
                        if let Some(name) = closest {
                            format!("--color-{}", name.replace('_', "-"))
                        } else {
                            format!("--color-{}", canon_hex.trim_start_matches('#').to_lowercase())
                        }
                    });
                    css.push_str(&format!("    {}: {};", var, canon_hex));
                    css.push('\n');
                }
                css.push_str("}\n");
                // Build canonical_map for reporting
                let mut canonical_map: std::collections::HashMap<&String, Vec<&String>> = std::collections::HashMap::new();
                for (hex, canon) in &hex_to_canonical {
                    canonical_map.entry(canon).or_default().push(hex);
                }
                // Output the mapping of canonical hex -> all merged hexes
                let map_path = "colours_map.json";
                match std::fs::File::create(map_path) {
                    Ok(mut file) => {
                        if let Err(e) = serde_json::to_writer_pretty(&mut file, &canonical_map) {
                            eprintln!("Failed to write mapping file {}: {}", map_path, e);
                        } else {
                            println!("Wrote canonical color mapping to {}", map_path);
                        }
                    }
                    Err(e) => eprintln!("Failed to create mapping file {}: {}", map_path, e),
                }
                // CLI output about optimization
                let unique_hexes = counts.len();
                let canonical_count = canonical_map.len();
                println!(
                    "Optimization: Reduced {unique_hexes} unique hex codes to {canonical_count} canonical CSS variables using perceptual color clustering (Delta E < {delta_e}).\nSee colours_map.json for mappings.",
                    unique_hexes = unique_hexes,
                    canonical_count = canonical_count,
                    delta_e = delta_e_threshold
                );
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
            match out {
                Some(ref out_path) => {
                    if let Err(e) = std::fs::write(out_path, json) {
                        eprintln!("Failed to write output file {}: {}", out_path, e);
                        std::process::exit(1);
                    }
                }
                None => {
                    println!("{}", json);
                }
            }
        }
        Commands::Replace { patterns, ignore } => {
            use std::collections::HashMap;
            use std::fs;
            use glob::glob;
            use regex::Regex;
            // Load mapping
            let map: HashMap<String, Vec<String>> = match fs::read_to_string("colours_map.json") {
                Ok(s) => serde_json::from_str(&s).expect("Invalid colours_map.json"),
                Err(e) => {
                    eprintln!("Failed to read colours_map.json: {}", e);
                    std::process::exit(1);
                }
            };
            // Parse colours.css for valid vars
            let css = fs::read_to_string("colours.css").expect("Could not read colours.css");
            let mut canon_to_var = HashMap::new();
            for line in css.lines() {
                if let Some((var, hex)) = line.trim().strip_prefix("--color-").and_then(|rest| rest.split_once(":")) {
                    let var_name = format!("--color-{}", var.trim());
                    let hex_val = hex.trim().trim_end_matches(';').to_lowercase();
                    canon_to_var.insert(hex_val.clone(), var_name);
                }
            }
            // Build hex->var map strictly from colours.css
            let mut hex_to_var = HashMap::new();
            for (canon, hexes) in &map {
                let canon_hex = canon.trim_start_matches('#').to_lowercase();
                let css_hex = format!("#{}", canon_hex);
                let var = match canon_to_var.get(&css_hex) {
                    Some(v) => v.clone(),
                    None => {
                        eprintln!("No variable name found in colours.css for canonical hex {}", canon);
                        std::process::exit(1);
                    }
                };
                for h in hexes {
                    hex_to_var.insert(h.to_lowercase(), var.clone());
                }
            }
            // For each file matching glob
            let mut total_replacements = 0;
            let mut files_changed = 0;
            let exts = ["css", "scss", "sass", "vue", "astro", "svelte"];
            for pat in patterns {
                for entry in glob(pat).expect("Invalid glob pattern") {
                    if let Ok(path) = entry {
                        // skip if matches any ignore pattern
                        if ignore.iter().any(|ig| path.to_string_lossy().contains(ig)) {
                            continue;
                        }
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            if !exts.contains(&ext) {
                                continue;
                            }
                        } else {
                            continue;
                        }
                        let content = match fs::read_to_string(&path) {
                            Ok(c) => c,
                            Err(_) => continue,
                        };
                        let mut replaced = content.clone();
                        let mut file_replacements = 0;
                        for (hex, var) in &hex_to_var {
                            // Regex for hex (case-insensitive, with or without #)
                            let re = Regex::new(&format!(r"(?i){}", regex::escape(hex))).unwrap();
                            let new_replaced = re.replace_all(&replaced, format!("var({})", var));
                            let count = new_replaced.matches(&format!("var({})", var)).count();
                            if count > replaced.matches(hex).count() {
                                file_replacements += count;
                            }
                            replaced = new_replaced.into_owned();
                        }
                        if file_replacements > 0 && replaced != content {
                            fs::write(&path, replaced).expect("Failed to write file");
                            files_changed += 1;
                            total_replacements += file_replacements;
                            println!("Replaced {} hex codes in {}", file_replacements, path.display());
                        }
                    }
                }
            }
            println!("Total replacements: {} in {} files", total_replacements, files_changed);
        }
    }
}
