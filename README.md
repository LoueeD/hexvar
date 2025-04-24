# hexvar

A CLI tool to scan your codebase for hex color codes, deduplicate and optimize them, and generate CSS custom properties (variables) for easy maintainability and refactoring.

## Features

- **Scans** CSS, SCSS, SASS, Vue, Astro, and Svelte files for hex color codes.
- **Deduplicates** visually similar colors using LAB color clustering (Delta E).
- **Outputs**:
  - `colours.css`: Canonical CSS custom properties for all deduplicated colors.
  - `colours_map.json`: Mapping of all original hex codes to their canonical CSS variable for safe refactoring.
  - `colours.json`: Raw count of all hex codes found (for stats/auditing).
- **Readable variable names**: Uses CSS color names where possible (e.g. `--color-tomato`), otherwise falls back to hex.
- **CLI summary**: Prints a report on how many colors were optimized.

## Installation

1. Install Rust: https://rustup.rs/
2. Clone this repo:
   ```sh
   git clone <this-repo-url>
   cd stylebang
   ```
3. Build:
   ```sh
   cargo build --release
   ```
4. Run:
   ```sh
   cargo run -- scan "<glob>" --out colours.json --css-vars colours.css
   ```
   Example:
   ```sh
   cargo run -- scan "src/**/*" --out colours.json --css-vars colours.css
   ```

## Usage

```
hexvar scan <glob> [--out <json>] [--css-vars <css>]
```

- `<glob>`: Glob pattern(s) to scan (e.g. `src/**/*.css`)
- `--out <json>`: Output JSON file with hex code counts (default: stdout)
- `--css-vars <css>`: Output CSS file with deduplicated variables

---

### Replace Command

```
hexvar replace <glob> [--ignore <pattern>]
```

- `<glob>`: Glob pattern(s) for files in which to replace hex codes (e.g. `src/**/*.css`)
- `--ignore <pattern>`: Patterns or directories to ignore (e.g. `node_modules`)

This command will replace all hex color codes in the matched files with their corresponding CSS custom properties (variables) as defined in `colours.css`. The mapping is determined by `colours_map.json`.

**Warning:** The replace command is destructive—it will overwrite files in-place. Make sure you are using version control (e.g., git) and commit your changes before running this command to avoid accidental data loss.

---

### Example Output

**colours.css**
```css
:root {
  --color-tomato: #ff6347;
  --color-gray: #888888;
  /* ... */
}
```

**colours_map.json**
```json
{
  "#ff6347": ["#ff6347", "#ff6350", "#ff6348"],
  "#888888": ["#888888", "#878787"]
}
```

## How It Works

- Finds all hex codes in your codebase.
- Groups visually similar colors (Delta E < 10 in LAB space) into a single canonical color.
- Outputs CSS variables for each canonical color.
- Outputs a mapping of all merged hex codes for safe refactoring.

## Why?

- **Refactor safely**: Replace all color codes in your codebase with canonical CSS variables.
- **Reduce duplication**: Avoid hundreds of nearly-identical color variables.
- **Improve maintainability**: Use readable, consistent variable names.

## License
MIT
