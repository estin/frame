use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::style::Color;
use serde::Deserialize;
use thiserror::Error;

use crate::model::UiConfig;

/// The name of the built-in theme that frame ships with.
pub const DEFAULT_THEME_NAME: &str = "default";

/// The name of the built-in light theme.
pub const LIGHT_THEME_NAME: &str = "light";

/// Names of the themes frame ships with.
pub const BUILTIN_THEMES: &[&str] = &[DEFAULT_THEME_NAME, LIGHT_THEME_NAME];

/// Parsed color theme for the TUI
#[derive(Debug, Clone)]
pub struct Theme {
    /// All color fields are public so theme files can be serialized. Set
    /// known keys via [`Theme::set_named_color`] or by mutating directly.
    pub background: Color,
    pub text: Color,
    pub text_bright: Color,
    pub highlight: Color,
    pub dim: Color,
    pub red: Color,
    pub yellow: Color,
    pub green: Color,
    pub cyan: Color,
    pub purple: Color,
    pub blue: Color,
    pub selection_bg: Color,
    pub selection_border: Color,
    pub selection_id: Color,
    pub search_match_bg: Color,
    pub search_match_fg: Color,
    pub flash_bg: Color,
    /// Background for selected rows in SELECT mode
    pub bulk_selection_bg: Color,
    /// Per-tag colors
    pub tag_colors: HashMap<String, Color>,
}

impl Default for Theme {
    fn default() -> Self {
        let mut tag_colors = HashMap::new();
        tag_colors.insert("research".into(), Color::Rgb(0x44, 0x88, 0xFF));
        tag_colors.insert("design".into(), Color::Rgb(0x44, 0xDD, 0xFF));
        tag_colors.insert("bug".into(), Color::Rgb(0xFF, 0x44, 0x44));
        tag_colors.insert("cc".into(), Color::Rgb(0x44, 0xFF, 0x88));
        tag_colors.insert("cc-added".into(), Color::Rgb(0xCC, 0x66, 0xFF));
        tag_colors.insert("needs-input".into(), Color::Rgb(0xFF, 0xD7, 0x00));

        Theme {
            background: Color::Rgb(0x0C, 0x00, 0x1B),
            text: Color::Rgb(0xB0, 0xAA, 0xFF),
            text_bright: Color::Rgb(0xFF, 0xFF, 0xFF),
            highlight: Color::Rgb(0xFB, 0x41, 0x96),
            dim: Color::Rgb(0x7D, 0x78, 0xBF),
            red: Color::Rgb(0xFF, 0x44, 0x44),
            yellow: Color::Rgb(0xFF, 0xD7, 0x00),
            green: Color::Rgb(0x44, 0xFF, 0x88),
            cyan: Color::Rgb(0x44, 0xDD, 0xFF),
            purple: Color::Rgb(0xCC, 0x66, 0xFF),
            blue: Color::Rgb(0x44, 0x88, 0xFF),
            selection_bg: Color::Rgb(0x43, 0x16, 0x3E),
            selection_border: Color::Rgb(0xFB, 0x41, 0x96),
            selection_id: Color::Rgb(0xDA, 0xB8, 0xF0),
            search_match_bg: Color::Rgb(0x40, 0xE0, 0xD0),
            search_match_fg: Color::Rgb(0x0C, 0x00, 0x1B),
            flash_bg: Color::Rgb(0x3D, 0x2E, 0x10),
            bulk_selection_bg: Color::Rgb(0x13, 0x20, 0x4A),
            tag_colors,
        }
    }
}

/// Parse a hex color string like "#FF4444" into an RGB Color (public for use by tag color popup)
pub fn parse_hex_color_pub(hex: &str) -> Option<Color> {
    parse_hex_color(hex)
}

/// Parse a hex color string like "#FF4444" into an RGB Color
fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// A theme file: `frame/themes/<name>.toml` or `<config>/frame/themes/<name>.toml`.
///
/// `[colors]` takes the same key names as `[ui.colors]`, and `[tags]` maps tag
/// names to colors. `extends` names another theme or a built-in preset to
/// start from, so a file can override one color and keep the rest.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeFile {
    #[serde(default)]
    pub extends: Option<String>,
    #[serde(default)]
    pub colors: BTreeMap<String, String>,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
}

