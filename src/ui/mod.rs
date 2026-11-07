//! Rendering: layout, header, footer, and view dispatch.

pub mod canvas;
pub mod graph;
pub mod widgets;

mod cpu;
mod mem;
mod net;
mod overlays;
mod proc;

use crate::MIN_HEIGHT;
use crate::MIN_WIDTH;
use crate::app::{App, Layout, Overlay, Preset, ToastLevel};
use crate::format;
use crate::theme::{Rgb, Style, Theme};
use canvas::{Align, Canvas, Rect};

/// Largest canvas we will allocate. Real terminals never approach this, but a
/// malformed PTY window size should not be able to exhaust memory.
pub const MAX_CANVAS_WIDTH: u16 = 1024;
/// See [`MAX_CANVAS_WIDTH`].
pub const MAX_CANVAS_HEIGHT: u16 = 512;

/// Render one complete frame. Layout rectangles are returned separately so
/// mouse handling never needs to guess where a widget ended up.
#[must_use]
pub fn render(app: &App, width: u16, height: u16) -> (Canvas, Layout) {
    let width = width.min(MAX_CANVAS_WIDTH);
    let height = height.min(MAX_CANVAS_HEIGHT);
    let mut canvas = Canvas::new(width, height);
    canvas.set_ascii(app.ascii());
    let mut layout = Layout {
        screen: Rect::new(0, 0, width, height),
        ..Layout::default()
    };
    if app.config.theme_background {
        canvas.set_background(Some(app.theme.background.into()));
    }

    if width < MIN_WIDTH || height < MIN_HEIGHT {
        draw_too_small(&mut canvas, app, width, height);
        return (canvas, layout);
    }

    let header_h = if height >= 18 { 2 } else { 1 };
    layout.tabs = draw_header(&mut canvas, app, Rect::new(0, 0, width, header_h));
    let footer_h = 1;
    let content = Rect::new(
        0,
        header_h,
        width,
        height.saturating_sub(header_h).saturating_sub(footer_h),
    );

    if app.preset == Preset::Overview && width >= 84 && content.h >= 20 {
        draw_overview(&mut canvas, app, content, &mut layout);
    } else if app.preset == Preset::Overview {
        if width >= 72 && content.h >= 17 {
            let top_h = (content.h * 45 / 100).clamp(8, 11);
            let (top, bottom) = content.split_top(top_h);
            let halves = top.columns(2);
            if let Some(left) = halves.first() {
                cpu::draw(&mut canvas, app, *left, true);
            }
            if let Some(right) = halves.get(1) {
                mem::draw(&mut canvas, app, *right, true);
            }
            proc::draw(&mut canvas, app, bottom, &mut layout);
        } else if content.h >= 15 {
            let top_h = (content.h * 42 / 100).max(7);
            let (top, bottom) = content.split_top(top_h);
            cpu::draw(&mut canvas, app, top, true);
            proc::draw(&mut canvas, app, bottom, &mut layout);
        } else {
            proc::draw(&mut canvas, app, content, &mut layout);
        }
    } else {
        match app.preset {
            Preset::Cpu => cpu::draw(&mut canvas, app, content, false),
            Preset::Memory => mem::draw(&mut canvas, app, content, false),
            Preset::Network => net::draw(&mut canvas, app, content, false, &mut layout),
            Preset::Processes => proc::draw(&mut canvas, app, content, &mut layout),
            Preset::Overview => {}
        }
    }

    draw_footer(&mut canvas, app, Rect::new(0, height - 1, width, 1));
    overlays::draw(&mut canvas, app, layout.screen);
    (canvas, layout)
}

