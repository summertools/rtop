//! Terminal lifecycle and the diff renderer.
//!
//! Crossterm handles the platform differences (ANSI on Unix, VT sequences on
//! Windows). The renderer keeps a front buffer so only changed cells are
//! written, which keeps redraws flicker-free even on slow terminals.

use std::io::{self, IsTerminal, Write, stdout};
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::style::{
    Attribute, Color as CtColor, Print, ResetColor, SetAttribute, SetBackgroundColor,
    SetForegroundColor,
};
use crossterm::terminal::{
    Clear, ClearType, DisableLineWrap, EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen,
    disable_raw_mode, enable_raw_mode,
};
use crossterm::{execute, queue};

use crate::theme::{Color, Rgb, Style};
use crate::ui::canvas::Canvas;

static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Terminal colour capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    TrueColor,
    Ansi256,
    Ansi16,
    None,
}

impl ColorMode {
    /// Choose a mode from the environment and explicit CLI switches.
    #[must_use]
    pub fn detect(truecolor: bool, tty_mode: bool, no_color: bool) -> Self {
        if no_color
            || std::env::var_os("NO_COLOR").is_some()
            || std::env::var("CLICOLOR").is_ok_and(|value| value == "0")
        {
            return Self::None;
        }
        let term = std::env::var("TERM").unwrap_or_default();
        if term.eq_ignore_ascii_case("dumb") {
            return Self::None;
        }
        if tty_mode {
            return Self::Ansi16;
        }
        let colorterm = std::env::var("COLORTERM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let truecolor_env = colorterm.contains("truecolor")
            || colorterm.contains("24bit")
            || std::env::var_os("WT_SESSION").is_some()
            || std::env::var("TERM_PROGRAM").is_ok_and(|value| {
                value.eq_ignore_ascii_case("iTerm.app")
                    || value.eq_ignore_ascii_case("WezTerm")
                    || value.eq_ignore_ascii_case("vscode")
            });
        if truecolor && truecolor_env {
            Self::TrueColor
        } else if term.contains("256color") || !colorterm.is_empty() {
            Self::Ansi256
        } else {
            Self::Ansi16
        }
    }
}

/// RAII terminal state. Restores raw mode, the alternate screen, the cursor,
/// mouse capture, and bracketed paste even when the application panics.
#[derive(Debug)]
pub struct TerminalGuard {
    mouse: bool,
}

impl TerminalGuard {
    pub fn enter(mouse: bool) -> io::Result<Self> {
        if !stdout().is_terminal() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "stdout is not a terminal",
            ));
        }
        enable_raw_mode()?;
        let mut out = stdout();
        let result = execute!(
            out,
            EnterAlternateScreen,
            Hide,
            DisableLineWrap,
            Clear(ClearType::All),
            EnableBracketedPaste
        );
        if result.is_err() {
            let _ = execute!(
                out,
                Show,
                EnableLineWrap,
                DisableBracketedPaste,
                LeaveAlternateScreen
            );
            let _ = disable_raw_mode();
            return result.map(|()| Self { mouse });
        }
        if mouse {
            let _ = execute!(out, EnableMouseCapture);
        }
        TERMINAL_ACTIVE.store(true, Ordering::SeqCst);
        Ok(Self { mouse })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal(self.mouse);
    }
}

/// Best-effort restore used from the panic hook and normal shutdown.
pub fn restore_terminal(mouse: bool) {
    if !TERMINAL_ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut out = stdout();
    let _ = execute!(
        out,
        Show,
        ResetColor,
        SetAttribute(Attribute::Reset),
        DisableBracketedPaste,
        EnableLineWrap,
        LeaveAlternateScreen
    );
    if mouse {
        let _ = execute!(out, DisableMouseCapture);
    }
    let _ = disable_raw_mode();
    let _ = out.flush();
}

/// Restore without knowing whether mouse capture was enabled.
pub fn restore_terminal_best_effort() {
    if TERMINAL_ACTIVE.swap(false, Ordering::SeqCst) {
        let mut out = stdout();
        let _ = execute!(
            out,
            Show,
            ResetColor,
            SetAttribute(Attribute::Reset),
            DisableBracketedPaste,
            DisableMouseCapture,
            EnableLineWrap,
            LeaveAlternateScreen
        );
        let _ = disable_raw_mode();
        let _ = out.flush();
    }
}