/// A theme name is a plain file stem. No separators, no `..`, no dots.
fn is_valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Where frame looks for theme files, in priority order: project, then global.
#[derive(Debug, Clone)]
pub struct ThemeDirs {
    /// `<project>/frame/themes`
    pub project: PathBuf,
    /// `<config home>/frame/themes`
    pub global: PathBuf,
}

impl ThemeDirs {
    pub fn new(project: PathBuf, global: PathBuf) -> Self {
        ThemeDirs { project, global }
    }

    /// The theme directories for a project on this machine.
    pub fn for_project(frame_dir: &Path) -> Self {
        ThemeDirs {
            project: frame_dir.join("themes"),
            global: Self::global(),
        }
    }

    /// The shared theme directory for every project.
    pub fn global() -> PathBuf {
        crate::io::registry::config_dir()
            .join("frame")
            .join("themes")
    }

    /// Every theme name present in either directory, sorted.
    pub fn names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for dir in [&self.project, &self.global] {
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                if is_valid_theme_name(stem)
                    && !names.iter().any(|known: &String| known.as_str() == stem)
                {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        names
    }

    /// The file that wins for `name`: the project file, else the global file.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        if !is_valid_theme_name(name) {
            return None;
        }
        let file = format!("{name}.toml");
        [&self.project, &self.global]
            .into_iter()
            .map(|dir| dir.join(&file))
            .find(|path| path.is_file())
    }

    /// Resolve a theme name to a palette, or `None` when no usable file exists.
    ///
    /// A broken theme file must not stop the TUI from rendering, so errors are
    /// swallowed here. `fr theme list` reports them through [`Self::load_checked`].
    pub fn load(&self, name: &str) -> Option<Theme> {
        self.load_checked(name).ok().flatten()
    }

    /// Resolve a theme name and report why it failed.
    pub fn load_checked(&self, name: &str) -> Result<Option<Theme>, ThemeFileError> {
        self.resolve(name, &mut Vec::new())
    }

    fn resolve(
        &self,
        name: &str,
        stack: &mut Vec<String>,
    ) -> Result<Option<Theme>, ThemeFileError> {
        if !is_valid_theme_name(name) {
            return Err(ThemeFileError::InvalidName(name.to_string()));
        }
        if stack.iter().any(|seen| seen == name) {
            return Err(ThemeFileError::Cycle(name.to_string()));
        }
        let Some(path) = self.find(name) else {
            return Ok(None);
        };
        let file = read_theme_file(&path)?;

        let mut theme = match file.extends.as_deref() {
            Some(parent) => {
                stack.push(name.to_string());
                let base = self.resolve(parent, stack);
                stack.pop();
                base?.unwrap_or_else(|| Theme::preset(parent).unwrap_or_default())
            }
            None => Theme::default(),
        };
        theme.apply_file(&file);
        Ok(Some(theme))
    }
}