fn draw_overview(canvas: &mut Canvas, app: &App, content: Rect, layout: &mut Layout) {
    let mut top_h = (content.h * 42 / 100).clamp(8, 14);
    let mut net_h = if content.h >= 27 {
        (content.h * 24 / 100).clamp(6, 9)
    } else {
        0
    };
    if content.h.saturating_sub(top_h + net_h) < 7 {
        net_h = 0;
        top_h = content.h.saturating_sub(7).max(6);
    }
    let (top, rest) = content.split_top(top_h);
    let (net_area, proc_area) = if net_h > 0 {
        rest.split_top(net_h)
    } else {
        (Rect::new(0, 0, 0, 0), rest)
    };
    let halves = top.columns(2);
    if let Some(left) = halves.first() {
        cpu::draw(canvas, app, *left, true);
    }
    if let Some(right) = halves.get(1) {
        mem::draw(canvas, app, *right, true);
    }
    if !net_area.is_empty() {
        net::draw(canvas, app, net_area, true, layout);
    }
    proc::draw(canvas, app, proc_area, layout);
}

fn draw_header(canvas: &mut Canvas, app: &App, rect: Rect) -> Vec<(Preset, Rect)> {
    let theme = &app.theme;
    let mut tabs = Vec::new();
    let mut x = rect.x.saturating_add(1);
    let badge = widgets::chip(
        canvas,
        rect,
        x,
        rect.y,
        "rtop",
        theme.background,
        theme.title,
        true,
    );
    x = x.saturating_add(badge).saturating_add(1);

    let compact = rect.w < 92;
    let battery_reserve = if app.config.show_battery
        && app
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.battery.as_ref())
            .is_some()
    {
        10
    } else {
        0
    };
    let pause_reserve = if app.paused { 9 } else { 0 };
    let reserve_right = (11 + battery_reserve + pause_reserve).min(rect.w / 2);
    let limit = rect.right().saturating_sub(reserve_right);
    let mut items: Vec<(Preset, String, u16)> = Vec::with_capacity(5);
    for preset in Preset::all() {
        let text = if compact {
            preset.label().to_owned()
        } else {
            format!("{} {}", preset.index(), preset.label())
        };
        let tab_width = u16::try_from(format::width(&text))
            .unwrap_or(0)
            .saturating_add(2);
        items.push((preset, text, tab_width));
    }
    let mut fits = 0usize;
    let mut cursor = x;
    for (_, _, width) in &items {
        if cursor.saturating_add(*width) > limit {
            break;
        }
        cursor = cursor.saturating_add(*width).saturating_add(1);
        fits += 1;
    }
    let active_pos = items
        .iter()
        .position(|(preset, _, _)| *preset == app.preset)
        .unwrap_or(0);
    let start = if fits == 0 {
        active_pos
    } else {
        (active_pos + 1).saturating_sub(fits)
    };
    let visible = if fits == 0 { 1 } else { fits };
    for (preset, text, tab_width) in items.iter().skip(start).take(visible) {
        if x.saturating_add(*tab_width) > rect.right().saturating_sub(2) {
            break;
        }
        let tab_rect = Rect::new(x, rect.y, *tab_width, 1);
        if *preset == app.preset {
            let style = Style {
                fg: theme.highlight_fg.into(),
                bg: theme.highlight_bg.into(),
                bold: true,
                ..Style::plain()
            };
            canvas.fill(tab_rect, style);
            canvas.text_aligned(tab_rect, rect.y, text, style, Align::Center);
        } else {
            canvas.text_aligned(tab_rect, rect.y, text, Style::fg(theme.dim), Align::Center);
        }
        tabs.push((*preset, tab_rect));
        x = x.saturating_add(*tab_width).saturating_add(1);
    }

    // Right side of the first row: help marker, battery, and clock.
    let mut right = rect.right().saturating_sub(1);
    let clock = app.clock();
    let clock_width = u16::try_from(format::width(clock)).unwrap_or(8);
    if rect.w > clock_width + 8 {
        let clock_rect = Rect::new(right.saturating_sub(clock_width), rect.y, clock_width, 1);
        canvas.text_aligned(
            clock_rect,
            rect.y,
            clock,
            Style::fg(theme.foreground).bold(),
            Align::Right,
        );
        right = right.saturating_sub(clock_width + 1);
    }
    if app.config.show_battery {
        if let Some(battery) = app
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.battery.as_ref())
        {
            let text = if battery.state == crate::platform::BatteryState::Charging {
                format!("BAT {}%+", battery.percent)
            } else {
                format!("BAT {}%", battery.percent)
            };
            let width = u16::try_from(format::width(&text)).unwrap_or(0);
            if right.saturating_sub(width) > x {
                let color = if battery.percent <= 15 {
                    theme.critical
                } else if battery.percent <= 35 {
                    theme.warning
                } else {
                    theme.good
                };
                let battery_rect = Rect::new(right.saturating_sub(width), rect.y, width, 1);
                canvas.text_aligned(battery_rect, rect.y, &text, Style::fg(color), Align::Right);
                right = right.saturating_sub(width + 1);
            }
        }
    }
    if app.paused && right.saturating_sub(9) > x {
        let width = widgets::chip(
            canvas,
            Rect::new(right.saturating_sub(8), rect.y, 8, 1),
            right.saturating_sub(8),
            rect.y,
            "PAUSED",
            theme.background,
            theme.warning,
            true,
        );
        right = right.saturating_sub(width + 1);
    }
    if right > x + 2 {
        canvas.text(
            rect,
            right.saturating_sub(1),
            rect.y,
            "?",
            Style::fg(theme.dim),
        );
    }

    if rect.h > 1 {
        draw_header_info(canvas, app, Rect::new(rect.x, rect.y + 1, rect.w, 1));
    }
    tabs
}

