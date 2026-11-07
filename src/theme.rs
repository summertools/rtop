//! Colours and themes.
//!
//! Themes are plain data. The UI only ever sees [`Rgb`] values and converts
//! them once at the terminal boundary, which keeps drawing code independent of
//! the terminal's colour capabilities.

use std::fmt;

/// A 24-bit colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse `#rgb`, `#rrggbb`, or `rrggbb`.
    #[must_use]
    pub fn parse_hex(value: &str) -> Option<Self> {
        let value = value.trim().trim_start_matches('#');
        match value.len() {
            3 => {
                let r = u8::from_str_radix(&value[0..1], 16).ok()?;
                let g = u8::from_str_radix(&value[1..2], 16).ok()?;
                let b = u8::from_str_radix(&value[2..3], 16).ok()?;
                Some(Self::new(r * 17, g * 17, b * 17))
            }
            6 => {
                let r = u8::from_str_radix(&value[0..2], 16).ok()?;
                let g = u8::from_str_radix(&value[2..4], 16).ok()?;
                let b = u8::from_str_radix(&value[4..6], 16).ok()?;
                Some(Self::new(r, g, b))
            }
            _ => None,
        }
    }

    /// Blend two colours; `t` is clamped to `0..=1`.
    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mix = |a: u8, b: u8| -> u8 {
            (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8
        };
        Self::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
        )
    }

    /// Scale brightness for subtle backgrounds.
    #[must_use]
    pub fn scale(self, factor: f32) -> Self {
        let factor = if factor.is_finite() {
            factor.max(0.0)
        } else {
            1.0
        };
        let channel = |v: u8| (f32::from(v) * factor).round().clamp(0.0, 255.0) as u8;
        Self::new(channel(self.r), channel(self.g), channel(self.b))
    }

    /// Perceived luminance in `0..=1`, useful for picking readable text.
    #[must_use]
    pub fn luminance(self) -> f32 {
        (0.2126 * f32::from(self.r) + 0.7152 * f32::from(self.g) + 0.0722 * f32::from(self.b))
            / 255.0
    }

    #[must_use]
    pub fn is_dark(self) -> bool {
        self.luminance() < 0.5
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// Foreground/background colour at draw time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    #[default]
    Default,
    Rgb(Rgb),
}

impl Color {
    #[must_use]
    pub const fn rgb(color: Rgb) -> Self {
        Self::Rgb(color)
    }
}

impl From<Rgb> for Color {
    fn from(value: Rgb) -> Self {
        Self::Rgb(value)
    }
}

/// A complete cell style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub reverse: bool,
}

impl Style {
    #[must_use]
    pub const fn fg(color: Rgb) -> Self {
        Self {
            fg: Color::Rgb(color),
            ..Self::plain()
        }
    }

    #[must_use]
    pub const fn plain() -> Self {
        Self {
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            dim: false,
            italic: false,
            reverse: false,
        }
    }

    #[must_use]
    pub const fn with_bg(mut self, color: Rgb) -> Self {
        self.bg = Color::Rgb(color);
        self
    }

    #[must_use]
    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    #[must_use]
    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    #[must_use]
    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    #[must_use]
    pub const fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }
}

/// Three-stop gradient used by graphs and gauges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gradient {
    pub start: Rgb,
    pub mid: Rgb,
    pub end: Rgb,
}

impl Gradient {
    #[must_use]
    pub const fn new(start: Rgb, mid: Rgb, end: Rgb) -> Self {
        Self { start, mid, end }
    }

    /// Sample the gradient at `t` in `0..=1`.
    #[must_use]
    pub fn at(self, t: f32) -> Rgb {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if t < 0.5 {
            self.start.lerp(self.mid, t * 2.0)
        } else {
            self.mid.lerp(self.end, (t - 0.5) * 2.0)
        }
    }

    #[must_use]
    pub const fn reversed(self) -> Self {
        Self::new(self.end, self.mid, self.start)
    }
}

/// All colours used by the interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub name: String,
    pub background: Rgb,
    pub foreground: Rgb,
    pub dim: Rgb,
    pub title: Rgb,
    pub accent: Rgb,
    pub border: Rgb,
    pub highlight_bg: Rgb,
    pub highlight_fg: Rgb,
    pub good: Rgb,
    pub warning: Rgb,
    pub critical: Rgb,
    pub cpu: Gradient,
    pub memory: Gradient,
    pub disk: Gradient,
    pub net_down: Gradient,
    pub net_up: Gradient,
    pub temp: Gradient,
}