fn read_theme_file(path: &Path) -> Result<ThemeFile, ThemeFileError> {
    let text = fs::read_to_string(path).map_err(|source| ThemeFileError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| ThemeFileError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Why a theme file could not be used.
#[derive(Debug, Error)]
pub enum ThemeFileError {
    #[error("invalid theme name {0:?}")]
    InvalidName(String),
    #[error("theme {0:?} is part of an extends cycle")]
    Cycle(String),
    #[error("{}: {source}", .path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{}: {source}", .path.display())]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

impl Theme {
    /// Look up a named preset theme by name.
    ///
    /// Returns `None` for names frame does not know, which callers treat as
    /// "use the default theme".
    pub fn preset(name: &str) -> Option<Self> {
        match name {
            DEFAULT_THEME_NAME => Some(Theme::default()),
            LIGHT_THEME_NAME => Some(Theme::light()),
            _ => None,
        }
    }

    /// A light preset theme, for terminals and project setups that prefer it.
    pub fn light() -> Self {
        let mut tag_colors = HashMap::new();
        tag_colors.insert("research".into(), Color::Rgb(0x1F, 0x4E, 0x8F));
        tag_colors.insert("design".into(), Color::Rgb(0x00, 0x52, 0x74));
        tag_colors.insert("bug".into(), Color::Rgb(0xC0, 0x00, 0x00));
        tag_colors.insert("cc".into(), Color::Rgb(0x00, 0x66, 0x33));
        tag_colors.insert("cc-added".into(), Color::Rgb(0x66, 0x2B, 0x99));
        tag_colors.insert("needs-input".into(), Color::Rgb(0x8A, 0x6B, 0x00));

        Theme {
            background: Color::Rgb(0xF7, 0xF7, 0xFA),
            text: Color::Rgb(0x21, 0x21, 0x3A),
            text_bright: Color::Rgb(0x00, 0x00, 0x00),
            highlight: Color::Rgb(0xB0, 0x00, 0x6E),
            dim: Color::Rgb(0x5C, 0x5C, 0x7A),
            red: Color::Rgb(0xC0, 0x00, 0x00),
            yellow: Color::Rgb(0x8A, 0x6B, 0x00),
            green: Color::Rgb(0x00, 0x66, 0x33),
            cyan: Color::Rgb(0x00, 0x52, 0x74),
            purple: Color::Rgb(0x6E, 0x2B, 0x99),
            blue: Color::Rgb(0x1F, 0x4E, 0x8F),
            selection_bg: Color::Rgb(0xE3, 0xE3, 0xF2),
            selection_border: Color::Rgb(0xB0, 0x00, 0x6E),
            selection_id: Color::Rgb(0x33, 0x33, 0x66),
            search_match_bg: Color::Rgb(0x00, 0x52, 0x74),
            search_match_fg: Color::Rgb(0xFF, 0xFF, 0xFF),
            flash_bg: Color::Rgb(0xF3, 0xEC, 0xD8),
            bulk_selection_bg: Color::Rgb(0xE3, 0xE3, 0xF2),
            tag_colors,
        }
    }

    /// Create a theme from built-in presets and project UI overrides.
    ///
    /// This reads no theme files. Use [`Theme::load`] in a running project so
    /// that `[ui] theme` can resolve to a `themes/<name>.toml` file.
    pub fn from_config(ui: &UiConfig) -> Self {
        let mut theme = match ui.theme.as_deref() {
            Some(name) => Theme::preset(name).unwrap_or_default(),
            None => Theme::default(),
        };
        theme.apply_overrides(ui);
        theme
    }

    /// Create a theme from theme files, built-in presets and UI overrides.
    ///
    /// A project theme file wins over a global file of the same name, and a
    /// theme file wins over a built-in preset. A name that resolves to nothing
    /// uses the default palette. `[ui.colors]` and `[ui.tag_colors]` apply
    /// last, on top of whichever palette won.
    pub fn load(ui: &UiConfig, dirs: &ThemeDirs) -> Self {
        let mut theme = match ui.theme.as_deref() {
            Some(name) => dirs
                .load(name)
                .unwrap_or_else(|| Theme::preset(name).unwrap_or_default()),
            None => Theme::default(),
        };
        theme.apply_overrides(ui);
        theme
    }

    fn apply_overrides(&mut self, ui: &UiConfig) {
        // Apply color overrides from [ui.colors]
        for (key, value) in &ui.colors {
            if let Some(color) = parse_hex_color(value) {
                self.set_named_color(key, color);
            }
        }

        // Apply tag color overrides from [ui.tag_colors]
        for (tag, value) in &ui.tag_colors {
            if let Some(color) = parse_hex_color(value) {
                self.tag_colors.insert(tag.clone(), color);
            }
        }
    }

    fn apply_file(&mut self, file: &ThemeFile) {
        for (key, value) in &file.colors {
            if let Some(color) = parse_hex_color(value) {
                self.set_named_color(key, color);
            }
        }
        for (tag, value) in &file.tags {
            if let Some(color) = parse_hex_color(value) {
                self.tag_colors.insert(tag.clone(), color);
            }
        }
    }

    /// Set one named theme color. Unknown keys are ignored, so a theme file
    /// written for a newer frame still renders in an older one.
    fn set_named_color(&mut self, key: &str, color: Color) {
        match key {
            "background" => self.background = color,
            "text" => self.text = color,
            "text_bright" => self.text_bright = color,
            "highlight" => self.highlight = color,
            "dim" => self.dim = color,
            "red" => self.red = color,
            "yellow" => self.yellow = color,
            "green" => self.green = color,
            "cyan" => self.cyan = color,
            "purple" => self.purple = color,
            "blue" => self.blue = color,
            "selection_bg" => self.selection_bg = color,
            "selection_border" => self.selection_border = color,
            "selection_id" => self.selection_id = color,
            "search_match_bg" => self.search_match_bg = color,
            "search_match_fg" => self.search_match_fg = color,
            "flash_bg" => self.flash_bg = color,
            "bulk_selection_bg" | "selection_bg_bulk" => self.bulk_selection_bg = color,
            _ => {}
        }
    }
    /// Get the color for a tag, falling back to text color
    pub fn tag_color(&self, tag: &str) -> Color {
        self.tag_colors.get(tag).copied().unwrap_or(self.text)
    }

    /// Set or replace a tag color.
    pub fn set_tag_color(&mut self, tag: &str, color: Color) {
        self.tag_colors.insert(tag.to_string(), color);
    }

    /// Get the color for a task state
    pub fn state_color(&self, state: crate::model::TaskState) -> Color {
        match state {
            crate::model::TaskState::Todo => self.text,
            crate::model::TaskState::Active => self.highlight,
            crate::model::TaskState::Blocked => self.red,
            crate::model::TaskState::Done => self.text,
            crate::model::TaskState::Parked => self.yellow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(
            parse_hex_color("#FF4444"),
            Some(Color::Rgb(0xFF, 0x44, 0x44))
        );
        assert_eq!(
            parse_hex_color("#0C001B"),
            Some(Color::Rgb(0x0C, 0x00, 0x1B))
        );
        assert_eq!(parse_hex_color("FF4444"), None); // missing #
        assert_eq!(parse_hex_color("#FF44"), None); // too short
        assert_eq!(parse_hex_color("#ZZZZZZ"), None); // invalid hex
    }

    #[test]
    fn test_default_theme_has_all_tag_colors() {
        let theme = Theme::default();
        assert_eq!(
            theme.tag_colors.get("research"),
            Some(&Color::Rgb(0x44, 0x88, 0xFF))
        );
        assert_eq!(
            theme.tag_colors.get("bug"),
            Some(&Color::Rgb(0xFF, 0x44, 0x44))
        );
        assert_eq!(
            theme.tag_colors.get("cc"),
            Some(&Color::Rgb(0x44, 0xFF, 0x88))
        );
    }

    #[test]
    fn test_preset_default() {
        let theme = Theme::preset(DEFAULT_THEME_NAME).expect("default preset");
        assert_eq!(theme.background, Color::Rgb(0x0C, 0x00, 0x1B));
        assert_eq!(theme.text, Color::Rgb(0xB0, 0xAA, 0xFF));
    }

    #[test]
    fn test_preset_light() {
        let theme = Theme::preset("light").expect("light preset");
        assert_eq!(theme.background, Color::Rgb(0xF7, 0xF7, 0xFA));
        assert_eq!(theme.text, Color::Rgb(0x21, 0x21, 0x3A));
        assert_eq!(theme.highlight, Color::Rgb(0xB0, 0x00, 0x6E));
    }

    #[test]
    fn test_preset_unknown_is_none() {
        assert!(Theme::preset("does-not-exist").is_none());
    }

    #[test]
    fn test_from_config_missing_theme_uses_default() {
        assert_eq!(
            Theme::from_config(&UiConfig::default()).background,
            Color::Rgb(0x0C, 0x00, 0x1B)
        );
    }

    #[test]
    fn test_from_config_named_theme_selects_preset() {
        let ui = UiConfig {
            theme: Some("light".into()),
            ..Default::default()
        };
        assert_eq!(
            Theme::from_config(&ui).background,
            Color::Rgb(0xF7, 0xF7, 0xFA)
        );
    }

    #[test]
    fn test_from_config_unknown_theme_falls_back_to_default() {
        let ui = UiConfig {
            theme: Some("does-not-exist".into()),
            ..Default::default()
        };
        assert_eq!(
            Theme::from_config(&ui).background,
            Color::Rgb(0x0C, 0x00, 0x1B)
        );
    }

    #[test]
    fn test_from_config_named_theme_plus_override() {
        let mut ui = UiConfig {
            theme: Some("light".into()),
            ..Default::default()
        };
        ui.colors.insert("background".into(), "#000000".into());
        assert_eq!(Theme::from_config(&ui).background, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn test_from_config_overrides() {
        let mut ui = UiConfig::default();
        ui.colors.insert("background".into(), "#000000".into());
        ui.tag_colors.insert("custom".into(), "#112233".into());

        let theme = Theme::from_config(&ui);
        assert_eq!(theme.background, Color::Rgb(0, 0, 0));
        assert_eq!(
            theme.tag_colors.get("custom"),
            Some(&Color::Rgb(0x11, 0x22, 0x33))
        );
        // Unchanged defaults still present
        assert_eq!(theme.text, Color::Rgb(0xB0, 0xAA, 0xFF));
    }

    #[test]
    fn test_tag_color_fallback() {
        let theme = Theme::default();
        assert_eq!(theme.tag_color("research"), Color::Rgb(0x44, 0x88, 0xFF));
        // Unknown tag falls back to text color
        assert_eq!(theme.tag_color("unknown"), theme.text);
    }

    #[test]
    fn test_state_color() {
        use crate::model::TaskState;
        let theme = Theme::default();
        assert_eq!(theme.state_color(TaskState::Active), theme.highlight);
        assert_eq!(theme.state_color(TaskState::Blocked), theme.red);
        assert_eq!(theme.state_color(TaskState::Done), theme.text);
        assert_eq!(theme.state_color(TaskState::Parked), theme.yellow);
    }

    // -- Theme files --------------------------------------------------------

    fn theme_dirs(root: &std::path::Path) -> ThemeDirs {
        ThemeDirs::new(root.join("frame/themes"), root.join("config/frame/themes"))
    }

    fn write_theme(root: &std::path::Path, name: &str, body: &str) {
        let dir = root.join("frame/themes");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.toml")), body).unwrap();
    }

    fn write_global_theme(root: &std::path::Path, name: &str, body: &str) {
        let dir = root.join("config/frame/themes");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.toml")), body).unwrap();
    }

    fn ui_with_theme(name: &str) -> UiConfig {
        UiConfig {
            theme: Some(name.into()),
            ..Default::default()
        }
    }

    #[test]
    fn a_project_theme_file_sets_the_palette() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_theme(
            tmp.path(),
            "custom",
            "[colors]\nbackground = \"#010203\"\ntext = \"#040506\"\n[tags]\nbug = \"#070809\"\n",
        );
        let dirs = theme_dirs(tmp.path());
        let theme = Theme::load(&ui_with_theme("custom"), &dirs);
        assert_eq!(theme.background, Color::Rgb(1, 2, 3));
        assert_eq!(theme.text, Color::Rgb(4, 5, 6));
        assert_eq!(theme.tag_color("bug"), Color::Rgb(7, 8, 9));
        // Unset colors keep the default palette.
        assert_eq!(theme.highlight, Theme::default().highlight);
    }

    #[test]
    fn a_project_theme_file_wins_over_a_global_one() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_theme(tmp.path(), "shared", "[colors]\nbackground = \"#111111\"\n");
        write_global_theme(tmp.path(), "shared", "[colors]\nbackground = \"#222222\"\n");
        let dirs = theme_dirs(tmp.path());
        let theme = Theme::load(&ui_with_theme("shared"), &dirs);
        assert_eq!(theme.background, Color::Rgb(0x11, 0x11, 0x11));
    }

    #[test]
    fn a_global_theme_file_is_used_when_the_project_has_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_global_theme(tmp.path(), "shared", "[colors]\nbackground = \"#222222\"\n");
        let dirs = theme_dirs(tmp.path());
        let theme = Theme::load(&ui_with_theme("shared"), &dirs);
        assert_eq!(theme.background, Color::Rgb(0x22, 0x22, 0x22));
    }

    #[test]
    fn extends_starts_from_a_builtin_and_changes_one_color() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_theme(
            tmp.path(),
            "tweak",
            "extends = \"default\"\n[colors]\nbackground = \"#010101\"\n",
        );
        let dirs = theme_dirs(tmp.path());
        let theme = Theme::load(&ui_with_theme("tweak"), &dirs);
        assert_eq!(theme.background, Color::Rgb(1, 1, 1));
        assert_eq!(theme.text, Theme::default().text);
    }

    #[test]
    fn extends_starts_from_another_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_theme(
            tmp.path(),
            "base",
            "[colors]\nbackground = \"#010101\"\ntext = \"#020202\"\n",
        );
        write_theme(
            tmp.path(),
            "child",
            "extends = \"base\"\n[colors]\ntext = \"#030303\"\n",
        );
        let dirs = theme_dirs(tmp.path());
        let theme = Theme::load(&ui_with_theme("child"), &dirs);
        assert_eq!(theme.background, Color::Rgb(1, 1, 1));
        assert_eq!(theme.text, Color::Rgb(3, 3, 3));
    }

    #[test]
    fn an_extends_cycle_is_reported_and_falls_back() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_theme(tmp.path(), "a", "extends = \"b\"\n");
        write_theme(tmp.path(), "b", "extends = \"a\"\n");
        let dirs = theme_dirs(tmp.path());
        assert!(dirs.load_checked("a").is_err());
        let theme = Theme::load(&ui_with_theme("a"), &dirs);
        assert_eq!(theme.background, Theme::default().background);
    }

    #[test]
    fn a_missing_theme_file_falls_back_to_a_builtin() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dirs = theme_dirs(tmp.path());
        let theme = Theme::load(&ui_with_theme(LIGHT_THEME_NAME), &dirs);
        assert_eq!(theme.background, Theme::light().background);
    }

    #[test]
    fn a_broken_theme_file_falls_back_to_the_default() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_theme(tmp.path(), "broken", "[colors\n");
        let dirs = theme_dirs(tmp.path());
        assert!(dirs.load_checked("broken").is_err());
        let theme = Theme::load(&ui_with_theme("broken"), &dirs);
        assert_eq!(theme.background, Theme::default().background);
    }

    #[test]
    fn a_theme_name_cannot_escape_the_theme_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dirs = theme_dirs(tmp.path());
        assert!(dirs.load_checked("../project").is_err());
        let theme = Theme::load(&ui_with_theme("../project"), &dirs);
        assert_eq!(theme.background, Theme::default().background);
    }

    #[test]
    fn ui_overrides_apply_on_top_of_a_file_theme() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_theme(tmp.path(), "custom", "[colors]\nbackground = \"#010101\"\n");
        let dirs = theme_dirs(tmp.path());
        let mut ui = ui_with_theme("custom");
        ui.colors.insert("background".into(), "#000000".into());
        assert_eq!(Theme::load(&ui, &dirs).background, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn names_lists_project_and_global_files_once() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_theme(tmp.path(), "beta", "");
        write_theme(tmp.path(), "alpha", "");
        write_global_theme(tmp.path(), "beta", "");
        write_global_theme(tmp.path(), "gamma", "");
        let dir = tmp.path().join("config/frame/themes");
        std::fs::write(dir.join("notes.txt"), "").unwrap();
        assert_eq!(
            theme_dirs(tmp.path()).names(),
            vec!["alpha", "beta", "gamma"]
        );
    }
}