#[derive(Default)]
struct AppliedStyle {
    fg: Option<CtColor>,
    bg: Option<CtColor>,
    bold: bool,
    dim: bool,
    italic: bool,
    reverse: bool,
    valid: bool,
}

#[derive(Debug)]
struct Renderer {
    front: Option<Canvas>,
    mode: ColorMode,
    background: Option<Rgb>,
}

impl Renderer {
    fn new(mode: ColorMode, background: Option<Rgb>) -> Self {
        Self {
            front: None,
            mode,
            background,
        }
    }

    fn draw(&mut self, canvas: &Canvas) -> io::Result<()> {
        let mut out = io::BufWriter::with_capacity(64 * 1024, stdout().lock());
        let resized = self.front.as_ref().is_none_or(|front| {
            front.width() != canvas.width() || front.height() != canvas.height()
        });
        if resized {
            queue!(out, Clear(ClearType::All))?;
            self.front = None;
        }
        let mut applied = AppliedStyle::default();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                let cell = canvas.cell(x, y);
                if cell.continuation {
                    continue;
                }
                if self
                    .front
                    .as_ref()
                    .is_some_and(|front| front.cell(x, y) == cell)
                {
                    continue;
                }
                queue!(out, MoveTo(x, y))?;
                apply_style(
                    &mut out,
                    &mut applied,
                    cell.style,
                    self.mode,
                    self.background,
                )?;
                queue!(out, Print(cell.ch))?;
            }
        }
        out.flush()?;
        self.front = Some(canvas.clone());
        Ok(())
    }
}

/// Draw frames until the caller stops. Owned by `main`; exposed here so the
/// terminal setup and drawing remain one module.
#[derive(Debug)]
pub struct Screen {
    renderer: Renderer,
}

impl Screen {
    #[must_use]
    pub fn new(mode: ColorMode, background: Option<Rgb>) -> Self {
        Self {
            renderer: Renderer::new(mode, background),
        }
    }

    pub fn draw(&mut self, canvas: &Canvas) -> io::Result<()> {
        self.renderer.draw(canvas)
    }
}

fn apply_style(
    out: &mut impl Write,
    applied: &mut AppliedStyle,
    style: Style,
    mode: ColorMode,
    theme_background: Option<Rgb>,
) -> io::Result<()> {
    let (mut fg, mut bg) = match mode {
        ColorMode::TrueColor => (to_crossterm(style.fg), to_crossterm(style.bg)),
        ColorMode::Ansi256 => (
            quantize256(style.fg).map_or(CtColor::Reset, CtColor::AnsiValue),
            quantize256(style.bg).map_or(CtColor::Reset, CtColor::AnsiValue),
        ),
        ColorMode::Ansi16 => (
            quantize16(style.fg).map_or(CtColor::Reset, CtColor::AnsiValue),
            quantize16(style.bg).map_or(CtColor::Reset, CtColor::AnsiValue),
        ),
        ColorMode::None => (CtColor::Reset, CtColor::Reset),
    };
    let mut reverse = style.reverse;
    if mode == ColorMode::None {
        // Without colour, a distinct background becomes reverse video so
        // selections and badges stay visible. The theme background itself is
        // not a selection and must not paint the whole screen.
        let is_theme_background = match (style.bg, theme_background) {
            (Color::Rgb(bg), Some(theme_bg)) => bg == theme_bg,
            _ => false,
        };
        if style.bg != Color::Default && !is_theme_background {
            reverse = true;
        }
        fg = CtColor::Reset;
        bg = CtColor::Reset;
    }

    let modifiers_changed = !applied.valid
        || applied.bold != style.bold
        || applied.dim != style.dim
        || applied.italic != style.italic
        || applied.reverse != reverse;
    if modifiers_changed {
        queue!(out, SetAttribute(Attribute::Reset))?;
        applied.fg = None;
        applied.bg = None;
        if style.bold {
            queue!(out, SetAttribute(Attribute::Bold))?;
        }
        if style.dim {
            queue!(out, SetAttribute(Attribute::Dim))?;
        }
        if style.italic {
            queue!(out, SetAttribute(Attribute::Italic))?;
        }
        if reverse {
            queue!(out, SetAttribute(Attribute::Reverse))?;
        }
        applied.bold = style.bold;
        applied.dim = style.dim;
        applied.italic = style.italic;
        applied.reverse = reverse;
        applied.valid = true;
    }
    if applied.fg != Some(fg) {
        queue!(out, SetForegroundColor(fg))?;
        applied.fg = Some(fg);
    }
    if applied.bg != Some(bg) {
        queue!(out, SetBackgroundColor(bg))?;
        applied.bg = Some(bg);
    }
    Ok(())
}

