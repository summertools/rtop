//! Memory, swap, and disk panel.

use crate::app::App;
use crate::format;
use crate::theme::{Gradient, Style};
use crate::ui::canvas::{Align, Canvas, Rect};

struct Metric {
    label: &'static str,
    value: String,
    ratio: f32,
    gradient: Gradient,
}

pub fn draw(canvas: &mut Canvas, app: &App, rect: Rect, compact: bool) {
    let theme = &app.theme;
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

    let show_graph = !compact && inner.w >= 92 && inner.h >= 8;
    let (left, right) = if show_graph {
        let left_w = inner.w * 62 / 100;
        (
            Rect::new(inner.x, inner.y, left_w, inner.h),
            Some(Rect::new(
                inner.x + left_w + 1,
                inner.y,
                inner.w.saturating_sub(left_w + 1),
                inner.h,
            )),
        )
    } else {
        (inner, None)
    };

    let metrics = build_metrics(app, memory);
    let show_disks = !snapshot.disks.is_empty();
    if compact {
        let disk_h = if show_disks {
            (1 + snapshot.disks.len().min(2) as u16).min(left.h.saturating_sub(4))
        } else {
            0
        };
        let disk_h = if disk_h >= 2 { disk_h } else { 0 };
        let metric_area_h = left.h.saturating_sub(disk_h);
        let shown_metrics = u16::try_from(metrics.len())
            .unwrap_or(u16::MAX)
            .min(metric_area_h);
        draw_metrics(
            canvas,
            app,
            Rect::new(left.x, left.y, left.w, metric_area_h),
            &metrics,
        );
        let leftover = metric_area_h.saturating_sub(shown_metrics);
        if leftover >= 3 {
            draw_history(
                canvas,
                app,
                Rect::new(left.x, left.y + shown_metrics, left.w, leftover),
            );
        }
        if show_disks {
            draw_disks(
                canvas,
                app,
                Rect::new(left.x, left.y + metric_area_h, left.w, disk_h),
                true,
            );
        }
    } else {
        let disk_h = if show_disks {
            (1 + snapshot.disks.len().min(6) as u16).min(left.h / 2)
        } else {
            0
        };
        let metric_h = u16::try_from(metrics.len())
            .unwrap_or(u16::MAX)
            .min(left.h.saturating_sub(disk_h));
        draw_metrics(
            canvas,
            app,
            Rect::new(left.x, left.y, left.w, metric_h),
            &metrics,
        );
        let mut used_h = metric_h;
        if show_disks && used_h < left.h {
            let disk_actual = disk_h.min(left.h - used_h);
            draw_disks(
                canvas,
                app,
                Rect::new(left.x, left.y + used_h, left.w, disk_actual),
                false,
            );
            used_h += disk_actual;
        }
        let leftover = left.h.saturating_sub(used_h);
        if leftover >= 3 {
            draw_top_memory(
                canvas,
                app,
                Rect::new(left.x, left.y + used_h, left.w, leftover),
            );
        }
    }
    if let Some(right) = right {
        draw_history(canvas, app, right);
    }
}

fn build_metrics(app: &App, memory: &crate::platform::MemorySnapshot) -> Vec<Metric> {
    let theme = &app.theme;
    let total = memory.total.max(1);
    let mut metrics = Vec::with_capacity(6);
    metrics.push(Metric {
        label: "Used",
        value: format::bytes(memory.used),
        ratio: memory.used as f32 / total as f32,
        gradient: theme.memory,
    });
    metrics.push(Metric {
        label: "Available",
        value: format::bytes(memory.available),
        ratio: memory.available as f32 / total as f32,
        gradient: theme.disk,
    });
    if let Some(cached) = memory.cached {
        metrics.push(Metric {
            label: "Cached",
            value: format::bytes(cached),
            ratio: cached as f32 / total as f32,
            gradient: theme.net_down,
        });
    }
    metrics.push(Metric {
        label: "Free",
        value: format::bytes(memory.free),
        ratio: memory.free as f32 / total as f32,
        gradient: Gradient::new(theme.good, theme.good, theme.accent),
    });
    if memory.swap_total > 0 {
        metrics.push(Metric {
            label: "Swap",
            value: format!(
                "{} / {}",
                format::bytes_short(memory.swap_used),
                format::bytes_short(memory.swap_total)
            ),
            ratio: memory.swap_used as f32 / memory.swap_total as f32,
            gradient: theme.net_up,
        });
    }
    metrics
}