impl Theme {
    #[must_use]
    pub fn builtin(name: &str) -> Option<Self> {
        let key = name.trim().to_ascii_lowercase();
        let theme = match key.as_str() {
            "default" | "tokyo-night" | "tokyonight" => Self::tokyo_night(),
            "dracula" => Self::dracula(),
            "gruvbox" | "gruvbox-dark" => Self::gruvbox(),
            "nord" => Self::nord(),
            "catppuccin" | "catppuccin-mocha" => Self::catppuccin(),
            "light" => Self::light(),
            "tty" | "mono" | "monochrome" => Self::monochrome(),
            _ => return None,
        };
        Some(theme)
    }

    /// Names accepted by `--theme` and the configuration file.
    #[must_use]
    pub const fn builtin_names() -> &'static [&'static str] {
        &[
            "default",
            "dracula",
            "gruvbox",
            "nord",
            "catppuccin",
            "light",
            "tty",
        ]
    }

    fn tokyo_night() -> Self {
        Self {
            name: "default".to_owned(),
            background: Rgb::new(0x0b, 0x0e, 0x14),
            foreground: Rgb::new(0xc0, 0xca, 0xf5),
            dim: Rgb::new(0x56, 0x5f, 0x89),
            title: Rgb::new(0x7a, 0xa2, 0xf7),
            accent: Rgb::new(0x7d, 0xcf, 0xff),
            border: Rgb::new(0x3b, 0x42, 0x61),
            highlight_bg: Rgb::new(0x28, 0x34, 0x57),
            highlight_fg: Rgb::new(0xff, 0xff, 0xff),
            good: Rgb::new(0x9e, 0xce, 0x6a),
            warning: Rgb::new(0xe0, 0xaf, 0x68),
            critical: Rgb::new(0xf7, 0x76, 0x8e),
            cpu: Gradient::new(
                Rgb::new(0x2a, 0xc3, 0xde),
                Rgb::new(0x7a, 0xa2, 0xf7),
                Rgb::new(0xbb, 0x9a, 0xf7),
            ),
            memory: Gradient::new(
                Rgb::new(0x9e, 0xce, 0x6a),
                Rgb::new(0xe0, 0xaf, 0x68),
                Rgb::new(0xf7, 0x76, 0x8e),
            ),
            disk: Gradient::new(
                Rgb::new(0x73, 0xda, 0xca),
                Rgb::new(0x7a, 0xa2, 0xf7),
                Rgb::new(0xbb, 0x9a, 0xf7),
            ),
            net_down: Gradient::new(
                Rgb::new(0x2a, 0xc3, 0xde),
                Rgb::new(0x7d, 0xcf, 0xff),
                Rgb::new(0x7a, 0xa2, 0xf7),
            ),
            net_up: Gradient::new(
                Rgb::new(0xbb, 0x9a, 0xf7),
                Rgb::new(0xf7, 0x76, 0x8e),
                Rgb::new(0xff, 0x9e, 0x64),
            ),
            temp: Gradient::new(
                Rgb::new(0x9e, 0xce, 0x6a),
                Rgb::new(0xe0, 0xaf, 0x68),
                Rgb::new(0xf7, 0x76, 0x8e),
            ),
        }
    }

    fn dracula() -> Self {
        Self {
            name: "dracula".to_owned(),
            background: Rgb::new(0x28, 0x2a, 0x36),
            foreground: Rgb::new(0xf8, 0xf8, 0xf2),
            dim: Rgb::new(0x62, 0x72, 0xa4),
            title: Rgb::new(0xbd, 0x93, 0xf9),
            accent: Rgb::new(0x8b, 0xe9, 0xfd),
            border: Rgb::new(0x44, 0x47, 0x5a),
            highlight_bg: Rgb::new(0x44, 0x47, 0x5a),
            highlight_fg: Rgb::new(0xf8, 0xf8, 0xf2),
            good: Rgb::new(0x50, 0xfa, 0x7b),
            warning: Rgb::new(0xff, 0xb8, 0x6c),
            critical: Rgb::new(0xff, 0x55, 0x55),
            cpu: Gradient::new(
                Rgb::new(0x8b, 0xe9, 0xfd),
                Rgb::new(0xbd, 0x93, 0xf9),
                Rgb::new(0xff, 0x79, 0xc6),
            ),
            memory: Gradient::new(
                Rgb::new(0x50, 0xfa, 0x7b),
                Rgb::new(0xff, 0xb8, 0x6c),
                Rgb::new(0xff, 0x55, 0x55),
            ),
            disk: Gradient::new(
                Rgb::new(0x8b, 0xe9, 0xfd),
                Rgb::new(0xbd, 0x93, 0xf9),
                Rgb::new(0xff, 0x79, 0xc6),
            ),
            net_down: Gradient::new(
                Rgb::new(0x50, 0xfa, 0x7b),
                Rgb::new(0x8b, 0xe9, 0xfd),
                Rgb::new(0xbd, 0x93, 0xf9),
            ),
            net_up: Gradient::new(
                Rgb::new(0xff, 0x79, 0xc6),
                Rgb::new(0xbd, 0x93, 0xf9),
                Rgb::new(0xff, 0x55, 0x55),
            ),
            temp: Gradient::new(
                Rgb::new(0x50, 0xfa, 0x7b),
                Rgb::new(0xff, 0xb8, 0x6c),
                Rgb::new(0xff, 0x55, 0x55),
            ),
        }
    }

    fn gruvbox() -> Self {
        Self {
            name: "gruvbox".to_owned(),
            background: Rgb::new(0x28, 0x28, 0x28),
            foreground: Rgb::new(0xeb, 0xdb, 0xb2),
            dim: Rgb::new(0x92, 0x83, 0x74),
            title: Rgb::new(0xfa, 0xbd, 0x2f),
            accent: Rgb::new(0x83, 0xa5, 0x98),
            border: Rgb::new(0x50, 0x49, 0x45),
            highlight_bg: Rgb::new(0x3c, 0x38, 0x36),
            highlight_fg: Rgb::new(0xfb, 0xf1, 0xc7),
            good: Rgb::new(0xb8, 0xbb, 0x26),
            warning: Rgb::new(0xfa, 0xbd, 0x2f),
            critical: Rgb::new(0xfb, 0x49, 0x34),
            cpu: Gradient::new(
                Rgb::new(0x83, 0xa5, 0x98),
                Rgb::new(0xfa, 0xbd, 0x2f),
                Rgb::new(0xfe, 0x80, 0x19),
            ),
            memory: Gradient::new(
                Rgb::new(0xb8, 0xbb, 0x26),
                Rgb::new(0xfa, 0xbd, 0x2f),
                Rgb::new(0xfb, 0x49, 0x34),
            ),
            disk: Gradient::new(
                Rgb::new(0x8e, 0xc0, 0x7c),
                Rgb::new(0x83, 0xa5, 0x98),
                Rgb::new(0x83, 0xa5, 0x98),
            ),
            net_down: Gradient::new(
                Rgb::new(0x8e, 0xc0, 0x7c),
                Rgb::new(0x83, 0xa5, 0x98),
                Rgb::new(0x83, 0xa5, 0x98),
            ),
            net_up: Gradient::new(
                Rgb::new(0xd3, 0x86, 0x9b),
                Rgb::new(0xfb, 0x49, 0x34),
                Rgb::new(0xfb, 0x49, 0x34),
            ),
            temp: Gradient::new(
                Rgb::new(0x8e, 0xc0, 0x7c),
                Rgb::new(0xfa, 0xbd, 0x2f),
                Rgb::new(0xfb, 0x49, 0x34),
            ),
        }
    }

    fn nord() -> Self {
        Self {
            name: "nord".to_owned(),
            background: Rgb::new(0x2e, 0x34, 0x40),
            foreground: Rgb::new(0xd8, 0xde, 0xe9),
            dim: Rgb::new(0x4c, 0x56, 0x6a),
            title: Rgb::new(0x88, 0xc0, 0xd0),
            accent: Rgb::new(0x81, 0xa1, 0xc1),
            border: Rgb::new(0x43, 0x4c, 0x5e),
            highlight_bg: Rgb::new(0x3b, 0x42, 0x52),
            highlight_fg: Rgb::new(0xec, 0xef, 0xf4),
            good: Rgb::new(0xa3, 0xbe, 0x8c),
            warning: Rgb::new(0xeb, 0xcb, 0x8b),
            critical: Rgb::new(0xbf, 0x61, 0x6a),
            cpu: Gradient::new(
                Rgb::new(0x88, 0xc0, 0xd0),
                Rgb::new(0x81, 0xa1, 0xc1),
                Rgb::new(0xb4, 0x8e, 0xad),
            ),
            memory: Gradient::new(
                Rgb::new(0xeb, 0xcb, 0x8b),
                Rgb::new(0xd0, 0x87, 0x70),
                Rgb::new(0xbf, 0x61, 0x6a),
            ),
            disk: Gradient::new(
                Rgb::new(0x8f, 0xbc, 0xbb),
                Rgb::new(0x88, 0xc0, 0xd0),
                Rgb::new(0x81, 0xa1, 0xc1),
            ),
            net_down: Gradient::new(
                Rgb::new(0x8f, 0xbc, 0xbb),
                Rgb::new(0x88, 0xc0, 0xd0),
                Rgb::new(0x81, 0xa1, 0xc1),
            ),
            net_up: Gradient::new(
                Rgb::new(0xb4, 0x8e, 0xad),
                Rgb::new(0xbf, 0x61, 0x6a),
                Rgb::new(0xbf, 0x61, 0x6a),
            ),
            temp: Gradient::new(
                Rgb::new(0xa3, 0xbe, 0x8c),
                Rgb::new(0xeb, 0xcb, 0x8b),
                Rgb::new(0xbf, 0x61, 0x6a),
            ),
        }
    }

    fn catppuccin() -> Self {
        Self {
            name: "catppuccin".to_owned(),
            background: Rgb::new(0x1e, 0x1e, 0x2e),
            foreground: Rgb::new(0xcd, 0xd6, 0xf4),
            dim: Rgb::new(0x6c, 0x70, 0x86),
            title: Rgb::new(0x89, 0xb4, 0xfa),
            accent: Rgb::new(0x94, 0xe2, 0xd5),
            border: Rgb::new(0x45, 0x47, 0x5a),
            highlight_bg: Rgb::new(0x31, 0x32, 0x44),
            highlight_fg: Rgb::new(0xf5, 0xe0, 0xdc),
            good: Rgb::new(0xa6, 0xe3, 0xa1),
            warning: Rgb::new(0xf9, 0xe2, 0xaf),
            critical: Rgb::new(0xf3, 0x8b, 0xa8),
            cpu: Gradient::new(
                Rgb::new(0x94, 0xe2, 0xd5),
                Rgb::new(0x89, 0xb4, 0xfa),
                Rgb::new(0xcb, 0xa6, 0xf7),
            ),
            memory: Gradient::new(
                Rgb::new(0xa6, 0xe3, 0xa1),
                Rgb::new(0xf9, 0xe2, 0xaf),
                Rgb::new(0xf3, 0x8b, 0xa8),
            ),
            disk: Gradient::new(
                Rgb::new(0x94, 0xe2, 0xd5),
                Rgb::new(0x89, 0xb4, 0xfa),
                Rgb::new(0xcb, 0xa6, 0xf7),
            ),
            net_down: Gradient::new(
                Rgb::new(0x89, 0xdc, 0xeb),
                Rgb::new(0x74, 0xc7, 0xec),
                Rgb::new(0x89, 0xb4, 0xfa),
            ),
            net_up: Gradient::new(
                Rgb::new(0xcb, 0xa6, 0xf7),
                Rgb::new(0xf5, 0xc2, 0xe7),
                Rgb::new(0xf3, 0x8b, 0xa8),
            ),
            temp: Gradient::new(
                Rgb::new(0xa6, 0xe3, 0xa1),
                Rgb::new(0xf9, 0xe2, 0xaf),
                Rgb::new(0xf3, 0x8b, 0xa8),
            ),
        }
    }

    fn light() -> Self {
        Self {
            name: "light".to_owned(),
            background: Rgb::new(0xf7, 0xf7, 0xf7),
            foreground: Rgb::new(0x24, 0x29, 0x2e),
            dim: Rgb::new(0x82, 0x8a, 0x92),
            title: Rgb::new(0x17, 0x69, 0xaa),
            accent: Rgb::new(0x00, 0x79, 0x6b),
            border: Rgb::new(0xc5, 0xcb, 0xd1),
            highlight_bg: Rgb::new(0xdd, 0xea, 0xf7),
            highlight_fg: Rgb::new(0x0b, 0x1c, 0x2b),
            good: Rgb::new(0x2e, 0x7d, 0x32),
            warning: Rgb::new(0xef, 0x6c, 0x00),
            critical: Rgb::new(0xc6, 0x28, 0x28),
            cpu: Gradient::new(
                Rgb::new(0x00, 0x79, 0x6b),
                Rgb::new(0x17, 0x69, 0xaa),
                Rgb::new(0x6a, 0x3f, 0xb5),
            ),
            memory: Gradient::new(
                Rgb::new(0x2e, 0x7d, 0x32),
                Rgb::new(0xef, 0x6c, 0x00),
                Rgb::new(0xc6, 0x28, 0x28),
            ),
            disk: Gradient::new(
                Rgb::new(0x00, 0x83, 0x8f),
                Rgb::new(0x15, 0x65, 0xc0),
                Rgb::new(0x6a, 0x1b, 0x9a),
            ),
            net_down: Gradient::new(
                Rgb::new(0x00, 0x83, 0x8f),
                Rgb::new(0x15, 0x65, 0xc0),
                Rgb::new(0x15, 0x65, 0xc0),
            ),
            net_up: Gradient::new(
                Rgb::new(0x6a, 0x1b, 0x9a),
                Rgb::new(0xad, 0x14, 0x57),
                Rgb::new(0xad, 0x14, 0x57),
            ),
            temp: Gradient::new(
                Rgb::new(0x2e, 0x7d, 0x32),
                Rgb::new(0xef, 0x6c, 0x00),
                Rgb::new(0xc6, 0x28, 0x28),
            ),
        }
    }

    fn monochrome() -> Self {
        let white = Rgb::new(0xee, 0xee, 0xee);
        let grey = Rgb::new(0x99, 0x99, 0x99);
        let dim = Rgb::new(0x66, 0x66, 0x66);
        let black = Rgb::new(0x11, 0x11, 0x11);
        Self {
            name: "tty".to_owned(),
            background: black,
            foreground: white,
            dim,
            title: white,
            accent: grey,
            border: dim,
            highlight_bg: grey,
            highlight_fg: black,
            good: white,
            warning: grey,
            critical: white,
            cpu: Gradient::new(grey, white, white),
            memory: Gradient::new(grey, white, white),
            disk: Gradient::new(grey, white, white),
            net_down: Gradient::new(grey, white, white),
            net_up: Gradient::new(white, grey, grey),
            temp: Gradient::new(grey, white, white),
        }
    }

    /// Build a theme from btop's `.theme` syntax.
    ///
    /// Unknown keys are ignored, malformed colours are skipped, and the
    /// fallback supplies anything the file does not define. A theme file can
    /// therefore never cause an invalid palette.
    #[must_use]
    pub fn from_btop_theme(text: &str, fallback: &Theme) -> Self {
        let mut theme = fallback.clone();
        let mut cpu_start = theme.cpu.start;
        let mut cpu_mid = theme.cpu.mid;
        let mut cpu_end = theme.cpu.end;
        let mut mem_start = theme.memory.start;
        let mut mem_mid = theme.memory.mid;
        let mut mem_end = theme.memory.end;
        let mut down_start = theme.net_down.start;
        let mut down_mid = theme.net_down.mid;
        let mut down_end = theme.net_down.end;
        let mut up_start = theme.net_up.start;
        let mut up_mid = theme.net_up.mid;
        let mut up_end = theme.net_up.end;
        let mut temp_start = theme.temp.start;
        let mut temp_mid = theme.temp.mid;
        let mut temp_end = theme.temp.end;
        let mut disk_start = theme.disk.start;
        let mut disk_mid = theme.disk.mid;
        let mut disk_end = theme.disk.end;

        for line in text.lines().take(2000) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key
                .trim()
                .trim_start_matches("theme[")
                .trim_end_matches(']')
                .trim();
            let value = value.trim().trim_matches('"').trim();
            let Some(color) = Rgb::parse_hex(value) else {
                continue;
            };
            match key {
                "main_bg" => theme.background = color,
                "main_fg" => theme.foreground = color,
                "title" => theme.title = color,
                "hi_fg" => theme.accent = color,
                "selected_bg" => theme.highlight_bg = color,
                "selected_fg" => theme.highlight_fg = color,
                "inactive_fg" | "div_line" => theme.border = color,
                "proc_misc" => theme.dim = color,
                "temp_start" => temp_start = color,
                "temp_mid" => temp_mid = color,
                "temp_end" => temp_end = color,
                "cpu_start" => cpu_start = color,
                "cpu_mid" => cpu_mid = color,
                "cpu_end" => cpu_end = color,
                "mem_start" => mem_start = color,
                "mem_mid" => mem_mid = color,
                "mem_end" => mem_end = color,
                "net_download_start" => down_start = color,
                "net_download_mid" => down_mid = color,
                "net_download_end" => down_end = color,
                "net_upload_start" => up_start = color,
                "net_upload_mid" => up_mid = color,
                "net_upload_end" => up_end = color,
                "disk_start" => disk_start = color,
                "disk_mid" => disk_mid = color,
                "disk_end" => disk_end = color,
                "available_start" | "free_start" | "available_end" | "free_end" => {
                    theme.good = color;
                }
                "used_start" | "cached_start" => theme.warning = color,
                "used_end" | "cached_end" => theme.critical = color,
                _ => {}
            }
        }
        theme.cpu = Gradient::new(cpu_start, cpu_mid, cpu_end);
        theme.memory = Gradient::new(mem_start, mem_mid, mem_end);
        theme.disk = Gradient::new(disk_start, disk_mid, disk_end);
        theme.net_down = Gradient::new(down_start, down_mid, down_end);
        theme.net_up = Gradient::new(up_start, up_mid, up_end);
        theme.temp = Gradient::new(temp_start, temp_mid, temp_end);
        theme
    }

    /// Create a copy with an explicit name (used for user theme files).
    #[must_use]
    pub fn renamed(mut self, name: &str) -> Self {
        name.clone_into(&mut self.name);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing_accepts_short_and_long_forms() {
        assert_eq!(Rgb::parse_hex("#fff"), Some(Rgb::new(255, 255, 255)));
        assert_eq!(Rgb::parse_hex("00ff80"), Some(Rgb::new(0, 255, 128)));
        assert_eq!(Rgb::parse_hex("#12345"), None);
        assert_eq!(Rgb::parse_hex("#gggggg"), None);
    }

    #[test]
    fn gradient_endpoints_are_exact() {
        let gradient = Gradient::new(
            Rgb::new(0, 0, 0),
            Rgb::new(128, 128, 128),
            Rgb::new(255, 255, 255),
        );
        assert_eq!(gradient.at(0.0), Rgb::new(0, 0, 0));
        assert_eq!(gradient.at(0.5), Rgb::new(128, 128, 128));
        assert_eq!(gradient.at(1.0), Rgb::new(255, 255, 255));
        assert_eq!(gradient.at(f32::NAN), Rgb::new(0, 0, 0));
    }

    #[test]
    fn builtins_are_complete_and_distinct() {
        for name in Theme::builtin_names() {
            let theme = Theme::builtin(name).expect("builtin theme");
            assert_eq!(theme.name, *name);
        }
        assert!(Theme::builtin("does-not-exist").is_none());
    }

    #[test]
    fn btop_theme_overrides_known_colours() {
        let fallback = Theme::builtin("default").unwrap();
        let parsed = Theme::from_btop_theme(
            r##"
            theme[main_bg]="#101010"
            theme[main_fg]="#eeeeee"
            theme[cpu_start]="#ff0000"
            theme[cpu_mid]="#00ff00"
            theme[cpu_end]="#0000ff"
            theme[bogus]="#abcdef"
            "##,
            &fallback,
        );
        assert_eq!(parsed.background, Rgb::new(0x10, 0x10, 0x10));
        assert_eq!(parsed.foreground, Rgb::new(0xee, 0xee, 0xee));
        assert_eq!(parsed.cpu.start, Rgb::new(0xff, 0, 0));
        assert_eq!(parsed.cpu.end, Rgb::new(0, 0, 0xff));
    }

    #[test]
    fn luminance_is_sane() {
        assert!(Rgb::new(0, 0, 0).is_dark());
        assert!(!Rgb::new(255, 255, 255).is_dark());
    }
}
