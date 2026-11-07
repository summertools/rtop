//! Reusable widgets: bars, meter rows, scrollbars, chips, and key hints.

use unicode_width::UnicodeWidthChar;

use super::canvas::{Align, Canvas, Rect};
use crate::format;
use crate::theme::{Gradient, Rgb, Style, Theme};

const EIGHTHS: [char; 7] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// Draw a one-line horizontal bar. Filled cells use the gradient; the track
/// is deliberately dim so bars read at a glance without shouting.
pub fn bar(
    canvas: &mut Canvas,
    rect: Rect,
    ratio: f32,
    gradient: Gradient,
    theme: &Theme,
    tty: bool,
) {
    if rect.is_empty() || rect.h == 0 {
        return;
    }
    let ratio = if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let width = f32::from(rect.w);
    let filled = ratio * width;
    let full = filled.floor() as u16;
    let fraction = filled - f32::from(full);
    let track = Style::fg(theme.dim).dim();
    for index in 0..rect.w {
        let position = (f32::from(index) + 1.0) / width;
        let style = Style::fg(gradient.at(position));
        if index < full.min(rect.w) {
            canvas.put(rect.x + index, rect.y, if tty { '#' } else { '█' }, style);
        } else if index == full && fraction > 0.02 && !tty {
            let eighth = ((fraction * 8.0).round() as usize).clamp(1, 7);
            canvas.put(rect.x + index, rect.y, EIGHTHS[eighth - 1], style);
        } else {
            canvas.put(rect.x + index, rect.y, if tty { '-' } else { '░' }, track);
        }
    }
}

/// A compact metric row: label, bar, value, percentage.
pub fn meter_row(
    canvas: &mut Canvas,
    rect: Rect,
    label: &str,
    value: &str,
    ratio: f32,
    gradient: Gradient,
    theme: &Theme,
    tty: bool,
) {
    if rect.is_empty() || rect.h == 0 || rect.w < 12 {
        return;
    }
    let clean_label = format::sanitize(label);
    let clean_value = format::sanitize(value);
    let percent = format::percent(ratio * 100.0);
    let label_w = 9u16.min(rect.w);
    let percent_w = 5u16;
    let value_w = 10u16;
    let mut x = rect.x;
    let label_w = label_w.min(rect.w.saturating_sub(percent_w + value_w + 4));
    let used = canvas.text(
        rect,
        x,
        rect.y,
        &format::truncate(&clean_label, usize::from(label_w)),
        Style::fg(theme.dim),
    );
    x = x.saturating_add(used.max(label_w));
    let remaining = rect.right().saturating_sub(x);
    let bar_w = remaining.saturating_sub(percent_w + value_w + 3).min(80);
    if bar_w >= 4 {
        canvas.text(rect, x, rect.y, " ", Style::plain());
        bar(
            canvas,
            Rect::new(x + 1, rect.y, bar_w, 1),
            ratio,
            gradient,
            theme,
            tty,
        );
        x = x.saturating_add(bar_w + 1);
    }
    let value_x = rect.right().saturating_sub(percent_w + value_w + 1);
    if value_x > x {
        canvas.text_aligned(
            Rect::new(value_x, rect.y, value_w, 1),
            rect.y,
            &clean_value,
            Style::fg(theme.foreground),
            Align::Right,
        );
    }
    canvas.text_aligned(
        Rect::new(rect.right().saturating_sub(percent_w), rect.y, percent_w, 1),
        rect.y,
        &percent,
        Style::fg(gradient.at(ratio.clamp(0.0, 1.0))).bold(),
        Align::Right,
    );
}

/// Vertical scrollbar in a one-column rectangle.
pub fn scrollbar(
    canvas: &mut Canvas,
    rect: Rect,
    offset: usize,
    visible: usize,
    total: usize,
    theme: &Theme,
    tty: bool,
) {
    if rect.is_empty() || total <= visible || visible == 0 {
        return;
    }
    let height = usize::from(rect.h);
    let thumb = ((visible * height) / total).max(1).min(height);
    let max_offset = total.saturating_sub(visible);
    let travel = height.saturating_sub(thumb);
    let thumb_top = (offset.min(max_offset) * travel)
        .checked_div(max_offset)
        .unwrap_or(0);
    for row in 0..height {
        let ch = if row >= thumb_top && row < thumb_top + thumb {
            if tty { '#' } else { '┃' }
        } else if tty {
            '|'
        } else {
            ' '
        };
        let style = if row >= thumb_top && row < thumb_top + thumb {
            Style::fg(theme.accent)
        } else {
            Style::fg(theme.dim).dim()
        };
        canvas.put(rect.x, rect.y + row as u16, ch, style);
    }
}

/// Draw a filled chip and return its width.
pub fn chip(
    canvas: &mut Canvas,
    rect: Rect,
    x: u16,
    y: u16,
    text: &str,
    fg: Rgb,
    bg: Rgb,
    bold: bool,
) -> u16 {
    let clean = format::sanitize(text);
    let content_width = format::width(&clean).min(usize::from(rect.w));
    let width = u16::try_from(content_width).unwrap_or(0).saturating_add(2);
    if width == 0 || y < rect.y || y >= rect.bottom() || x >= rect.right() {
        return 0;
    }
    let width = width.min(rect.right().saturating_sub(x));
    let style = Style {
        fg: fg.into(),
        bg: bg.into(),
        bold,
        ..Style::plain()
    };
    for index in 0..width {
        canvas.put(x + index, y, ' ', style);
    }
    let text_rect = Rect::new(x, y, width, 1);
    canvas.text_aligned(text_rect, y, &clean, style, Align::Center);
    width
}