fn draw_metrics(canvas: &mut Canvas, app: &App, rect: Rect, metrics: &[Metric]) {
    if rect.is_empty() {
        return;
    }
    let rows = usize::from(rect.h).min(metrics.len());
    for (index, metric) in metrics.iter().take(rows).enumerate() {
        crate::ui::widgets::meter_row(
            canvas,
            Rect::new(rect.x, rect.y + index as u16, rect.w, 1),
            metric.label,
            &metric.value,
            metric.ratio,
            metric.gradient,
            &app.theme,
            app.graph_symbol() == crate::config::GraphSymbol::Tty,
        );
    }
    if rows == 0 {
        super::placeholder(canvas, app, rect, "memory data unavailable");
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
    let percent_width = 5u16;
    let mem_width = 8u16;
    canvas.text_aligned(
        header,
        header.y,
        "memory",
        Style::fg(theme.dim).dim(),
        Align::Right,
    );
    let mut processes: Vec<&crate::platform::ProcessInfo> = snapshot.processes.iter().collect();
    processes.sort_by(|a, b| b.memory.cmp(&a.memory).then_with(|| a.pid.cmp(&b.pid)));
    let total = snapshot.memory.total.max(1);
    for (offset, process) in processes
        .iter()
        .take(usize::from(rect.h.saturating_sub(1)))
        .enumerate()
    {
        let y = rect.y + 1 + offset as u16;
        let row = Rect::new(rect.x, y, rect.w, 1);
        if app.selected_pid == Some(process.pid) {
            canvas.fill(
                row,
                Style {
                    fg: theme.highlight_fg.into(),
                    bg: theme.highlight_bg.into(),
                    ..Style::plain()
                },
            );
        }
        let name_width = row.w.saturating_sub(mem_width + percent_width + 2);
        canvas.text_truncated(
            row,
            row.x,
            y,
            &process.name,
            if app.selected_pid == Some(process.pid) {
                Style::fg(theme.highlight_fg).bold()
            } else {
                Style::fg(theme.foreground)
            },
            name_width,
        );
        canvas.text_aligned(
            Rect::new(
                row.right().saturating_sub(percent_width + mem_width + 1),
                y,
                mem_width,
                1,
            ),
            y,
            &format::bytes_short(process.memory),
            if app.selected_pid == Some(process.pid) {
                Style::fg(theme.highlight_fg)
            } else {
                Style::fg(theme.dim)
            },
            Align::Right,
        );
        canvas.text_aligned(
            Rect::new(
                row.right().saturating_sub(percent_width),
                y,
                percent_width,
                1,
            ),
            y,
            &format::percent(process.memory as f32 * 100.0 / total as f32),
            Style::fg(if app.selected_pid == Some(process.pid) {
                theme.highlight_fg
            } else {
                theme.accent
            }),
            Align::Right,
        );
    }
}

fn draw_history(canvas: &mut Canvas, app: &App, rect: Rect) {
    let theme = &app.theme;
    if rect.h < 3 || rect.w < 12 {
        return;
    }
    let header = Rect::new(rect.x, rect.y, rect.w, 1);
    canvas.text(header, header.x, header.y, "history", super::dim(theme));
    let peak = app.memory_history.max();
    let text = format!("peak {peak:.0}%");
    canvas.text_aligned(
        header,
        header.y,
        &text,
        Style::fg(theme.dim).dim(),
        Align::Right,
    );
    let gutter = if rect.w >= 20 { 5 } else { 0 };
    let graph = Rect::new(
        rect.x,
        rect.y + 1,
        rect.w.saturating_sub(gutter),
        rect.h.saturating_sub(1),
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
    if gutter > 0 {
        for (offset, label) in [(0u16, "100%"), (graph.h.saturating_sub(1), "0%")] {
            canvas.text_aligned(
                Rect::new(
                    rect.right().saturating_sub(gutter),
                    graph.y + offset,
                    gutter,
                    1,
                ),
                graph.y + offset,
                label,
                Style::fg(theme.dim).dim(),
                Align::Right,
            );
        }
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
    let disks = &snapshot.disks;
    let mut drawn = 0usize;
    for disk in disks.iter().take(rows_available) {
        let row = Rect::new(rect.x, rect.y + 1 + drawn as u16, rect.w, 1);
        if compact {
            crate::ui::widgets::meter_row(
                canvas,
                row,
                &disk.mount,
                &format::bytes_short(disk.used),
                disk.used as f32 / disk.total.max(1) as f32,
                theme.disk,
                theme,
                app.graph_symbol() == crate::config::GraphSymbol::Tty,
            );
        } else {
            draw_disk_row(canvas, app, row, disk);
        }
        drawn += 1;
    }
    if disks.len() > drawn && drawn > 0 {
        let row = Rect::new(rect.x, rect.y + 1 + drawn as u16, rect.w, 1);
        if row.y < rect.bottom() {
            canvas.text(
                row,
                row.x,
                row.y,
                &format!("+{} more", disks.len() - drawn),
                Style::fg(theme.dim).dim(),
            );
        }
    }
}

fn draw_disk_row(canvas: &mut Canvas, app: &App, rect: Rect, disk: &crate::platform::DiskSnapshot) {
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
        crate::ui::widgets::bar(
            canvas,
            Rect::new(x, rect.y, bar_w, 1),
            ratio,
            theme.disk,
            theme,
            app.graph_symbol() == crate::config::GraphSymbol::Tty,
        );
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
