//! Configuration loading and validation.
//!
//! The configuration is intentionally small, typed, and side-effect free.
//! Reading a malformed file never prevents the monitor from starting; the
//! problem is reported as a warning while safe defaults remain in effect.

use std::fmt;
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::theme::Theme;

/// Largest configuration or theme file we are willing to read.
const MAX_CONFIG_BYTES: u64 = 128 * 1024;
/// Largest theme name we accept.
const MAX_THEME_NAME: usize = 64;

/// Graph character set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphSymbol {
    #[default]
    Braille,
    Block,
    Tty,
}

impl GraphSymbol {
    /// Parse a graph symbol name. Also accepts the names btop uses.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "braille" => Some(Self::Braille),
            "block" => Some(Self::Block),
            "tty" | "ascii" => Some(Self::Tty),
            _ => None,
        }
    }
}

impl fmt::Display for GraphSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Braille => f.write_str("braille"),
            Self::Block => f.write_str("block"),
            Self::Tty => f.write_str("tty"),
        }
    }
}

/// Process table sort key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcSort {
    #[default]
    Cpu,
    Memory,
    Pid,
    Name,
    User,
}

impl ProcSort {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "cpu" => Some(Self::Cpu),
            "mem" | "memory" => Some(Self::Memory),
            "pid" => Some(Self::Pid),
            "name" | "program" => Some(Self::Name),
            "user" => Some(Self::User),
            _ => None,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "mem",
            Self::Pid => "pid",
            Self::Name => "name",
            Self::User => "user",
        }
    }

    #[must_use]
    pub const fn cycle(self) -> Self {
        match self {
            Self::Cpu => Self::Memory,
            Self::Memory => Self::Pid,
            Self::Pid => Self::Name,
            Self::Name => Self::User,
            Self::User => Self::Cpu,
        }
    }
}

impl fmt::Display for ProcSort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// User-visible configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Refresh interval in milliseconds. Clamped to `250..=10_000`.
    pub update_ms: u64,
    /// Initial preset: `0` overview, `1` CPU, `2` memory/disks, `3` network,
    /// `4` processes.
    pub preset: u8,
    /// Graph character set.
    pub graph_symbol: GraphSymbol,
    /// Built-in theme name or a file in the themes directory.
    pub theme: String,
    /// Use 24-bit colours. When false, colours are quantised to 256 colours.
    pub truecolor: bool,
    /// Force TTY-friendly borders and graph symbols.
    pub tty_mode: bool,
    /// Use rounded box corners where the character set allows it.
    pub rounded_corners: bool,
    /// Paint the theme background across the whole screen.
    pub theme_background: bool,
    /// Show the battery indicator when the platform exposes one.
    pub show_battery: bool,
    /// Default process sort.
    pub proc_sort: ProcSort,
    /// Start with the process tree enabled.
    pub proc_tree: bool,
    /// Reverse the process sort order.
    pub proc_reversed: bool,
    /// Automatically follow the busiest network interface.
    pub net_auto: bool,
    /// Preferred network interface when `net_auto` is false.
    pub net_iface: String,
    /// Enable mouse reporting.
    pub mouse: bool,
    /// Disable process signalling entirely.
    pub read_only: bool,
    /// Show the help footer with key hints.
    pub show_key_hints: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            update_ms: 1_000,
            preset: 0,
            graph_symbol: GraphSymbol::Braille,
            theme: "default".to_owned(),
            truecolor: true,
            tty_mode: false,
            rounded_corners: true,
            theme_background: false,
            show_battery: true,
            proc_sort: ProcSort::Cpu,
            proc_tree: false,
            proc_reversed: false,
            net_auto: true,
            net_iface: String::new(),
            mouse: true,
            read_only: false,
            show_key_hints: true,
        }
    }
}

