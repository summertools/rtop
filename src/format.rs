//! Formatting helpers shared by the UI and the tests.
//!
//! Everything that crosses the boundary from the operating system into the
//! terminal goes through [`sanitize`] first. That is deliberate: process
//! names, command lines, mount points, and interface names are attacker
//! controlled on a busy or hostile machine, and a raw ESC byte must never
//! reach the terminal.

use std::borrow::Cow;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
const SHORT_UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];

/// Format a byte count with binary units and a width-friendly precision.
///
/// `1024` becomes `1.00 KiB`, `10 * 1024` becomes `10.0 KiB`, and
/// `999 * 1024` becomes `999 KiB`.
#[must_use]
pub fn bytes(value: u64) -> String {
    if value < 1024 {
        return format!("{value} B");
    }
    let (scaled, unit) = scale(value as f64, 1024.0, &UNITS);
    if scaled >= 100.0 {
        format!("{scaled:.0} {unit}")
    } else if scaled >= 10.0 {
        format!("{scaled:.1} {unit}")
    } else {
        format!("{scaled:.2} {unit}")
    }
}

/// Compact byte count for dense tables, for example `9.4M`.
#[must_use]
pub fn bytes_short(value: u64) -> String {
    if value < 1024 {
        return format!("{value}B");
    }
    let (scaled, unit) = scale(value as f64, 1024.0, &SHORT_UNITS);
    if scaled >= 10.0 {
        format!("{scaled:.0}{unit}")
    } else {
        format!("{scaled:.1}{unit}")
    }
}

/// Format a floating-point byte rate such as `1.24 MiB/s`.
#[must_use]
pub fn rate(bytes_per_second: f64) -> String {
    if !bytes_per_second.is_finite() || bytes_per_second <= 0.0 {
        return "0 B/s".to_owned();
    }
    if bytes_per_second < 1024.0 {
        return format!("{bytes_per_second:.0} B/s");
    }
    let (scaled, unit) = scale(bytes_per_second, 1024.0, &UNITS);
    if scaled >= 100.0 {
        format!("{scaled:.0} {unit}/s")
    } else if scaled >= 10.0 {
        format!("{scaled:.1} {unit}/s")
    } else {
        format!("{scaled:.2} {unit}/s")
    }
}

/// Compact byte rate for dense disk and network rows, for example `1.2M/s`.
#[must_use]
pub fn rate_short(bytes_per_second: f64) -> String {
    if !bytes_per_second.is_finite() || bytes_per_second <= 0.0 {
        return "0/s".to_owned();
    }
    if bytes_per_second < 1024.0 {
        return format!("{bytes_per_second:.0}B/s");
    }
    let (scaled, unit) = scale(bytes_per_second, 1024.0, &SHORT_UNITS);
    if scaled >= 10.0 {
        format!("{scaled:.0}{unit}/s")
    } else {
        format!("{scaled:.1}{unit}/s")
    }
}

/// Format the same rate in bits per second.
#[must_use]
pub fn bit_rate(bytes_per_second: f64) -> String {
    let bits = bytes_per_second * 8.0;
    if !bits.is_finite() || bits <= 0.0 {
        return "0 bit/s".to_owned();
    }
    let (scaled, unit) = scale(
        bits,
        1000.0,
        &["bit/s", "kbit/s", "Mbit/s", "Gbit/s", "Tbit/s", "Pbit/s"],
    );
    if scaled >= 100.0 {
        format!("{scaled:.0} {unit}")
    } else if scaled >= 10.0 {
        format!("{scaled:.1} {unit}")
    } else {
        format!("{scaled:.2} {unit}")
    }
}

fn scale(mut value: f64, base: f64, units: &[&'static str; 6]) -> (f64, &'static str) {
    let mut index = 0;
    while value >= base && index + 1 < units.len() {
        value /= base;
        index += 1;
    }
    (value, units[index])
}

/// Render a percentage without a trailing `.0`.
#[must_use]
pub fn percent(value: f32) -> String {
    let value = if value.is_finite() {
        value.clamp(0.0, 100.0)
    } else {
        0.0
    };
    if value > 0.0 && value < 1.0 {
        return "<1%".to_owned();
    }
    format!("{value:.0}%")
}

/// Render a percentage with explicit precision.
#[must_use]
pub fn percent_precise(value: f32, precision: usize) -> String {
    let value = if value.is_finite() {
        value.clamp(0.0, 100.0)
    } else {
        0.0
    };
    format!("{value:.precision$}%")
}

/// Human-readable uptime, for example `13:50:04` or `3d 04:12:33`.
#[must_use]
pub fn uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let secs = seconds % 60;
    if days > 0 {
        format!("{days}d {hours:02}:{minutes:02}:{secs:02}")
    } else {
        format!("{hours:02}:{minutes:02}:{secs:02}")
    }
}

