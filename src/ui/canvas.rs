//! A small, dependency-free cell buffer with clipping and wide-character
//! handling.
//!
//! The renderer draws into a [`Canvas`] and diffs it against the previous
//! frame, so the terminal only receives cells that actually changed. All
//! drawing is bounds checked; a malformed layout can never write outside the
//! screen or panic.

use unicode_width::UnicodeWidthChar;

use crate::format;
use crate::theme::{Color, Style};

/// A rectangle in terminal cells. `right` and `bottom` are exclusive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    #[must_use]
    pub const fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }
    #[must_use]
    pub const fn right(self) -> u16 {
        self.x.saturating_add(self.w)
    }

    #[must_use]
    pub const fn bottom(self) -> u16 {
        self.y.saturating_add(self.h)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.w == 0 || self.h == 0
    }

    #[must_use]
    pub const fn contains(self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// Shrink on every side, stopping at zero.
    #[must_use]
    pub const fn inset(self, dx: u16, dy: u16) -> Self {
        let w = self.w.saturating_sub(dx.saturating_mul(2));
        let h = self.h.saturating_sub(dy.saturating_mul(2));
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
            w,
            h,
        }
    }

    /// Split off `amount` rows from the top.
    #[must_use]
    pub const fn split_top(self, amount: u16) -> (Self, Self) {
        let amount = if amount > self.h { self.h } else { amount };
        (
            Self::new(self.x, self.y, self.w, amount),
            Self::new(
                self.x,
                self.y.saturating_add(amount),
                self.w,
                self.h - amount,
            ),
        )
    }

    /// Split off `amount` rows from the bottom.
    #[must_use]
    pub const fn split_bottom(self, amount: u16) -> (Self, Self) {
        let amount = if amount > self.h { self.h } else { amount };
        let top_h = self.h - amount;
        (
            Self::new(self.x, self.y, self.w, top_h),
            Self::new(self.x, self.y.saturating_add(top_h), self.w, amount),
        )
    }

    /// Split off `amount` columns from the left.
    #[must_use]
    pub const fn split_left(self, amount: u16) -> (Self, Self) {
        let amount = if amount > self.w { self.w } else { amount };
        (
            Self::new(self.x, self.y, amount, self.h),
            Self::new(
                self.x.saturating_add(amount),
                self.y,
                self.w - amount,
                self.h,
            ),
        )
    }

    /// Split off `amount` columns from the right.
    #[must_use]
    pub const fn split_right(self, amount: u16) -> (Self, Self) {
        let amount = if amount > self.w { self.w } else { amount };
        let left_w = self.w - amount;
        (
            Self::new(self.x, self.y, left_w, self.h),
            Self::new(self.x.saturating_add(left_w), self.y, amount, self.h),
        )
    }

    /// Split `count` columns evenly, distributing the remainder to the left.
    #[must_use]
    pub fn columns(self, count: u16) -> Vec<Self> {
        if count == 0 {
            return Vec::new();
        }
        let base = self.w / count;
        let extra = self.w % count;
        let mut out = Vec::with_capacity(count as usize);
        let mut x = self.x;
        for index in 0..count {
            let width = base + u16::from(index < extra);
            out.push(Self::new(x, self.y, width, self.h));
            x = x.saturating_add(width);
        }
        out
    }

    /// Split `count` rows evenly, distributing the remainder to the top.
    #[must_use]
    pub fn rows(self, count: u16) -> Vec<Self> {
        if count == 0 {
            return Vec::new();
        }
        let base = self.h / count;
        let extra = self.h % count;
        let mut out = Vec::with_capacity(count as usize);
        let mut y = self.y;
        for index in 0..count {
            let height = base + u16::from(index < extra);
            out.push(Self::new(self.x, y, self.w, height));
            y = y.saturating_add(height);
        }
        out
    }

    /// Intersection with another rectangle.
    #[must_use]
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Self::new(x, y, right.saturating_sub(x), bottom.saturating_sub(y))
    }

    /// Centre a box of the given size inside this rectangle.
    #[must_use]
    pub fn center(self, width: u16, height: u16) -> Self {
        let width = width.min(self.w);
        let height = height.min(self.h);
        Self::new(
            self.x + (self.w - width) / 2,
            self.y + (self.h - height) / 2,
            width,
            height,
        )
    }
}

