//! Memory, swap, and disk panel.
//!
//! The panel is deliberately organised as a small set of visual layers rather
//! than a stack of near-identical meters:
//!
//! * a composition strip showing used, reclaimable cache, and free memory;
//! * a compact fact line for available memory and swap;
//! * a full-width history graph when vertical space allows;
//! * disk and top-process sections below.
//!
//! This keeps the most important value (how full memory is) at the top while
//! avoiding five full-width bars that repeat the same total.

use crate::app::App;
use crate::config::MemStyle;
use crate::format;
use crate::platform::{DiskSnapshot, MemorySnapshot, ProcessInfo};
use crate::theme::{Gradient, Style, Theme};
use crate::ui::canvas::{Align, Canvas, Rect};

/// Memory split used by the composition strip and its legend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Composition {
    used: u64,
    cached: u64,
    free: u64,
    total: u64,
}

impl Composition {
    fn new(memory: &MemorySnapshot) -> Self {
        let total = memory.total;
        let used = memory.used.min(total);
        let available = total.saturating_sub(used);
        let cached = memory.cached.unwrap_or(0).min(available);
        Self {
            used,
            cached,
            free: available.saturating_sub(cached),
            total,
        }
    }
}

struct LegendItem {
    glyph: char,
    label: &'static str,
    value: String,
    short_value: String,
    style: Style,
}

pub fn draw(canvas: &mut Canvas, app: &App, rect: Rect, compact: bool) {
    let theme = &app.theme;
    let dots = app.config.mem_style == MemStyle::Dots;
    let Some(snapshot) = app.snapshot.as_ref() else {
        let inner = super::panel(canvas, app, rect, "mem", !compact, None);
        super::placeholder(canvas, app, inner, "collecting memory data…");
        return;
    };
    let memory = &snapshot.memory;
    if memory.total == 0 {
        let inner = super::panel(canvas, app, rect, "mem", !compact, None);
        super::placeholder(canvas, app, inner, "memory information unavailable");
        return;
    }
    let ratio = memory.used as f32 / memory.total as f32;
    let suffix = format!(
        "{}  {} / {}",
        format::percent(ratio * 100.0),
        format::bytes(memory.used),
        format::bytes(memory.total)
    );
    let inner = super::panel(
        canvas,
        app,
        rect,
        "mem",
        !compact,
        Some((&suffix, Style::fg(theme.title).bold())),
    );
    if inner.is_empty() {
        return;
    }

    let composition = Composition::new(memory);
    let mut y = inner.y;
    let bottom = inner.bottom();

    // The composition strip is the visual anchor. Keep a legend row whenever
    // there is room for it; it explains the three glyph/colour groups without
    // making the user infer them from separate meter rows.
    let hero_h = if inner.h >= 4 && inner.w >= 24 { 2 } else { 1 };
    if y < bottom {
        draw_composition_bar(
            canvas,
            app,
            Rect::new(inner.x, y, inner.w, 1),
            memory,
            composition,
            dots,
        );
        y = y.saturating_add(1);
    }
    if hero_h >= 2 && y < bottom {
        draw_composition_legend(
            canvas,
            app,
            Rect::new(inner.x, y, inner.w, 1),
            memory,
            composition,
            dots,
        );
        y = y.saturating_add(1);
    }

    // A single, quiet fact line is easier to scan than four more meters.
    if y < bottom && inner.w >= 20 {
        draw_facts(canvas, app, Rect::new(inner.x, y, inner.w, 1), memory);
        y = y.saturating_add(1);
    }

    let mut remaining = bottom.saturating_sub(y);
    if remaining == 0 {
        return;
    }
    if !compact {
        let has_lower = !snapshot.disks.is_empty() || !snapshot.processes.is_empty();
        let min_lower = if has_lower { 8 } else { 0 };
        let history_h = if inner.w >= 28 && remaining >= 6 {
            let budget = remaining.saturating_sub(min_lower);
            if budget >= 3 {
                (remaining / 3).clamp(3, 7).min(budget)
            } else {
                0
            }
        } else {
            0
        };
        if history_h > 0 {
            draw_history(
                canvas,
                app,
                Rect::new(inner.x, y, inner.w, history_h),
                false,
            );
            y = y.saturating_add(history_h);
            remaining = remaining.saturating_sub(history_h);
        }
    }
    if remaining == 0 {
        return;
    }
    let lower = Rect::new(inner.x, y, inner.w, remaining);
    if compact {
        draw_compact_lower(canvas, app, lower, snapshot);
    } else {
        draw_full_lower(canvas, app, lower, snapshot);
    }
}