/// Short duration used in the process table.
#[must_use]
pub fn duration_short(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

/// Make an operating-system string safe to place in a terminal.
///
/// Control characters are replaced by spaces (newlines) or `?`, Unicode
/// bidirectional overrides are neutralised, and zero-width characters are
/// dropped. This prevents escape injection and visual spoofing.
#[must_use]
pub fn sanitize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\t' | '\n' | '\r' | '\u{2028}' | '\u{2029}' => out.push(' '),
            '\u{200B}'..='\u{200D}' | '\u{FEFF}' | '\u{00AD}' => {}
            '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{061C}' => out.push('?'),
            c if c.is_control() => out.push('?'),
            c => out.push(c),
        }
    }
    out
}

/// Sanitise and cap the number of characters in one pass.
#[must_use]
pub fn sanitize_limited(input: &str, max_chars: usize) -> String {
    let clean = sanitize(input);
    if clean.chars().count() <= max_chars {
        clean
    } else {
        clean.chars().take(max_chars).collect()
    }
}

/// Display width of a string, ignoring control characters.
#[must_use]
pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Truncate to `max_width` terminal cells, appending an ellipsis.
#[must_use]
pub fn truncate(text: &str, max_width: usize) -> Cow<'_, str> {
    truncate_with(text, max_width, '…')
}

/// Truncate to `max_width` cells using an ASCII fallback.
#[must_use]
pub fn truncate_ascii(text: &str, max_width: usize) -> Cow<'_, str> {
    truncate_with(text, max_width, '>')
}

fn truncate_with(text: &str, max_width: usize, ellipsis: char) -> Cow<'_, str> {
    if width(text) <= max_width {
        return Cow::Borrowed(text);
    }
    if max_width == 0 {
        return Cow::Borrowed("");
    }
    let budget = max_width.saturating_sub(1);
    let mut out = String::with_capacity(text.len().min(max_width * 4));
    let mut used = 0;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > budget {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out.push(ellipsis);
    Cow::Owned(out)
}

/// Pad on the right to a display width. Strings wider than `width` are not
/// truncated; callers usually truncate first.
#[must_use]
pub fn pad_right(text: &str, width: usize) -> String {
    let used = UnicodeWidthStr::width(text);
    if used >= width {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + width - used);
    out.push_str(text);
    out.extend(std::iter::repeat_n(' ', width - used));
    out
}

/// Pad on the left to a display width.
#[must_use]
pub fn pad_left(text: &str, width: usize) -> String {
    let used = UnicodeWidthStr::width(text);
    if used >= width {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + width - used);
    out.extend(std::iter::repeat_n(' ', width - used));
    out.push_str(text);
    out
}

/// Replace path separators so a command stays on one line.
#[must_use]
pub fn oneline(input: &str) -> String {
    sanitize(input)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_formatting_is_stable() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1023), "1023 B");
        assert_eq!(bytes(1024), "1.00 KiB");
        assert_eq!(bytes(10 * 1024), "10.0 KiB");
        assert_eq!(bytes(100 * 1024), "100 KiB");
        assert_eq!(bytes(8 * 1024 * 1024 * 1024), "8.00 GiB");
        assert_eq!(bytes_short(9 * 1024 * 1024), "9.0M");
        assert_eq!(bytes_short(64 * 1024 * 1024), "64M");
    }

    #[test]
    fn rates_handle_zero_and_infinity() {
        assert_eq!(rate(0.0), "0 B/s");
        assert_eq!(rate(f64::NAN), "0 B/s");
        assert_eq!(rate(1536.0), "1.50 KiB/s");
        assert_eq!(rate_short(1536.0), "1.5K/s");
        assert_eq!(bit_rate(1536.0), "12.3 kbit/s");
    }

    #[test]
    fn percentages_are_clamped() {
        assert_eq!(percent(-1.0), "0%");
        assert_eq!(percent(0.4), "<1%");
        assert_eq!(percent(42.4), "42%");
        assert_eq!(percent(f32::NAN), "0%");
    }

    #[test]
    fn uptime_formats_days() {
        assert_eq!(uptime(50_000), "13:53:20");
        assert_eq!(uptime(90_061), "1d 01:01:01");
        assert_eq!(duration_short(3_661), "1h 01m");
    }

    #[test]
    fn sanitize_strips_escape_sequences_and_bidi() {
        let dirty = "evil\u{1b}[2Jname\u{202e}txt\nnext";
        let clean = sanitize(dirty);
        assert!(!clean.contains('\u{1b}'));
        assert!(!clean.contains('\u{202e}'));
        assert!(!clean.contains('\n'));
        assert_eq!(clean, "evil?[2Jname?txt next");
    }

    #[test]
    fn truncate_respects_wide_characters() {
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("日本語", 4), "日…");
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn padding_uses_display_width() {
        assert_eq!(pad_right("abc", 5), "abc  ");
        assert_eq!(pad_left("abc", 5), "  abc");
        assert_eq!(pad_right("日本", 2), "日本");
    }

    #[test]
    fn sanitize_limited_caps_after_cleaning() {
        let value = "a\nbcdef";
        assert_eq!(sanitize_limited(value, 4), "a bc");
    }
}