fn draw_header_info(canvas: &mut Canvas, app: &App, rect: Rect) {
    let theme = &app.theme;
    let Some(snapshot) = app.snapshot.as_ref() else {
        canvas.text(
            rect,
            rect.x + 1,
            rect.y,
            "collecting system information…",
            Style::fg(theme.dim),
        );
        return;
    };
    let host = &snapshot.host;
    let mut x = rect.x + 1;
    let host_style = Style::fg(theme.title).bold();
    let host_width = u16::try_from(format::width(&host.hostname))
        .unwrap_or(0)
        .min(rect.w / 4);
    x = x.saturating_add(canvas.text_truncated(
        rect,
        x,
        rect.y,
        &host.hostname,
        host_style,
        host_width,
    ));
    let user = host.user.clone();
    let left_limit = rect.w / 2;
    if rect.w > 70 && x.saturating_add(19) <= left_limit {
        x = x.saturating_add(widgets::separator(canvas, rect, x, rect.y, theme));
        x = x.saturating_add(canvas.text_truncated(
            rect,
            x,
            rect.y,
            &user,
            Style::fg(theme.accent),
            16,
        ));
    }
    if rect.w > 104 && x.saturating_add(25) <= left_limit {
        x = x.saturating_add(widgets::separator(canvas, rect, x, rect.y, theme));
        x = x.saturating_add(canvas.text_truncated(
            rect,
            x,
            rect.y,
            &host.os,
            Style::fg(theme.dim),
            22,
        ));
    }

    // Right-aligned operational facts.
    let mut facts: Vec<(String, Style)> = Vec::new();
    facts.push((
        format!("up {}", format::uptime(snapshot.cpu.uptime_secs)),
        Style::fg(theme.foreground),
    ));
    facts.push((
        format!(
            "load {:.2} {:.2} {:.2}",
            snapshot.cpu.load[0], snapshot.cpu.load[1], snapshot.cpu.load[2]
        ),
        Style::fg(theme.dim),
    ));
    if let Some(temp) = snapshot.cpu.temperature {
        facts.push((
            format!("{temp:.0}°C"),
            Style::fg(crate::ui::graph::temperature_color(temp, theme.temp)),
        ));
    }
    if snapshot.cpu.average_frequency_mhz > 0 {
        facts.push((
            format!(
                "{:.2} GHz",
                snapshot.cpu.average_frequency_mhz as f64 / 1000.0
            ),
            Style::fg(theme.dim),
        ));
    }
    let mut fact_width: u16 = facts
        .iter()
        .map(|(text, _)| u16::try_from(format::width(text)).unwrap_or(0))
        .sum::<u16>()
        .saturating_add(3 * (facts.len().saturating_sub(1)) as u16);
    while fact_width + 12 > rect.w / 2 && facts.len() > 1 {
        fact_width = fact_width.saturating_sub(
            u16::try_from(format::width(&facts.pop().map(|f| f.0).unwrap_or_default()))
                .unwrap_or(0)
                .saturating_add(3),
        );
    }
    let mut fact_x = rect.right().saturating_sub(fact_width + 1);
    for (index, (text, style)) in facts.iter().enumerate() {
        if index > 0 {
            fact_x = fact_x.saturating_add(widgets::separator(canvas, rect, fact_x, rect.y, theme));
        }
        let width = u16::try_from(format::width(text)).unwrap_or(0);
        canvas.text(rect, fact_x, rect.y, text, *style);
        fact_x = fact_x.saturating_add(width);
    }

    // Use the space between the identity and the facts for the CPU model.
    let model_x = x.saturating_add(2);
    if model_x < rect.right().saturating_sub(fact_width + 2) {
        let available = rect
            .right()
            .saturating_sub(fact_width + 2)
            .saturating_sub(model_x);
        canvas.text_truncated(
            rect,
            model_x,
            rect.y,
            &host.cpu_model,
            Style::fg(theme.dim).dim(),
            available,
        );
    }
}

