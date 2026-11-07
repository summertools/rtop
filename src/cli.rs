//! Command-line interface.

use std::path::PathBuf;

use clap::Parser;

use crate::config::{Config, GraphSymbol};

#[derive(Parser, Debug)]
#[command(
    name = "btop-rs",
    version,
    about = "A fast, elegant terminal system monitor",
    long_about = "btop-rs shows CPU, memory, disks, network activity, and processes in a \
                  responsive terminal interface. It is a native Rust implementation inspired \
                  by btop, with a focus on safe parsing, a calm visual design, and predictable \
                  behaviour on Linux, macOS, and Windows."
)]
pub struct Cli {
    /// Start with a preset: 0 overview, 1 CPU, 2 memory, 3 network, 4 processes
    #[arg(short = 'p', long, value_name = "PRESET")]
    pub preset: Option<u8>,

    /// Refresh interval in milliseconds (250-10000)
    #[arg(short = 'u', long, value_name = "MS")]
    pub update: Option<u64>,

    /// Built-in theme name or a .theme file in the themes directory
    #[arg(long, value_name = "THEME")]
    pub theme: Option<String>,

    /// Graph symbols: braille, block, or tty
    #[arg(long, value_name = "STYLE")]
    pub graph_symbol: Option<String>,

    /// Force 16-colour TTY-friendly output
    #[arg(short = 't', long, alias = "tty_on")]
    pub tty: bool,

    /// Quantise colours to the 256-colour palette
    #[arg(long, alias = "low-color")]
    pub low_color: bool,

    /// Disable colour entirely
    #[arg(long)]
    pub no_color: bool,

    /// Do not capture the mouse
    #[arg(long)]
    pub no_mouse: bool,

    /// Disable process signalling
    #[arg(long, alias = "read-only")]
    pub read_only: bool,

    /// Use an explicit configuration file
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Render one plain frame to stdout and exit (useful for tests and scripts)
    #[arg(long)]
    pub dump: bool,

    /// Width used by --dump (default 120)
    #[arg(long, value_name = "COLS", default_value_t = 120)]
    pub width: u16,

    /// Height used by --dump (default 36)
    #[arg(long, value_name = "ROWS", default_value_t = 36)]
    pub height: u16,

    /// List built-in and user themes, then exit
    #[arg(long)]
    pub list_themes: bool,
}

impl Cli {
    /// Apply command-line overrides to a loaded configuration.
    ///
    /// Returns human-readable warnings for invalid values. Command-line
    /// options are allowed to override the file, but never bypass validation.
    #[must_use]
    pub fn apply(&self, config: &mut Config) -> Vec<String> {
        let mut warnings = Vec::new();
        if let Some(preset) = self.preset {
            if preset <= 4 {
                config.preset = preset;
            } else {
                warnings.push(format!("preset {preset} is outside 0..=4; ignoring it"));
            }
        }
        if let Some(update) = self.update {
            config.update_ms = update;
        }
        if let Some(theme) = &self.theme {
            config.theme.clone_from(theme);
        }
        if let Some(symbol) = &self.graph_symbol {
            if let Some(symbol) = GraphSymbol::parse(symbol) {
                config.graph_symbol = symbol;
            } else {
                warnings.push(format!(
                    "unknown graph symbol {symbol:?}; expected braille, block, or tty"
                ));
            }
        }
        if self.tty {
            config.tty_mode = true;
            config.graph_symbol = GraphSymbol::Tty;
            config.rounded_corners = false;
        }
        if self.low_color {
            config.truecolor = false;
        }
        if self.no_mouse {
            config.mouse = false;
        }
        if self.read_only {
            config.read_only = true;
        }
        if self.dump {
            config.mouse = false;
            config.read_only = true;
        }
        warnings.extend(config.validate());
        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli() -> Cli {
        Cli::parse_from(["btop-rs"])
    }

    #[test]
    fn defaults_do_not_change_config() {
        let mut config = Config::default();
        let before = config.clone();
        let warnings = cli().apply(&mut config);
        assert!(warnings.is_empty());
        assert_eq!(config, before);
    }

    #[test]
    fn tty_forces_ascii_graphs() {
        let mut config = Config::default();
        let mut args = cli();
        args.tty = true;
        let _ = args.apply(&mut config);
        assert_eq!(config.graph_symbol, GraphSymbol::Tty);
        assert!(!config.rounded_corners);

        let mut config = Config::default();
        let mut args = cli();
        args.tty = true;
        args.graph_symbol = Some("braille".to_owned());
        let _ = args.apply(&mut config);
        assert_eq!(config.graph_symbol, GraphSymbol::Tty);
    }

    #[test]
    fn invalid_preset_is_ignored_with_warning() {
        let mut config = Config::default();
        let mut args = cli();
        args.preset = Some(9);
        let warnings = args.apply(&mut config);
        assert_eq!(config.preset, 0);
        assert!(warnings.iter().any(|warning| warning.contains("preset 9")));
    }

    #[test]
    fn dump_implies_read_only() {
        let mut config = Config::default();
        let mut args = cli();
        args.dump = true;
        let _ = args.apply(&mut config);
        assert!(config.read_only);
        assert!(!config.mouse);
    }

    #[test]
    fn invalid_theme_is_reported_by_validation() {
        let mut config = Config::default();
        let mut args = cli();
        args.theme = Some("../escape".to_owned());
        let warnings = args.apply(&mut config);
        assert_eq!(config.theme, "default");
        assert!(!warnings.is_empty());
    }
}