/// One terminal cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
    /// True for the second half of a double-width character.
    pub continuation: bool,
}

impl Cell {
    #[must_use]
    pub const fn blank(style: Style) -> Self {
        Self {
            ch: ' ',
            style,
            continuation: false,
        }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: Style::plain(),
            continuation: false,
        }
    }
}

/// Horizontal text alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// A rectangular grid of styled cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
    background: Option<Color>,
    ascii: bool,
}

impl Canvas {
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); usize::from(width) * usize::from(height)],
            background: None,
            ascii: false,
        }
    }

    /// Restrict drawing to ASCII. UI glyphs with a sensible ASCII equivalent
    /// are translated; anything else becomes `?` so a non-UTF-8 TTY never sees
    /// broken byte sequences.
    pub fn set_ascii(&mut self, ascii: bool) {
        self.ascii = ascii;
    }

    /// Paint `Color::Default` backgrounds with the theme background. Text
    /// drawn later inherits it automatically.
    pub fn set_background(&mut self, background: Option<Color>) {
        self.background = background;
        if let Some(background) = background {
            for cell in &mut self.cells {
                if cell.style.bg == Color::Default {
                    cell.style.bg = background;
                }
            }
        }
    }

    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    #[must_use]
    pub const fn area(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    #[must_use]
    pub fn cell(&self, x: u16, y: u16) -> Cell {
        if x >= self.width || y >= self.height {
            return Cell::default();
        }
        self.cells[self.index(x, y)]
    }

    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    fn index(&self, x: u16, y: u16) -> usize {
        usize::from(y) * usize::from(self.width) + usize::from(x)
    }

    fn set_raw(&mut self, x: u16, y: u16, ch: char, style: Style, continuation: bool) {
        if x < self.width && y < self.height {
            let index = self.index(x, y);
            let mut style = style;
            if style.bg == Color::Default {
                let existing = self.cells[index].style.bg;
                if existing != Color::Default {
                    style.bg = existing;
                } else if let Some(background) = self.background {
                    style.bg = background;
                }
            }
            self.cells[index] = Cell {
                ch,
                style,
                continuation,
            };
        }
    }

    /// Clear one cell, repairing a neighbouring wide character.
    fn erase_at(&mut self, x: u16, y: u16) {
        if x >= self.width || y >= self.height {
            return;
        }
        let cell = self.cell(x, y);
        if cell.continuation && x > 0 {
            let left = self.cell(x - 1, y).style;
            self.set_raw(x - 1, y, ' ', left, false);
        } else if UnicodeWidthChar::width(cell.ch) == Some(2) && x + 1 < self.width {
            let right = self.cell(x + 1, y).style;
            self.set_raw(x + 1, y, ' ', right, false);
        }
        let style = cell.style;
        self.set_raw(x, y, ' ', style, false);
    }

    /// Write one character, clipping and repairing wide characters.
    pub fn put(&mut self, x: u16, y: u16, ch: char, style: Style) {
        if x >= self.width || y >= self.height {
            return;
        }
        let ch = if self.ascii { ascii_char(ch) } else { ch };
        let cell_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if cell_width == 0 {
            return;
        }
        self.erase_at(x, y);
        if cell_width == 2 {
            if x + 1 >= self.width {
                // A wide glyph cannot be split across the edge; leave a blank.
                return;
            }
            self.erase_at(x + 1, y);
            self.set_raw(x, y, ch, style, false);
            self.set_raw(x + 1, y, ' ', style, true);
        } else {
            self.set_raw(x, y, ch, style, false);
        }
    }

    /// Fill a rectangle with spaces in the given style.
    pub fn fill(&mut self, rect: Rect, style: Style) {
        let rect = rect.intersect(self.area());
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                self.erase_at(x, y);
                self.set_raw(x, y, ' ', style, false);
            }
        }
    }

    /// Fill a rectangle with a single-width character.
    pub fn fill_char(&mut self, rect: Rect, ch: char, style: Style) {
        let ch = if self.ascii { ascii_char(ch) } else { ch };
        if UnicodeWidthChar::width(ch) != Some(1) {
            self.fill(rect, style);
            return;
        }
        let rect = rect.intersect(self.area());
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                self.erase_at(x, y);
                self.set_raw(x, y, ch, style, false);
            }
        }
    }

    /// Draw text starting at `(x, y)`, clipped to `clip`.
    ///
    /// Returns the number of cells written. Zero-width and control characters
    /// are skipped; callers should sanitise external strings first.
    pub fn text(&mut self, clip: Rect, x: u16, y: u16, text: &str, style: Style) -> u16 {
        let clip = clip.intersect(self.area());
        if y < clip.y || y >= clip.bottom() || x < clip.x || x >= clip.right() {
            return 0;
        }
        let mut cx = x;
        let mut written = 0;
        for ch in text.chars() {
            let ch = if self.ascii { ascii_char(ch) } else { ch };
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if ch_width == 0 {
                continue;
            }
            if cx >= clip.right() || cx.saturating_add(ch_width as u16) > clip.right() {
                break;
            }
            self.put(cx, y, ch, style);
            cx = cx.saturating_add(ch_width as u16);
            written += ch_width as u16;
        }
        written
    }

    /// Draw text that begins at `x` but is limited to `max_width` cells.
    /// The text is truncated with an ellipsis when it does not fit.
    pub fn text_truncated(
        &mut self,
        clip: Rect,
        x: u16,
        y: u16,
        text: &str,
        style: Style,
        max_width: u16,
    ) -> u16 {
        let available = clip.right().saturating_sub(x).min(max_width);
        if available == 0 {
            return 0;
        }
        let clean = self.display_text(text);
        let truncated = format::truncate(&clean, usize::from(available));
        self.text(clip, x, y, &truncated, style)
    }

    /// Sanitise, and in ASCII mode translate every glyph, so truncation and
    /// alignment measure what will actually be drawn.
    fn display_text(&self, text: &str) -> String {
        let clean = format::sanitize(text);
        if self.ascii {
            clean.chars().map(ascii_char).collect()
        } else {
            clean
        }
    }

    /// Draw text aligned within `rect` on row `y`.
    pub fn text_aligned(
        &mut self,
        rect: Rect,
        y: u16,
        text: &str,
        style: Style,
        align: Align,
    ) -> u16 {
        if rect.is_empty() || y < rect.y || y >= rect.bottom() {
            return 0;
        }
        let clean = self.display_text(text);
        let width = usize::from(rect.w);
        let fitted = format::truncate(&clean, width);
        let text_width = format::width(&fitted).min(width);
        let x = match align {
            Align::Left => rect.x,
            Align::Center => rect.x + u16::try_from((width - text_width) / 2).unwrap_or(0),
            Align::Right => rect
                .right()
                .saturating_sub(u16::try_from(text_width).unwrap_or(0)),
        };
        self.text(rect, x, y, &fitted, style)
    }

    /// Draw a horizontal line of a single-width character.
    pub fn hline(&mut self, rect: Rect, x: u16, y: u16, length: u16, ch: char, style: Style) {
        let clip = rect.intersect(self.area());
        if y < clip.y || y >= clip.bottom() {
            return;
        }
        let end = x.saturating_add(length).min(clip.right());
        for cx in x.max(clip.x)..end {
            self.put(cx, y, ch, style);
        }
    }

    /// Draw a vertical line of a single-width character.
    pub fn vline(&mut self, rect: Rect, x: u16, y: u16, length: u16, ch: char, style: Style) {
        let clip = rect.intersect(self.area());
        if x < clip.x || x >= clip.right() {
            return;
        }
        let end = y.saturating_add(length).min(clip.bottom());
        for cy in y.max(clip.y)..end {
            self.put(x, cy, ch, style);
        }
    }

    /// Draw a box and return its interior. Titles are placed on the top edge.
    ///
    /// Boxes too small for a border are cleared and an empty rectangle is
    /// returned, which keeps every caller safe without special cases.
    pub fn draw_box(
        &mut self,
        rect: Rect,
        rounded: bool,
        border_style: Style,
        title: Option<(&str, Style)>,
    ) -> Rect {
        let rect = rect.intersect(self.area());
        if rect.w < 2 || rect.h < 2 {
            self.fill(rect, border_style);
            return Rect::new(rect.x, rect.y, 0, 0);
        }
        let (tl, tr, bl, br, hz, vt) = if rounded {
            ('╭', '╮', '╰', '╯', '─', '│')
        } else {
            ('+', '+', '+', '+', '-', '|')
        };
        self.put(rect.x, rect.y, tl, border_style);
        self.put(rect.right() - 1, rect.y, tr, border_style);
        self.put(rect.x, rect.bottom() - 1, bl, border_style);
        self.put(rect.right() - 1, rect.bottom() - 1, br, border_style);
        for x in rect.x + 1..rect.right() - 1 {
            self.put(x, rect.y, hz, border_style);
            self.put(x, rect.bottom() - 1, hz, border_style);
        }
        for y in rect.y + 1..rect.bottom() - 1 {
            self.put(rect.x, y, vt, border_style);
            self.put(rect.right() - 1, y, vt, border_style);
        }
        if let Some((title, title_style)) = title {
            if rect.w > 10 {
                let max_title = usize::from(rect.w.saturating_sub(7));
                let clean = format::sanitize(title);
                let truncated = format::truncate(&clean, max_title);
                let x = rect.x + 3;
                self.put(x.saturating_sub(1), rect.y, ' ', title_style);
                self.text(rect, x, rect.y, &truncated, title_style);
            }
        }
        Rect::new(rect.x + 1, rect.y + 1, rect.w - 2, rect.h - 2)
    }

    /// Convert the canvas to unstyled text, trimming trailing spaces.
    #[must_use]
    pub fn plain(&self) -> String {
        let mut out = String::with_capacity(
            usize::from(self.width) * usize::from(self.height) + self.height as usize,
        );
        for y in 0..self.height {
            let mut line = String::with_capacity(usize::from(self.width));
            for x in 0..self.width {
                let cell = self.cell(x, y);
                if cell.continuation {
                    continue;
                }
                line.push(cell.ch);
            }
            while line.ends_with(' ') {
                line.pop();
            }
            out.push_str(&line);
            if y + 1 < self.height {
                out.push('\n');
            }
        }
        out
    }
}