impl Config {
    /// Path to the configuration file in the platform configuration directory
    /// (`$XDG_CONFIG_HOME`/`~/.config` on Linux, `Application Support` on
    /// macOS, `%APPDATA%` on Windows).
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("rtop").join("rtop.toml"))
    }

    /// Directory containing user-supplied `.theme` files.
    #[must_use]
    pub fn theme_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("rtop").join("themes"))
    }

    /// Load configuration.
    ///
    /// With no explicit path, a missing file is fine and defaults are used.
    /// An explicit path that does not exist is an error. Parse and validation
    /// problems are returned as warnings and leave defaults in place.
    pub fn load(explicit: Option<&Path>) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let path = match explicit {
            Some(path) => Some(path.to_path_buf()),
            None => Self::default_path(),
        };
        let Some(path) = path else {
            return (Self::default(), warnings);
        };

        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound && explicit.is_none() => {
                return (Self::default(), warnings);
            }
            Err(error) => {
                warnings.push(format!("cannot read {}: {error}", path.display()));
                return (Self::default(), warnings);
            }
        };
        if !metadata.is_file() {
            warnings.push(format!("{} is not a regular file", path.display()));
            return (Self::default(), warnings);
        }
        if metadata.len() > MAX_CONFIG_BYTES {
            warnings.push(format!(
                "{} is larger than {} KiB; ignoring it",
                path.display(),
                MAX_CONFIG_BYTES / 1024
            ));
            return (Self::default(), warnings);
        }
        let text = match read_capped(&path, MAX_CONFIG_BYTES) {
            Ok(text) => text,
            Err(error) => {
                warnings.push(format!("cannot read {}: {error}", path.display()));
                return (Self::default(), warnings);
            }
        };
        let mut config: Self = match toml::from_str(&text) {
            Ok(config) => config,
            Err(error) => {
                warnings.push(format!("invalid config {}: {error}", path.display()));
                return (Self::default(), warnings);
            }
        };
        warnings.extend(config.validate());
        (config, warnings)
    }

    /// Clamp invalid values and report what was changed.
    #[must_use]
    pub fn validate(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();
        if !(250..=10_000).contains(&self.update_ms) {
            warnings.push(format!(
                "update_ms {} is outside 250..=10000; using 1000",
                self.update_ms
            ));
            self.update_ms = 1_000;
        }
        if self.preset > 4 {
            warnings.push(format!("preset {} is outside 0..=4; using 0", self.preset));
            self.preset = 0;
        }
        if !valid_theme_name(&self.theme) {
            warnings.push(format!(
                "theme name {:?} is invalid; using \"default\"",
                self.theme
            ));
            "default".clone_into(&mut self.theme);
        }
        if self.net_iface.len() > 128 || self.net_iface.chars().any(char::is_control) {
            warnings.push("net_iface is invalid; using automatic selection".to_owned());
            self.net_iface.clear();
            self.net_auto = true;
        }
        warnings
    }

    /// Resolve the configured theme, loading a user theme file when needed.
    #[must_use]
    pub fn load_theme(&self) -> (Theme, Vec<String>) {
        let mut warnings = Vec::new();
        if let Some(theme) = Theme::builtin(&self.theme) {
            return (theme, warnings);
        }
        let Some(dir) = Self::theme_dir() else {
            warnings.push("cannot locate the configuration directory for themes".to_owned());
            return (
                Theme::builtin("default").expect("builtin default"),
                warnings,
            );
        };
        if !valid_theme_name(&self.theme) {
            warnings.push(format!("invalid theme name {:?}", self.theme));
            return (
                Theme::builtin("default").expect("builtin default"),
                warnings,
            );
        }
        let path = dir.join(format!("{}.theme", self.theme));
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() && metadata.len() <= MAX_CONFIG_BYTES => {}
            Ok(_) => {
                warnings.push(format!(
                    "theme file {} is invalid or too large",
                    path.display()
                ));
                return (
                    Theme::builtin("default").expect("builtin default"),
                    warnings,
                );
            }
            Err(error) => {
                warnings.push(format!("cannot load theme {}: {error}", path.display()));
                return (
                    Theme::builtin("default").expect("builtin default"),
                    warnings,
                );
            }
        }
        match read_capped(&path, MAX_CONFIG_BYTES) {
            Ok(text) => {
                let fallback = Theme::builtin("default").expect("builtin default");
                let theme = Theme::from_btop_theme(&text, &fallback).renamed(&self.theme);
                (theme, warnings)
            }
            Err(error) => {
                warnings.push(format!("cannot read theme {}: {error}", path.display()));
                (
                    Theme::builtin("default").expect("builtin default"),
                    warnings,
                )
            }
        }
    }
}

fn read_capped(path: &Path, limit: u64) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("file is larger than {limit} bytes"));
    }
    String::from_utf8(bytes).map_err(|error| format!("file is not valid UTF-8: {error}"))
}

/// Theme names may not contain separators or traversal sequences.
#[must_use]
pub fn valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_THEME_NAME
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let mut config = Config::default();
        assert_eq!(config.update_ms, 1_000);
        assert!(!config.read_only);
        assert!(config.validate().is_empty());
    }

    #[test]
    fn invalid_values_are_clamped() {
        let mut config = Config {
            update_ms: 1,
            preset: 9,
            theme: "../evil".to_owned(),
            net_iface: "bad\nname".to_owned(),
            ..Config::default()
        };
        let warnings = config.validate();
        assert_eq!(config.update_ms, 1_000);
        assert_eq!(config.preset, 0);
        assert_eq!(config.theme, "default");
        assert!(config.net_iface.is_empty());
        assert_eq!(warnings.len(), 4);
    }

    #[test]
    fn theme_names_are_restricted() {
        assert!(valid_theme_name("dracula"));
        assert!(valid_theme_name("my-theme_2"));
        assert!(!valid_theme_name(""));
        assert!(!valid_theme_name("../x"));
        assert!(!valid_theme_name("a/b"));
        assert!(!valid_theme_name("a\\b"));
        assert!(!valid_theme_name("a.b"));
    }

    #[test]
    fn toml_round_trip() {
        let config = Config {
            update_ms: 500,
            theme: "nord".to_owned(),
            proc_tree: true,
            ..Config::default()
        };
        let text = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn unknown_keys_are_ignored_for_forward_compatibility() {
        let parsed: Config = toml::from_str("update_ms = 250\nfuture_option = true\n").unwrap();
        assert_eq!(parsed.update_ms, 250);
    }

    #[test]
    fn malformed_config_falls_back_with_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "update_ms = \"not a number\"\n").unwrap();
        let (config, warnings) = Config::load(Some(&path));
        assert_eq!(config, Config::default());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("invalid config"))
        );
    }

    #[test]
    fn oversized_config_is_rejected_before_parsing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.toml");
        std::fs::write(&path, vec![b'#'; (MAX_CONFIG_BYTES + 1) as usize]).unwrap();
        let (config, warnings) = Config::load(Some(&path));
        assert_eq!(config, Config::default());
        assert!(warnings.iter().any(|warning| warning.contains("larger")));
    }

    #[test]
    fn missing_explicit_config_is_reported_but_harmless() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.toml");
        let (config, warnings) = Config::load(Some(&path));
        assert_eq!(config, Config::default());
        assert!(!warnings.is_empty());
    }
}
