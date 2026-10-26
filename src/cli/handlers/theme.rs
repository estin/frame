//! `fr theme` — list theme files and import palettes from other collections.

use std::collections::BTreeMap;
use std::fs;

use serde::Serialize;
use serde_json::json;

use crate::cli::commands::{ThemeAction, ThemeCmd, ThemeImportArgs, ThemeImportSource};
use crate::io::dryrun;
use crate::tui::theme::{BUILTIN_THEMES, DEFAULT_THEME_NAME, ThemeDirs};

pub fn cmd_theme(args: ThemeCmd, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    match args.action {
        None | Some(ThemeAction::List) => cmd_theme_list(json),
        Some(ThemeAction::Import(a)) => cmd_theme_import(a, json),
    }
}

fn cmd_theme_list(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let project = super::load_project_cwd()?;
    let dirs = ThemeDirs::for_project(&project.frame_dir);
    let ui = &project.config.ui;
    let active = ui.theme.as_deref().unwrap_or(DEFAULT_THEME_NAME);

    #[derive(Serialize)]
    struct Row {
        name: String,
        source: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        active: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    }

    let mut rows: Vec<Row> = Vec::new();

    // File themes first, so a file shadowing a built-in takes its slot.
    for name in dirs.names() {
        let source = if dirs.project.join(format!("{name}.toml")).is_file() {
            "project"
        } else {
            "global"
        };
        let path = dirs.find(&name).map(|path| path.display().to_string());
        let error = match dirs.load_checked(&name) {
            Ok(_) => None,
            Err(err) => Some(err.to_string()),
        };
        rows.push(Row {
            name: name.clone(),
            source: source.to_string(),
            path,
            active: name == active,
            error,
        });
    }

    // Built-ins that are not shadowed by a file.
    for name in BUILTIN_THEMES {
        if rows.iter().any(|row| row.name == *name) {
            continue;
        }
        rows.insert(
            0,
            Row {
                name: (*name).to_string(),
                source: "built-in".to_string(),
                path: None,
                active: *name == active,
                error: None,
            },
        );
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "active": active,
                "themes": rows,
            }))?
        );
        return Ok(());
    }

    if rows.is_empty() {
        println!("No themes found.");
        return Ok(());
    }

    let name_w = rows
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(4)
        .max(4);
    for row in &rows {
        let marker = if row.active { "*" } else { " " };
        let path = row.path.clone().unwrap_or_default();
        println!(
            "{} {:<width$}  {:<8}  {}",
            marker,
            row.name,
            row.source,
            path,
            width = name_w
        );
        if let Some(error) = &row.error {
            println!("    invalid: {error}");
        }
    }

    if !rows.iter().any(|row| row.active) {
        println!();
        println!(
            "theme = {:?} is not available; the default palette is in use.",
            active
        );
    }
    Ok(())
}

fn cmd_theme_import(args: ThemeImportArgs, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    dryrun::arm(args.dry_run);

    let text = fs::read_to_string(&args.file)
        .map_err(|err| format!("cannot read {}: {err}", args.file))?;

    let scheme = match args.source {
        ThemeImportSource::Base16 => Base16Scheme::parse(&text)?,
    };

    let name = args
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let name = match name {
        Some(value) => value,
        None => {
            if scheme.name.trim().is_empty() {
                std::path::Path::new(&args.file)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("")
            } else {
                scheme.name.as_str()
            }
        }
    };
    let slug = slugify(name).ok_or_else(|| "theme name is empty; pass --name".to_string())?;

    let dir = if args.global {
        ThemeDirs::global()
    } else {
        super::load_project_cwd()?.frame_dir.join("themes")
    };

    let path = dir.join(format!("{slug}.toml"));

    if path.exists() && !args.force {
        return Err(format!(
            "theme {slug:?} already exists at {}; pass --force to overwrite",
            path.display()
        )
        .into());
    }

    let body = scheme.to_toml(&slug);

    dryrun::create_dir_all(&dir)?;
    dryrun::write(&path, &body)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&super::with_dry_run(json!({
                "name": slug,
                "path": path.display().to_string(),
                "format": "base16",
                "scheme": scheme.name,
                "colors": scheme.palette.len(),
                "tags": 6,
            })))?
        );
        return Ok(());
    }

    if dryrun::is_active() {
        println!("Would write {}", path.display());
    } else {
        println!("Wrote {}", path.display());
    }
    println!();
    println!("Set theme = \"{slug}\" in [ui] to use it.");
    Ok(())
}

// ---------------------------------------------------------------------------
// base16 import
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Base16Scheme {
    name: String,
    author: Option<String>,
    palette: BTreeMap<String, String>,
}