fn to_crossterm(color: Color) -> CtColor {
    match color {
        Color::Default => CtColor::Reset,
        Color::Rgb(rgb) => CtColor::Rgb {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        },
    }
}

/// The standard xterm 16-colour palette, used for the low-colour fallback.
const PALETTE: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (128, 0, 0),
    (0, 128, 0),
    (128, 128, 0),
    (0, 0, 128),
    (128, 0, 128),
    (0, 128, 128),
    (192, 192, 192),
    (128, 128, 128),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (0, 0, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

fn quantize256(color: Color) -> Option<u8> {
    let Color::Rgb(rgb) = color else {
        return None;
    };
    let max = rgb.r.max(rgb.g).max(rgb.b);
    let min = rgb.r.min(rgb.g).min(rgb.b);
    if max.saturating_sub(min) < 12 {
        return Some(if max < 8 {
            16
        } else if max > 248 {
            231
        } else {
            232 + ((u16::from(max) - 8) * 24 / 247) as u8
        });
    }
    let level = |value: u8| -> u16 { (u16::from(value) * 5 + 127) / 255 };
    Some((16 + 36 * level(rgb.r) + 6 * level(rgb.g) + level(rgb.b)) as u8)
}

fn quantize16(color: Color) -> Option<u8> {
    let Color::Rgb(rgb) = color else {
        return None;
    };
    let mut best = 0u8;
    let mut best_distance = u32::MAX;
    for (index, (r, g, b)) in PALETTE.iter().enumerate() {
        let dr = i32::from(rgb.r) - i32::from(*r);
        let dg = i32::from(rgb.g) - i32::from(*g);
        let db = i32::from(rgb.b) - i32::from(*b);
        let distance = (dr * dr + dg * dg + db * db) as u32;
        if distance < best_distance {
            best_distance = distance;
            best = index as u8;
        }
    }
    Some(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monochrome_background_becomes_reverse() {
        let mut out = Vec::new();
        let mut applied = AppliedStyle::default();
        apply_style(
            &mut out,
            &mut applied,
            Style {
                fg: Color::Rgb(Rgb::new(255, 255, 255)),
                bg: Color::Rgb(Rgb::new(20, 20, 20)),
                bold: true,
                ..Style::plain()
            },
            ColorMode::None,
            Some(Rgb::new(20, 20, 20)),
        )
        .unwrap();
        assert!(!applied.reverse);
        let mut out = Vec::new();
        apply_style(
            &mut out,
            &mut applied,
            Style {
                fg: Color::Rgb(Rgb::new(0, 0, 0)),
                bg: Color::Rgb(Rgb::new(200, 200, 200)),
                ..Style::plain()
            },
            ColorMode::None,
            Some(Rgb::new(20, 20, 20)),
        )
        .unwrap();
        assert!(applied.reverse);
    }

    #[test]
    fn ansi_quantization_is_in_range() {
        for rgb in [
            Rgb::new(0, 0, 0),
            Rgb::new(255, 255, 255),
            Rgb::new(255, 0, 0),
            Rgb::new(18, 52, 86),
        ] {
            let index = quantize256(Color::Rgb(rgb)).unwrap();
            assert!(index >= 16);
            assert!(quantize16(Color::Rgb(rgb)).unwrap() < 16);
        }
    }

    #[test]
    fn detection_respects_no_color() {
        assert_eq!(ColorMode::detect(true, false, true), ColorMode::None);
    }
}