fn draw_composition_bar(
    canvas: &mut Canvas,
    app: &App,
    rect: Rect,
    memory: &MemorySnapshot,
    composition: Composition,
    dots: bool,
) {
    if rect.is_empty() || rect.w < 4 || memory.total == 0 {
        return;
    }
    let theme = &app.theme;
    let total = composition.total.max(1);
    let to_cells = |value: u64| -> u16 {
        let cells = (value as f64 / total as f64 * f64::from(rect.w)).round();
        u16::try_from(cells as u64).unwrap_or(rect.w).min(rect.w)
    };
    let used_cells = to_cells(composition.used);
    let cached_cells = to_cells(composition.cached).min(rect.w.saturating_sub(used_cells));
    let tty = app.graph_symbol() == crate::config::GraphSymbol::Tty;
    let (used_ch, cached_ch, free_ch) = if dots {
        if tty {
            ('#', 'O', '.')
        } else {
            ('●', '◉', '·')
        }
    } else if tty {
        ('#', '=', '-')
    } else {
        ('█', '▓', '░')
    };
    for index in 0..rect.w {
        let position = f32::from(index) / f32::from(rect.w.max(1));
        let (ch, style) = if index < used_cells {
            (used_ch, Style::fg(theme.memory.at(position)))
        } else if index < used_cells.saturating_add(cached_cells) {
            (cached_ch, Style::fg(theme.disk.at(0.55)))
        } else {
            (free_ch, Style::fg(theme.border).dim())
        };
        canvas.put(rect.x + index, rect.y, ch, style);
    }
}

fn draw_composition_legend(
    canvas: &mut Canvas,
    app: &App,
    rect: Rect,
    memory: &MemorySnapshot,
    composition: Composition,
    dots: bool,
) {
    if rect.is_empty() || rect.w < 12 {
        return;
    }
    let theme = &app.theme;
    let tty = app.graph_symbol() == crate::config::GraphSymbol::Tty;
    let (used_ch, cached_ch, free_ch) = if dots {
        if tty {
            ('#', 'O', '.')
        } else {
            ('●', '◉', '·')
        }
    } else if tty {
        ('#', '=', '-')
    } else {
        ('█', '▓', '░')
    };
    let mut items = vec![LegendItem {
        glyph: used_ch,
        label: "used",
        value: format::bytes(composition.used),
        short_value: format::bytes_short(composition.used),
        style: Style::fg(theme.memory.at(0.55)).bold(),
    }];
    if memory.cached.is_some() {
        items.push(LegendItem {
            glyph: cached_ch,
            label: "cached",
            value: format::bytes(composition.cached),
            short_value: format::bytes_short(composition.cached),
            style: Style::fg(theme.disk.at(0.55)),
        });
    }
    items.push(LegendItem {
        glyph: free_ch,
        label: "free",
        value: format::bytes(composition.free),
        short_value: format::bytes_short(composition.free),
        style: Style::fg(theme.dim),
    });

    let use_full = legend_width(&items, true) <= usize::from(rect.w);
    let mut x = rect.x;
    for (index, item) in items.iter().enumerate() {
        let separator = if index == 0 { 0 } else { 3 };
        let item_width = if use_full {
            legend_item_width(item, true)
        } else {
            legend_item_width(item, false)
        };
        if usize::from(x.saturating_sub(rect.x))
            .saturating_add(separator)
            .saturating_add(item_width)
            > usize::from(rect.w)
        {
            break;
        }
        if separator > 0 {
            x = x.saturating_add(canvas.text(
                rect,
                x,
                rect.y,
                " │ ",
                Style::fg(theme.border).dim(),
            ));
        }
        x = x.saturating_add(canvas.text(rect, x, rect.y, &format!("{} ", item.glyph), item.style));
        if use_full {
            x = x.saturating_add(canvas.text(rect, x, rect.y, item.label, Style::fg(theme.dim)));
            x = x.saturating_add(1);
            x = x.saturating_add(canvas.text(rect, x, rect.y, &item.value, item.style));
        } else {
            x = x.saturating_add(canvas.text(rect, x, rect.y, &item.short_value, item.style));
        }
    }
}

fn legend_width(items: &[LegendItem], full: bool) -> usize {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| legend_item_width(item, full) + if index == 0 { 0 } else { 3 })
        .sum()
}