/// Render `key label` pairs. Keys are accented; labels are dim.
pub fn key_hints(
    canvas: &mut Canvas,
    rect: Rect,
    items: &[(&str, &str)],
    theme: &Theme,
    separator: &str,
) -> u16 {
    let max = rect.w;
    let mut x = rect.x;
    let sep_style = Style::fg(theme.dim).dim();
    for (index, (key, label)) in items.iter().enumerate() {
        let mut needed = u16::try_from(format::width(key) + 1 + format::width(label)).unwrap_or(0);
        if index > 0 {
            needed = needed.saturating_add(u16::try_from(format::width(separator)).unwrap_or(0));
        }
        if x.saturating_add(needed) > rect.right() {
            break;
        }
        if index > 0 {
            x = x.saturating_add(canvas.text(rect, x, rect.y, separator, sep_style));
        }
        x = x.saturating_add(canvas.text(rect, x, rect.y, key, Style::fg(theme.accent).bold()));
        x = x.saturating_add(canvas.text(rect, x, rect.y, " ", Style::plain()));
        x = x.saturating_add(canvas.text(rect, x, rect.y, label, Style::fg(theme.dim)));
    }
    x.saturating_sub(rect.x).min(max)
}

/// Word-wrap text to `width` display cells. Long words are split rather than
/// being allowed to overflow.
#[must_use]
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let paragraph = format::sanitize(paragraph);
        let mut line = String::new();
        let mut line_width = 0;
        for word in paragraph.split_whitespace() {
            let word_width = format::width(word);
            if word_width <= width && line_width > 0 && line_width + 1 + word_width > width {
                lines.push(std::mem::take(&mut line));
                line_width = 0;
            }
            if word_width > width {
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                let mut chunk = String::new();
                let mut chunk_width = 0;
                for ch in word.chars() {
                    let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
                    if chunk_width + ch_width > width && !chunk.is_empty() {
                        lines.push(std::mem::take(&mut chunk));
                        chunk_width = 0;
                    }
                    chunk.push(ch);
                    chunk_width += ch_width;
                }
                line = chunk;
                line_width = chunk_width;
                continue;
            }
            if line.is_empty() {
                line.push_str(word);
                line_width = word_width;
            } else {
                line.push(' ');
                line.push_str(word);
                line_width += 1 + word_width;
            }
        }
        lines.push(line);
    }
    lines
}

/// Faint separator used between header fields.
pub fn separator(canvas: &mut Canvas, rect: Rect, x: u16, y: u16, theme: &Theme) -> u16 {
    canvas.text(rect, x, y, " · ", Style::fg(theme.dim).dim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GraphSymbol;

    fn theme() -> Theme {
        Theme::builtin("default").unwrap()
    }

    #[test]
    fn bar_is_clipped_and_proportional() {
        let mut canvas = Canvas::new(10, 1);
        let theme = theme();
        bar(
            &mut canvas,
            Rect::new(0, 0, 10, 1),
            0.5,
            theme.cpu,
            &theme,
            false,
        );
        let plain = canvas.plain();
        assert_eq!(plain.chars().filter(|c| *c == '█').count(), 5);
    }

    #[test]
    fn bar_never_draws_outside() {
        let mut canvas = Canvas::new(10, 2);
        let theme = theme();
        bar(
            &mut canvas,
            Rect::new(2, 1, 5, 1),
            2.0,
            theme.cpu,
            &theme,
            false,
        );
        assert_eq!(canvas.cell(0, 0).ch, ' ');
        assert_eq!(canvas.cell(1, 1).ch, ' ');
        assert_eq!(canvas.cell(7, 1).ch, ' ');
    }

    #[test]
    fn wrap_text_splits_long_words_and_paragraphs() {
        assert_eq!(wrap_text("hello world", 5), vec!["hello", "world"]);
        assert_eq!(wrap_text("abcdef", 3), vec!["abc", "def"]);
        assert_eq!(wrap_text("a\nb", 5), vec!["a", "b"]);
        assert!(wrap_text("", 5).is_empty() || wrap_text("", 5) == vec![String::new()]);
    }

    #[test]
    fn key_hints_stop_at_the_edge() {
        let mut canvas = Canvas::new(20, 1);
        let theme = theme();
        let used = key_hints(
            &mut canvas,
            Rect::new(0, 0, 20, 1),
            &[("↑↓", "move"), ("q", "quit"), ("?", "help")],
            &theme,
            "  ",
        );
        assert!(used <= 20);
    }

    #[test]
    fn scrollbar_has_a_visible_thumb() {
        let mut canvas = Canvas::new(3, 5);
        let theme = theme();
        scrollbar(&mut canvas, Rect::new(2, 0, 1, 5), 0, 2, 10, &theme, false);
        assert!(canvas.plain().contains('┃'));
    }

    #[test]
    fn bar_chars_follow_graph_mode() {
        let mut canvas = Canvas::new(4, 1);
        let theme = theme();
        bar(
            &mut canvas,
            Rect::new(0, 0, 4, 1),
            1.0,
            theme.cpu,
            &theme,
            true,
        );
        assert!(canvas.plain().chars().all(|c| c == '#'));
        let _ = GraphSymbol::Braille;
    }
}