impl Base16Scheme {
    fn parse(text: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let value: serde_json::Value = match serde_json::from_str(text) {
            Ok(value) => value,
            Err(json_err) => serde_yaml_ng::from_str(text)
                .map_err(|yaml_err| format!("not valid JSON or YAML: {json_err}; {yaml_err}"))?,
        };
        let obj = value.as_object().ok_or("expected a mapping")?;

        let name = obj
            .get("name")
            .or_else(|| obj.get("scheme"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let author = obj
            .get("author")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let palette = obj
            .get("palette")
            .and_then(|v| v.as_object())
            .unwrap_or(obj);

        let mut colors = BTreeMap::new();
        for (key, value) in palette {
            let Some(hex) = value.as_str() else {
                continue;
            };
            let upper = key.to_ascii_uppercase();
            if !BASE16_KEYS.contains(&upper.as_str()) {
                continue;
            }
            let normalized = normalize_hex(hex).map_err(|err| format!("{key}: {err}"))?;
            colors.insert(upper, normalized);
        }

        for key in BASE16_KEYS {
            if !colors.contains_key(key) {
                return Err(format!("missing palette color {key}").into());
            }
        }

        Ok(Base16Scheme {
            name,
            author,
            palette: colors,
        })
    }

    fn to_toml(&self, theme_name: &str) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        out.push_str("# Frame theme imported from a base16 scheme.\n");
        if self.name.is_empty() {
            let _ = writeln!(out, "# Name: {theme_name}");
        } else if let Some(author) = &self.author {
            let _ = writeln!(out, "# Scheme: {} by {}", self.name, author);
        } else {
            let _ = writeln!(out, "# Scheme: {}", self.name);
        }
        out.push_str("\n[colors]\n");
        for (field, key) in BASE16_COLOR_MAP {
            let _ = writeln!(out, "{field} = \"{}\"", self.palette[key]);
        }
        out.push_str("\n[tags]\n");
        for (tag, key) in BASE16_TAG_MAP {
            let _ = writeln!(out, "{tag} = \"{}\"", self.palette[key]);
        }
        out
    }
}

const BASE16_KEYS: [&str; 16] = [
    "BASE00", "BASE01", "BASE02", "BASE03", "BASE04", "BASE05", "BASE06", "BASE07", "BASE08",
    "BASE09", "BASE0A", "BASE0B", "BASE0C", "BASE0D", "BASE0E", "BASE0F",
];

const BASE16_COLOR_MAP: [(&str, &str); 18] = [
    ("background", "BASE00"),
    ("text", "BASE05"),
    ("text_bright", "BASE06"),
    ("highlight", "BASE0E"),
    ("dim", "BASE03"),
    ("red", "BASE08"),
    ("yellow", "BASE0A"),
    ("green", "BASE0B"),
    ("cyan", "BASE0C"),
    ("purple", "BASE0E"),
    ("blue", "BASE0D"),
    ("selection_bg", "BASE02"),
    ("selection_border", "BASE0E"),
    ("selection_id", "BASE06"),
    ("search_match_bg", "BASE0C"),
    ("search_match_fg", "BASE00"),
    ("flash_bg", "BASE01"),
    ("bulk_selection_bg", "BASE02"),
];

const BASE16_TAG_MAP: [(&str, &str); 6] = [
    ("research", "BASE0D"),
    ("design", "BASE0C"),
    ("bug", "BASE08"),
    ("cc", "BASE0B"),
    ("cc-added", "BASE0E"),
    ("needs-input", "BASE0A"),
];

fn normalize_hex(text: &str) -> Result<String, String> {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("invalid hex color {text:?}"));
    }
    Ok(format!("#{}", hex.to_ascii_uppercase()))
}