fn legend_item_width(item: &LegendItem, full: bool) -> usize {
    if full {
        2 + format::width(item.label) + 1 + format::width(&item.value)
    } else {
        2 + format::width(&item.short_value)
    }
}

fn draw_facts(canvas: &mut Canvas, app: &App, rect: Rect, memory: &MemorySnapshot) {
    if rect.is_empty() || rect.w < 16 {
        return;
    }
    let theme = &app.theme;
    let full = rect.w >= 44;
    let available_label = if full { "available" } else { "avail" };
    let available = if full {
        format::bytes(memory.available)
    } else {
        format::bytes_short(memory.available)
    };
    let swap_ratio = if memory.swap_total > 0 {
        (memory.swap_used as f32 / memory.swap_total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let swap_value = if memory.swap_total == 0 {
        "off".to_owned()
    } else if full {
        format!(
            "{} / {} ({})",
            format::bytes_short(memory.swap_used),
            format::bytes_short(memory.swap_total),
            format::percent(swap_ratio * 100.0)
        )
    } else {
        format::percent(swap_ratio * 100.0)
    };
    let swap_style = if memory.swap_total == 0 {
        Style::fg(theme.dim)
    } else {
        Style::fg(theme.net_up.at(swap_ratio)).bold()
    };

    let mut x = rect.x;
    x = x.saturating_add(canvas.text(rect, x, rect.y, available_label, Style::fg(theme.dim)));
    x = x.saturating_add(1);
    x = x.saturating_add(canvas.text(rect, x, rect.y, &available, Style::fg(theme.accent)));

    let swap = format!("swap {swap_value}");
    if x < rect.right() && format::width(&swap) + 3 <= usize::from(rect.right().saturating_sub(x)) {
        x = x.saturating_add(canvas.text(rect, x, rect.y, " · ", Style::fg(theme.border).dim()));
        x = x.saturating_add(canvas.text(rect, x, rect.y, "swap ", Style::fg(theme.dim)));
        canvas.text(rect, x, rect.y, &swap_value, swap_style);
    }
}

fn draw_compact_lower(
    canvas: &mut Canvas,
    app: &App,
    rect: Rect,
    snapshot: &crate::platform::Snapshot,
) {
    if rect.is_empty() {
        return;
    }
    let mut y = rect.y;
    let mut remaining = rect.h;
    if !snapshot.disks.is_empty() && remaining >= 2 {
        // Keep the dashboard tile airy: two disks at most, then a sparkline.
        let rows = snapshot.disks.len().min(2) as u16;
        let disk_h = (1 + rows).min(remaining);
        if disk_h >= 2 {
            draw_disks(canvas, app, Rect::new(rect.x, y, rect.w, disk_h), true);
            y = y.saturating_add(disk_h);
            remaining = remaining.saturating_sub(disk_h);
        }
    }
    if remaining >= 3 && rect.w >= 24 {
        draw_history(
            canvas,
            app,
            Rect::new(rect.x, y, rect.w, remaining.min(6)),
            true,
        );
    }
}

fn draw_full_lower(
    canvas: &mut Canvas,
    app: &App,
    rect: Rect,
    snapshot: &crate::platform::Snapshot,
) {
    if rect.is_empty() {
        return;
    }
    let show_disks = !snapshot.disks.is_empty();
    let show_processes = !snapshot.processes.is_empty();
    if !show_disks && !show_processes {
        return;
    }

    // Keep a vertical reading order: capacity, history, disks, then the
    // processes that account for the used memory. Full-width disk rows also
    // leave room for per-disk I/O rates on wide terminals.
    let mut y = rect.y;
    if show_disks {
        let reserve = if show_processes { 3 } else { 0 };
        let max_disk_rows = if show_processes {
            6
        } else {
            usize::from(rect.h)
        };
        let disk_h = (1 + snapshot.disks.len().min(max_disk_rows) as u16)
            .min(rect.h.saturating_sub(reserve));
        // A two-row disk section would only have room for its header and a
        // "+N more" line; skip it instead of teasing the user.
        if disk_h >= 3 {
            draw_disks(canvas, app, Rect::new(rect.x, y, rect.w, disk_h), false);
            y = y.saturating_add(disk_h);
        }
    }
    if show_processes && y < rect.bottom() {
        draw_top_memory(
            canvas,
            app,
            Rect::new(rect.x, y, rect.w, rect.bottom().saturating_sub(y)),
        );
    }
}

fn draw_history(canvas: &mut Canvas, app: &App, rect: Rect, compact: bool) {
    if rect.h < 2 || rect.w < 20 {
        return;
    }
    let theme = &app.theme;
    let header = Rect::new(rect.x, rect.y, rect.w, 1);
    canvas.text(
        header,
        header.x,
        header.y,
        "history",
        Style::fg(theme.dim).dim(),
    );
    let peak = app.memory_history.max();
    let average = app.memory_history.average();
    let now = app.memory_history.last();
    let text = if compact || rect.w < 46 {
        format!("peak {peak:.0}%")
    } else {
        format!("now {now:.0}%  peak {peak:.0}%  avg {average:.0}%")
    };
    canvas.text_aligned(
        header,
        header.y,
        &text,
        Style::fg(theme.dim).dim(),
        Align::Right,
    );

    let graph_area = Rect::new(rect.x, rect.y + 1, rect.w, rect.h - 1);
    let show_scale = !compact && graph_area.w >= 32 && graph_area.h >= 3;
    let gutter = if show_scale { 6 } else { 0 };
    let graph = Rect::new(
        graph_area.x,
        graph_area.y,
        graph_area.w.saturating_sub(gutter),
        graph_area.h,
    );
    crate::ui::graph::draw(
        canvas,
        graph,
        &app.memory_history,
        100.0,
        theme.memory,
        app.graph_symbol(),
        super::dim(theme),
    );
    if show_scale {
        draw_scale(
            canvas,
            Rect::new(
                graph_area.right().saturating_sub(gutter),
                graph_area.y,
                gutter,
                graph_area.h,
            ),
            theme,
        );
    }
}

fn draw_scale(canvas: &mut Canvas, rect: Rect, theme: &Theme) {
    if rect.h < 2 {
        return;
    }
    let mut labels = vec![(0u16, "100%"), (rect.h - 1, "0%")];
    if rect.h >= 4 {
        labels.push((rect.h / 2, "50%"));
    }
    for (offset, text) in labels {
        let y = rect.y + offset.min(rect.h - 1);
        canvas.text_aligned(rect, y, text, Style::fg(theme.dim).dim(), Align::Right);
    }
}

fn draw_disks(canvas: &mut Canvas, app: &App, rect: Rect, compact: bool) {
    let Some(snapshot) = app.snapshot.as_ref() else {
        return;
    };
    if rect.is_empty() || snapshot.disks.is_empty() {
        return;
    }
    let theme = &app.theme;
    let header = Rect::new(rect.x, rect.y, rect.w, 1);
    canvas.text(
        header,
        header.x,
        header.y,
        "disk",
        Style::fg(theme.title).bold(),
    );
    let total_read: f64 = snapshot.disks.iter().map(|disk| disk.read_rate).sum();
    let total_write: f64 = snapshot.disks.iter().map(|disk| disk.write_rate).sum();
    let io = format!(
        "R {}  W {}",
        format::rate(total_read),
        format::rate(total_write)
    );
    if format::width(&io) + 8 < usize::from(rect.w) {
        canvas.text_aligned(header, header.y, &io, Style::fg(theme.dim), Align::Right);
    }
    let rows_available = usize::from(rect.h.saturating_sub(1));
    if rows_available == 0 {
        return;
    }
    // A compact dashboard tile is a summary, not a scrollable list: draw the
    // available rows and let the full memory view expose every disk. Reserving
    // a "+N more" row here would hide a second useful disk.
    let show_more = !compact && snapshot.disks.len() > rows_available;
    let rows = if show_more {
        rows_available.saturating_sub(1)
    } else {
        rows_available
    };
    for (index, disk) in snapshot.disks.iter().take(rows).enumerate() {
        let row = Rect::new(rect.x, rect.y + 1 + index as u16, rect.w, 1);
        if compact {
            draw_mem_meter(
                canvas,
                app,
                row,
                &disk.mount,
                &format::bytes_short(disk.used),
                disk.used as f32 / disk.total.max(1) as f32,
                theme.disk,
            );
        } else {
            draw_disk_row(canvas, app, row, disk);
        }
    }
    if show_more {
        let drawn = rows;
        let row = Rect::new(rect.x, rect.y + 1 + drawn as u16, rect.w, 1);
        if row.y < rect.bottom() {
            canvas.text(
                row,
                row.x,
                row.y,
                &format!("+{} more", snapshot.disks.len() - drawn),
                Style::fg(theme.dim).dim(),
            );
        }
    }
}

fn draw_top_memory(canvas: &mut Canvas, app: &App, rect: Rect) {
    let Some(snapshot) = app.snapshot.as_ref() else {
        return;
    };
    if rect.h < 2 || rect.w < 24 {
        return;
    }
    let theme = &app.theme;
    let header = Rect::new(rect.x, rect.y, rect.w, 1);
    canvas.text(
        header,
        header.x,
        header.y,
        "top memory",
        Style::fg(theme.title).bold(),
    );

    let percent_w = 5u16;
    let value_w = 7u16;
    let rank_w = 2u16;
    let value_x = rect.right().saturating_sub(percent_w + 1 + value_w);
    let percent_x = rect.right().saturating_sub(percent_w);
    if rect.w >= 42 {
        canvas.text_aligned(
            Rect::new(value_x, header.y, value_w, 1),
            header.y,
            "memory",
            Style::fg(theme.dim).dim(),
            Align::Right,
        );
        canvas.text_aligned(
            Rect::new(percent_x, header.y, percent_w, 1),
            header.y,
            "share",
            Style::fg(theme.dim).dim(),
            Align::Right,
        );
    }

    let mut processes: Vec<&ProcessInfo> = snapshot.processes.iter().collect();
    processes.sort_by(|a, b| b.memory.cmp(&a.memory).then_with(|| a.pid.cmp(&b.pid)));
    let total = snapshot.memory.total.max(1);
    let capacity = usize::from(rect.h.saturating_sub(1));
    let show_more = processes.len() > capacity;
    let rows = if show_more {
        capacity.saturating_sub(1)
    } else {
        capacity
    };

    let bar_w = if rect.w >= 100 {
        20
    } else if rect.w >= 70 {
        12
    } else if rect.w >= 44 {
        6
    } else {
        0
    };
    let bar_x = if bar_w > 0 {
        value_x.saturating_sub(bar_w + 1)
    } else {
        value_x
    };
    let name_x = rect.x.saturating_add(rank_w + 1);
    let name_w = bar_x.saturating_sub(name_x).saturating_sub(1);

    for (index, process) in processes.iter().take(rows).enumerate() {
        let y = rect.y + 1 + index as u16;
        let row = Rect::new(rect.x, y, rect.w, 1);
        let selected = app.selected_pid == Some(process.pid);
        if selected {
            canvas.fill(
                row,
                Style {
                    fg: theme.highlight_fg.into(),
                    bg: theme.highlight_bg.into(),
                    ..Style::plain()
                },
            );
        }
        let primary = if selected {
            Style::fg(theme.highlight_fg).bold()
        } else {
            Style::fg(theme.foreground)
        };
        let secondary = if selected {
            Style::fg(theme.highlight_fg)
        } else {
            Style::fg(theme.dim)
        };
        let accent = if selected {
            Style::fg(theme.highlight_fg).bold()
        } else {
            Style::fg(theme.accent)
        };

        canvas.text_aligned(
            Rect::new(rect.x, y, rank_w, 1),
            y,
            &format!("{:>2}", index + 1),
            secondary,
            Align::Right,
        );
        if name_w > 0 {
            canvas.text_truncated(row, name_x, y, &process.name, primary, name_w);
        }
        let ratio = (process.memory as f32 / total as f32).clamp(0.0, 1.0);
        if bar_w > 0 {
            crate::ui::widgets::bar(
                canvas,
                Rect::new(bar_x, y, bar_w, 1),
                ratio,
                theme.memory,
                theme,
                app.graph_symbol() == crate::config::GraphSymbol::Tty,
            );
        }
        canvas.text_aligned(
            Rect::new(value_x, y, value_w, 1),
            y,
            &format::bytes_short(process.memory),
            secondary,
            Align::Right,
        );
        canvas.text_aligned(
            Rect::new(percent_x, y, percent_w, 1),
            y,
            &format::percent(ratio * 100.0),
            accent,
            Align::Right,
        );
    }
    if show_more {
        let y = rect.y + 1 + rows as u16;
        if y < rect.bottom() {
            canvas.text(
                Rect::new(rect.x, y, rect.w, 1),
                rect.x,
                y,
                &format!("+{} more", processes.len() - rows),
                Style::fg(theme.dim).dim(),
            );
        }
    }
}

fn draw_mem_meter(
    canvas: &mut Canvas,
    app: &App,
    rect: Rect,
    label: &str,
    value: &str,
    ratio: f32,
    gradient: Gradient,
) {
    if app.config.mem_style == MemStyle::Dots {
        crate::ui::widgets::dot_meter_row(
            canvas,
            rect,
            label,
            value,
            ratio,
            gradient,
            &app.theme,
            app.graph_symbol() == crate::config::GraphSymbol::Tty,
        );
    } else {
        crate::ui::widgets::meter_row(
            canvas,
            rect,
            label,
            value,
            ratio,
            gradient,
            &app.theme,
            app.graph_symbol() == crate::config::GraphSymbol::Tty,
        );
    }
}

fn draw_disk_row(canvas: &mut Canvas, app: &App, rect: Rect, disk: &DiskSnapshot) {
    let theme = &app.theme;
    if rect.is_empty() || rect.w < 20 {
        return;
    }
    let ratio = disk.used as f32 / disk.total.max(1) as f32;
    let show_io = rect.w >= 96;
    let io_w = if show_io { 16 } else { 0 };
    let value_w = 15u16;
    let percent_w = 4u16;
    let label_w = (rect.w / 5).clamp(8, 16);
    let bar_w = rect
        .w
        .saturating_sub(label_w + value_w + percent_w + io_w + 3)
        .min(28);
    let mut x = rect.x;
    let sanitized_mount = format::sanitize(&disk.mount);
    let label = format::truncate(&sanitized_mount, usize::from(label_w));
    canvas.text(rect, x, rect.y, &label, Style::fg(theme.foreground));
    x = rect.x + label_w + 1;
    if bar_w >= 4 {
        let bar_rect = Rect::new(x, rect.y, bar_w, 1);
        if app.config.mem_style == MemStyle::Dots {
            crate::ui::widgets::dot_bar(
                canvas,
                bar_rect,
                ratio,
                theme.disk,
                theme,
                app.graph_symbol() == crate::config::GraphSymbol::Tty,
            );
        } else {
            crate::ui::widgets::bar(
                canvas,
                bar_rect,
                ratio,
                theme.disk,
                theme,
                app.graph_symbol() == crate::config::GraphSymbol::Tty,
            );
        }
    }
    let percent_x = rect.right().saturating_sub(io_w + value_w + percent_w + 1);
    canvas.text_aligned(
        Rect::new(percent_x, rect.y, percent_w, 1),
        rect.y,
        &format::percent(ratio * 100.0),
        Style::fg(theme.foreground),
        Align::Right,
    );
    let value = format!(
        "{}/{}",
        format::bytes_short(disk.used),
        format::bytes_short(disk.total)
    );
    canvas.text_aligned(
        Rect::new(
            rect.right().saturating_sub(io_w + value_w),
            rect.y,
            value_w,
            1,
        ),
        rect.y,
        &value,
        Style::fg(theme.dim),
        Align::Right,
    );
    if show_io {
        let io = format!(
            "R {} W {}",
            format::rate_short(disk.read_rate),
            format::rate_short(disk.write_rate)
        );
        canvas.text_aligned(
            Rect::new(rect.right().saturating_sub(io_w), rect.y, io_w, 1),
            rect.y,
            &io,
            Style::fg(theme.dim).dim(),
            Align::Right,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory(total: u64, used: u64, available: u64, cached: Option<u64>) -> MemorySnapshot {
        MemorySnapshot {
            total,
            used,
            available,
            free: available,
            cached,
            ..MemorySnapshot::default()
        }
    }

    #[test]
    fn composition_segments_fill_the_total_without_underflow() {
        let normal = Composition::new(&memory(100, 40, 60, Some(20)));
        assert_eq!(normal.used, 40);
        assert_eq!(normal.cached, 20);
        assert_eq!(normal.free, 40);
        assert_eq!(normal.used + normal.cached + normal.free, normal.total);

        let over_cached = Composition::new(&memory(100, 70, 30, Some(50)));
        assert_eq!(over_cached.cached, 30);
        assert_eq!(over_cached.free, 0);
        assert_eq!(
            over_cached.used + over_cached.cached + over_cached.free,
            over_cached.total
        );

        let no_cache = Composition::new(&memory(100, 60, 40, None));
        assert_eq!(no_cache.cached, 0);
        assert_eq!(no_cache.free, 40);
        assert_eq!(no_cache.used + no_cache.free, no_cache.total);

        let oversized = Composition::new(&memory(100, 150, 0, Some(50)));
        assert_eq!(oversized.used, 100);
        assert_eq!(oversized.cached, 0);
        assert_eq!(oversized.free, 0);
    }
}