/// Translate a UI glyph to a stable ASCII equivalent.
#[must_use]
pub fn ascii_char(ch: char) -> char {
    match ch {
        '↑' => '^',
        '↓' | '▾' | '▼' => 'v',
        '←' => '<',
        '→' | '▸' | '▶' => '>',
        '·' | '│' | '┃' => '|',
        '…' => '.',
        '°' | '\u{00A0}' => ' ',
        '×' => 'x',
        '●' => '*',
        '○' | '◌' => 'o',
        '─' | '━' => '-',
        '├' | '└' | '┌' | '┐' | '┘' | '┤' | '┬' | '┴' | '┼' | '╭' | '╮' | '╰' | '╯' | '╔' | '╗'
        | '╚' | '╝' | '║' | '═' => '+',
        '█'
        | '▓'
        | '▒'
        | '░'
        | '▁'
        | '▂'
        | '▃'
        | '▄'
        | '▅'
        | '▆'
        | '▇'
        | '▏'
        | '▎'
        | '▍'
        | '▌'
        | '▋'
        | '▊'
        | '▉'
        | '\u{2800}'..='\u{28FF}' => '#',
        c if c.is_ascii() => c,
        _ => '?',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Rgb;

    fn style() -> Style {
        Style::fg(Rgb::new(255, 255, 255))
    }

    #[test]
    fn rect_splits_are_exact() {
        let rect = Rect::new(0, 0, 10, 10);
        let (top, bottom) = rect.split_top(4);
        assert_eq!(top, Rect::new(0, 0, 10, 4));
        assert_eq!(bottom, Rect::new(0, 4, 10, 6));
        let (left, right) = rect.split_left(3);
        assert_eq!(left.w, 3);
        assert_eq!(right.x, 3);
        assert_eq!(right.w, 7);
        let columns = rect.columns(3);
        assert_eq!(columns.iter().map(|r| r.w).sum::<u16>(), 10);
    }

    #[test]
    fn text_is_clipped_to_rectangle() {
        let mut canvas = Canvas::new(10, 3);
        canvas.text(Rect::new(2, 1, 5, 1), 2, 1, "hello world", style());
        assert_eq!(canvas.plain(), "\n  hello\n");
    }

    #[test]
    fn wide_characters_occupy_two_cells_and_repair() {
        let mut canvas = Canvas::new(5, 1);
        canvas.put(0, 0, '日', style());
        assert!(!canvas.cell(0, 0).continuation);
        assert!(canvas.cell(1, 0).continuation);
        canvas.put(0, 0, 'x', style());
        assert_eq!(canvas.cell(0, 0).ch, 'x');
        assert_eq!(canvas.cell(1, 0).ch, ' ');
        // A wide character at the right edge must not be split.
        canvas.put(4, 0, '日', style());
        assert_eq!(canvas.cell(4, 0).ch, ' ');
    }

    #[test]
    fn box_draws_border_and_title() {
        let mut canvas = Canvas::new(20, 5);
        let inner = canvas.draw_box(
            Rect::new(0, 0, 20, 5),
            true,
            style(),
            Some(("cpu", style())),
        );
        assert_eq!(inner, Rect::new(1, 1, 18, 3));
        let plain = canvas.plain();
        assert!(plain.contains("╭─ cpu"));
        assert!(plain.contains("╰"));
    }

    #[test]
    fn small_box_is_safe() {
        let mut canvas = Canvas::new(4, 4);
        let inner = canvas.draw_box(Rect::new(0, 0, 1, 1), true, style(), Some(("x", style())));
        assert!(inner.is_empty());
        assert_eq!(canvas.plain(), "\n\n\n");
    }

    #[test]
    fn ascii_mode_translates_ui_glyphs() {
        let mut canvas = Canvas::new(8, 1);
        canvas.set_ascii(true);
        canvas.text(Rect::new(0, 0, 8, 1), 0, 0, "↑ ↓ ● 日本", style());
        let plain = canvas.plain();
        assert!(plain.is_ascii(), "not ASCII: {plain:?}");
        assert!(plain.contains('^'));
        assert!(plain.contains('*'));
        assert!(plain.contains('?'));
    }

    #[test]
    fn text_preserves_an_explicit_background() {
        let mut canvas = Canvas::new(4, 1);
        let highlight = Style {
            fg: Color::Rgb(Rgb::new(255, 255, 255)),
            bg: Color::Rgb(Rgb::new(10, 20, 30)),
            ..Style::plain()
        };
        canvas.fill(Rect::new(0, 0, 4, 1), highlight);
        canvas.put(0, 0, 'x', style());
        assert_eq!(canvas.cell(0, 0).style.bg, highlight.bg);
        canvas.put(1, 0, '日', style());
        assert_eq!(canvas.cell(2, 0).style.bg, highlight.bg);
    }

    #[test]
    fn alignment_uses_display_width() {
        let mut canvas = Canvas::new(12, 1);
        canvas.text_aligned(Rect::new(0, 0, 12, 1), 0, "abc", style(), Align::Right);
        assert_eq!(canvas.plain(), "         abc");
        let mut canvas = Canvas::new(12, 1);
        canvas.text_aligned(Rect::new(0, 0, 12, 1), 0, "日本", style(), Align::Center);
        assert_eq!(canvas.plain(), "    日本");
    }
}