fn draw_footer(canvas: &mut Canvas, app: &App, rect: Rect) {
    let theme = &app.theme;
    let hints: Vec<(&str, &str)> = if app.filter_edit.is_some() {
        vec![
            ("type", "filter"),
            ("Enter", "apply"),
            ("Esc", "cancel"),
            ("Ctrl+U", "clear"),
        ]
    } else {
        match &app.overlay {
            Overlay::Help { .. } | Overlay::ProcessInfo { .. } => {
                vec![("↑↓", "scroll"), ("Esc", "close")]
            }
            Overlay::ConfirmKill { .. } => {
                vec![("←→", "signal"), ("Enter", "confirm"), ("Esc", "cancel")]
            }
            Overlay::None => match app.preset {
                Preset::Overview => vec![
                    ("↑↓", "move"),
                    ("Enter", "info"),
                    ("f", "filter"),
                    ("k", "kill"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
                Preset::Cpu | Preset::Memory => vec![
                    ("Tab", "next"),
                    ("+/-", "speed"),
                    ("s", "pause"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
                Preset::Network => vec![
                    ("[ ]", "iface"),
                    ("a", "auto"),
                    ("+/-", "speed"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
                Preset::Processes => vec![
                    ("↑↓", "move"),
                    ("Enter", "info"),
                    ("f", "filter"),
                    ("c/m/p/n", "sort"),
                    ("t", "tree"),
                    ("k", "kill"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
            },
        }
    };

    // Narrow terminals get the essentials instead of a truncated row that
    // loses the quit key.
    let hints = if rect.w < 72 && !app.overlay.is_open() && app.filter_edit.is_none() {
        match app.preset {
            Preset::Overview | Preset::Processes => {
                vec![("↑↓", "move"), ("?", "help"), ("q", "quit")]
            }
            _ => vec![("Tab", "next"), ("?", "help"), ("q", "quit")],
        }
    } else {
        hints
    };

    let toast_text = app.toast.as_ref().map(|toast| {
        let prefix = match toast.level {
            ToastLevel::Info => "",
            ToastLevel::Warning => "! ",
            ToastLevel::Error => "× ",
        };
        format!("{prefix}{}", toast.text)
    });
    let right = if let Some(buffer) = &app.filter_edit {
        Some((format!("filter: {buffer}_"), Style::fg(theme.accent).bold()))
    } else if let Some(text) = toast_text {
        let style = match app.toast.as_ref().map(|toast| toast.level) {
            Some(ToastLevel::Warning) => Style::fg(theme.warning).bold(),
            Some(ToastLevel::Error) => Style::fg(theme.critical).bold(),
            _ => Style::fg(theme.accent),
        };
        Some((text, style))
    } else if !app.filter.is_empty() {
        Some((format!("filter: {}", app.filter), Style::fg(theme.accent)))
    } else {
        None
    };
    let right_width = right.as_ref().map_or(0, |(text, _)| {
        u16::try_from(format::width(text))
            .unwrap_or(0)
            .min(rect.w / 2)
    });
    let hint_width = rect.w.saturating_sub(right_width + 2);
    if app.config.show_key_hints {
        widgets::key_hints(
            canvas,
            Rect::new(rect.x + 1, rect.y, hint_width, 1),
            &hints,
            theme,
            "  ",
        );
    }
    if let Some((text, style)) = right {
        let width = u16::try_from(format::width(&text))
            .unwrap_or(0)
            .min(rect.w / 2);
        let x = rect.right().saturating_sub(width + 1);
        let clean = format::truncate(&text, usize::from(width));
        canvas.text(rect, x, rect.y, &clean, style);
    }
}

fn draw_too_small(canvas: &mut Canvas, app: &App, width: u16, height: u16) {
    let theme = &app.theme;
    let lines: Vec<(String, Style)> = vec![
        ("rtop".to_owned(), Style::fg(theme.title).bold()),
        (String::new(), Style::default()),
        (
            "terminal too small".to_owned(),
            Style::fg(theme.critical).bold(),
        ),
        (
            format!("need {MIN_WIDTH}×{MIN_HEIGHT}, have {width}×{height}"),
            Style::fg(theme.dim),
        ),
        (String::new(), Style::default()),
        ("q quit".to_owned(), Style::fg(theme.accent)),
    ];
    let start_y = height.saturating_sub(lines.len() as u16) / 2;
    for (index, (text, style)) in lines.iter().enumerate() {
        let y = start_y + index as u16;
        if y >= height {
            break;
        }
        canvas.text_aligned(Rect::new(0, y, width, 1), y, text, *style, Align::Center);
    }
}

/// Draw a themed panel with a title and optional right-aligned suffix on the
/// top border. Returns the inner rectangle.
pub(crate) fn panel(
    canvas: &mut Canvas,
    app: &App,
    rect: Rect,
    title: &str,
    focused: bool,
    suffix: Option<(&str, Style)>,
) -> Rect {
    let theme = &app.theme;
    let border_style = if focused {
        Style::fg(theme.title).dim()
    } else {
        Style::fg(theme.border)
    };
    let title_style = Style::fg(theme.title).bold();
    let rounded =
        app.config.rounded_corners && app.graph_symbol() != crate::config::GraphSymbol::Tty;
    let inner = canvas.draw_box(rect, rounded, border_style, Some((title, title_style)));
    if let Some((text, style)) = suffix {
        let title_end = rect
            .x
            .saturating_add(3)
            .saturating_add(u16::try_from(format::width(title)).unwrap_or(0))
            .saturating_add(1);
        if title_end < rect.right().saturating_sub(2) {
            let available = rect.right().saturating_sub(title_end).saturating_sub(2);
            canvas.put(title_end.saturating_sub(1), rect.y, ' ', title_style);
            canvas.text_truncated(rect, title_end, rect.y, text, style, available);
        }
    }
    inner
}

/// Placeholder shown before the first snapshot or when a view has no data.
pub(crate) fn placeholder(canvas: &mut Canvas, app: &App, inner: Rect, text: &str) {
    if inner.is_empty() {
        return;
    }
    let y = inner.y + inner.h / 2;
    canvas.text_aligned(
        Rect::new(inner.x, y, inner.w, 1),
        y,
        text,
        Style::fg(app.theme.dim).dim(),
        Align::Center,
    );
}

pub(crate) fn dim(theme: &Theme) -> Style {
    Style::fg(theme.dim)
}

pub(crate) fn state_color(theme: &Theme, state: crate::platform::ProcessState) -> Rgb {
    use crate::platform::ProcessState;
    match state {
        ProcessState::Running => theme.good,
        ProcessState::DiskSleep | ProcessState::Sleeping | ProcessState::Idle => theme.dim,
        ProcessState::Zombie | ProcessState::Dead => theme.critical,
        ProcessState::Stopped | ProcessState::Tracing => theme.warning,
        _ => theme.accent,
    }
}