fn slugify(name: &str) -> Option<String> {
    let mut slug = String::new();
    let mut dash = false;
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            if dash && !slug.is_empty() {
                slug.push('-');
            }
            dash = false;
            slug.push(c);
        } else {
            dash = true;
        }
    }
    if slug.is_empty() {
        None
    } else {
        if slug.len() > 64 {
            slug.truncate(64);
            slug = slug.trim_end_matches('-').to_string();
        }
        Some(slug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::ThemeFile;

    const TINTED: &str = r##"{
      "system": "base16",
      "name": "Ocean",
      "author": "Test",
      "variant": "dark",
      "palette": {
        "base00": "#2b303b", "base01": "#343d46", "base02": "#4f5b66",
        "base03": "#65737e", "base04": "#a7adba", "base05": "#c0c5ce",
        "base06": "#dfe1e8", "base07": "#eff1f5", "base08": "#bf616a",
        "base09": "#d08770", "base0A": "#ebcb8b", "base0B": "#a3be8c",
        "base0C": "#96b5b4", "base0D": "#8fa1b3", "base0E": "#b48ead",
        "base0F": "#ab7967"
      }
    }"##;

    #[test]
    fn parses_a_tinted_scheme() {
        let scheme = Base16Scheme::parse(TINTED).unwrap();
        assert_eq!(scheme.name, "Ocean");
        assert_eq!(scheme.author.as_deref(), Some("Test"));
        assert_eq!(scheme.palette["BASE00"], "#2B303B");
    }

    #[test]
    fn parses_a_yaml_scheme() {
        let text = r##"
system: base16
name: Ocean
author: Test
variant: dark
palette:
  base00: "#2b303b"
  base01: "#343d46"
  base02: "#4f5b66"
  base03: "#65737e"
  base04: "#a7adba"
  base05: "#c0c5ce"
  base06: "#dfe1e8"
  base07: "#eff1f5"
  base08: "#bf616a"
  base09: "#d08770"
  base0A: "#ebcb8b"
  base0B: "#a3be8c"
  base0C: "#96b5b4"
  base0D: "#8fa1b3"
  base0E: "#b48ead"
  base0F: "#ab7967"
"##;
        let scheme = Base16Scheme::parse(text).unwrap();
        assert_eq!(scheme.name, "Ocean");
        assert_eq!(scheme.palette["BASE00"], "#2B303B");
    }

    #[test]
    fn parses_a_flat_yaml_scheme() {
        let text = r##"
base00: "#111111"
base01: "#222222"
base02: "#333333"
base03: "#444444"
base04: "#555555"
base05: "#666666"
base06: "#777777"
base07: "#888888"
base08: "#999999"
base09: "#aaaaaa"
base0A: "#bbbbbb"
base0B: "#cccccc"
base0C: "#dddddd"
base0D: "#eeeeee"
base0E: "#ffffff"
base0F: "#000000"
"##;
        let scheme = Base16Scheme::parse(text).unwrap();
        assert_eq!(scheme.palette["BASE0E"], "#FFFFFF");
    }

    #[test]
    fn parses_a_flat_scheme() {
        let text = r##"{
          "base00": "#111111", "base01": "#222222", "base02": "#333333",
          "base03": "#444444", "base04": "#555555", "base05": "#666666",
          "base06": "#777777", "base07": "#888888", "base08": "#999999",
          "base09": "#aaaaaa", "base0A": "#bbbbbb", "base0B": "#cccccc",
          "base0C": "#dddddd", "base0D": "#eeeeee", "base0E": "#ffffff",
          "base0F": "#000000"
        }"##;
        let scheme = Base16Scheme::parse(text).unwrap();
        assert_eq!(scheme.name, "");
        assert_eq!(scheme.palette["BASE0E"], "#FFFFFF");
    }

    #[test]
    fn a_missing_color_is_an_error() {
        let text = r##"{"base01": "#222222"}"##;
        let err = Base16Scheme::parse(text).unwrap_err().to_string();
        assert!(err.contains("BASE00"), "{err}");
    }

    #[test]
    fn an_invalid_hex_is_an_error() {
        let text = r##"{"base00": "zzz", "base01": "#222222"}"##;
        // missing all other keys too, but the hex error surfaces first only if
        // base00 is reached; palette iteration order is BTreeMap, so BASE00
        // comes first.
        let err = Base16Scheme::parse(text).unwrap_err().to_string();
        assert!(err.contains("invalid hex color"), "{err}");
    }

    #[test]
    fn slugifies_a_name() {
        assert_eq!(slugify("Ocean Dark").as_deref(), Some("ocean-dark"));
        assert_eq!(slugify("Ocean!!Dark").as_deref(), Some("ocean-dark"));
        assert_eq!(slugify("   ").as_deref(), None);
        assert_eq!(slugify("!!!").as_deref(), None);
    }

    #[test]
    fn generated_toml_parses_as_a_theme_file() {
        let scheme = Base16Scheme::parse(TINTED).unwrap();
        let body = scheme.to_toml("ocean");
        let file: ThemeFile = toml::from_str(&body).unwrap();
        assert_eq!(file.colors["background"], "#2B303B");
        assert_eq!(file.colors["highlight"], "#B48EAD");
        assert_eq!(file.tags["bug"], "#BF616A");
    }

    #[test]
    fn generated_toml_maps_base16_roles() {
        let scheme = Base16Scheme::parse(TINTED).unwrap();
        let body = scheme.to_toml("ocean");
        let file: ThemeFile = toml::from_str(&body).unwrap();
        assert_eq!(file.colors["background"], "#2B303B");
        assert_eq!(file.colors["text"], "#C0C5CE");
        assert_eq!(file.colors["dim"], "#65737E");
        assert_eq!(file.colors["red"], "#BF616A");
        assert_eq!(file.colors["yellow"], "#EBCB8B");
        assert_eq!(file.colors["green"], "#A3BE8C");
        assert_eq!(file.colors["cyan"], "#96B5B4");
        assert_eq!(file.colors["blue"], "#8FA1B3");
        assert_eq!(file.colors["purple"], "#B48EAD");
        assert_eq!(file.colors["highlight"], "#B48EAD");
        assert_eq!(file.colors["selection_bg"], "#4F5B66");
        assert_eq!(file.colors["flash_bg"], "#343D46");
    }
}
