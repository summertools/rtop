//! CPU panel.

use crate::app::App;
use crate::config::GraphSymbol;
use crate::format;
use crate::theme::Style;
use crate::ui::canvas::{Align, Canvas, Rect};

pub fn draw(canvas: &mut Canvas, app: &App, rect: Rect, compact: bool) {
    let theme = &app.theme;
    let Some(snapshot) = app.snapshot.as_ref() else {
        let inner = super::panel(canvas, app, rect, "cpu", !compact, None);
        super::placeholder(canvas, app, inner, "collecting CPU data…");
        return;
    };
    let cpu = &snapshot.cpu;
    let suffix = format!(
        "{}  {}",
        format::percent(cpu.overall_usage),
        if let Some(temp) = cpu.temperature {
            format!("{temp:.0}°C")
        } else {
            format!("{} cores", cpu.cores.len())
        }
    );
    let inner = super::panel(
        canvas,
        app,
        rect,
        "cpu",
        !compact,
        Some((&suffix, Style::fg(theme.title).bold())),
    );
    if inner.is_empty() {
        return;
    }

    let show_header = !compact && inner.h >= 10 && inner.w >= 48;
    let show_stats = inner.h >= 5;
    let header_h = u16::from(show_header);
    let stats_h = u16::from(show_stats);
    let main = Rect::new(
        inner.x,
        inner.y + header_h,
        inner.w,
        inner.h.saturating_sub(header_h + stats_h),
    );
    if main.is_empty() {
        return;
    }

    if show_header {
        draw_header_line(canvas, app, Rect::new(inner.x, inner.y, inner.w, 1));
    }

    let show_cores = inner.w >= 36 && main.h >= 2 && !cpu.cores.is_empty();
    let graph_w = if show_cores {
        (inner.w * 54 / 100).max(20)
    } else {
        inner.w
    };
    let graph_area = Rect::new(main.x, main.y, graph_w, main.h);
    draw_main_graph(canvas, app, graph_area, compact);
    if show_cores {
        let divider_x = main.x + graph_w;
        if divider_x < main.right() {
            canvas.vline(
                main,
                divider_x,
                main.y,
                main.h,
                '│',
                Style::fg(theme.border).dim(),
            );
        }
        let cores_area = Rect::new(
            divider_x + 2,
            main.y,
            inner.right().saturating_sub(divider_x + 2),
            main.h,
        );
        draw_cores(canvas, app, cores_area, compact);
    }

    if show_stats {
        let stats = Rect::new(inner.x, inner.bottom() - 1, inner.w, 1);
        draw_stats(canvas, app, stats);
    }
}

fn draw_header_line(canvas: &mut Canvas, app: &App, rect: Rect) {
    let theme = &app.theme;
    let snapshot = app.snapshot.as_ref().expect("snapshot checked");
    let cpu = &snapshot.cpu;
    let mut x = rect.x;
    let model_width = rect.w.saturating_sub(34).max(10);
    x = x.saturating_add(canvas.text_truncated(
        rect,
        x,
        rect.y,
        &snapshot.host.cpu_model,
        Style::fg(theme.foreground),
        model_width,
    ));
    let mut right = rect.right();
    if cpu.average_frequency_mhz > 0 {
        let text = format!("{:.2} GHz", cpu.average_frequency_mhz as f64 / 1000.0);
        let width = u16::try_from(format::width(&text)).unwrap_or(0);
        right = right.saturating_sub(width);
        canvas.text(rect, right, rect.y, &text, super::dim(theme));
        right = right.saturating_sub(2);
    }
    if let Some(temp) = cpu.temperature {
        let text = format!("{temp:.0}°C");
        let width = u16::try_from(format::width(&text)).unwrap_or(0);
        right = right.saturating_sub(width);
        canvas.text(
            rect,
            right,
            rect.y,
            &text,
            Style::fg(crate::ui::graph::temperature_color(temp, theme.temp)).bold(),
        );
        right = right.saturating_sub(2);
    }
    if cpu.cores.len() > 1 && right > x + 4 {
        let text = format!("{} cores", cpu.cores.len());
        let width = u16::try_from(format::width(&text)).unwrap_or(0);
        if right.saturating_sub(width) > x {
            canvas.text(
                rect,
                right.saturating_sub(width),
                rect.y,
                &text,
                super::dim(theme),
            );
        }
    }
}

fn draw_main_graph(canvas: &mut Canvas, app: &App, area: Rect, compact: bool) {
    let theme = &app.theme;
    if area.is_empty() {
        return;
    }
    let gutter = if area.w >= 28 && area.h >= 2 { 6 } else { 0 };
    let graph = Rect::new(area.x, area.y, area.w.saturating_sub(gutter), area.h);
    crate::ui::graph::draw(
        canvas,
        graph,
        &app.cpu_history,
        100.0,
        theme.cpu,
        app.graph_symbol(),
        super::dim(theme),
    );
    if gutter > 0 {
        draw_scale(
            canvas,
            app,
            Rect::new(area.right() - gutter, area.y, gutter, area.h),
            100.0,
        );
    }
    // Overlay current and average at the top-left, but only when there is
    // enough room to avoid hiding the graph.
    if graph.w >= 16 && graph.h >= 2 {
        let text = if !compact && graph.w >= 24 {
            format!(
                "{}  avg {}  peak {}",
                format::percent(app.cpu_history.last() as f32),
                format::percent(app.cpu_history.average() as f32),
                format::percent(app.cpu_history.max() as f32)
            )
        } else if graph.w >= 24 {
            format!(
                "{}  avg {}",
                format::percent(app.cpu_history.last() as f32),
                format::percent(app.cpu_history.average() as f32)
            )
        } else {
            format::percent(app.cpu_history.last() as f32)
        };
        canvas.text_truncated(
            graph,
            graph.x,
            graph.y,
            &text,
            Style::fg(theme.foreground).bold(),
            graph.w.min(30),
        );
    }
}

fn draw_scale(canvas: &mut Canvas, app: &App, rect: Rect, max: f64) {
    let theme = &app.theme;
    if rect.h < 2 {
        return;
    }
    let mut labels = vec![(0u16, format!("{max:.0}%"))];
    if rect.h >= 4 {
        labels.push((rect.h / 2, format!("{}%", max as u16 / 2)));
    }
    labels.push((rect.h - 1, "0%".to_owned()));
    for (offset, text) in labels {
        let y = rect.y + offset.min(rect.h - 1);
        canvas.text_aligned(rect, y, &text, Style::fg(theme.dim).dim(), Align::Right);
    }
}

fn draw_cores(canvas: &mut Canvas, app: &App, rect: Rect, compact: bool) {
    let theme = &app.theme;
    let Some(snapshot) = app.snapshot.as_ref() else {
        return;
    };
    let cores = &snapshot.cpu.cores;
    if rect.is_empty() || cores.is_empty() {
        return;
    }
    let rows_available = usize::from(rect.h);
    let columns = if rect.w >= 36
        || (!compact && rect.w >= 25)
        || (cores.len() > rows_available && rect.w >= 25)
    {
        2
    } else {
        1
    };
    let per_column = cores.len().div_ceil(columns).max(1);
    let core_height = if compact {
        1
    } else {
        u16::try_from((rows_available / per_column).clamp(1, 8)).unwrap_or(1)
    };
    let rows = usize::from(rect.h / core_height.max(1));
    let capacity = rows * columns;
    let show_more = cores.len() > capacity;
    let draw_capacity = if show_more {
        capacity.saturating_sub(1)
    } else {
        capacity
    };
    let column_width = rect.w / u16::try_from(columns).unwrap_or(1);
    for (drawn, usage) in cores.iter().enumerate().take(draw_capacity) {
        let column = drawn % columns;
        let row = drawn / columns;
        let cell = Rect::new(
            rect.x + u16::try_from(column).unwrap_or(0) * column_width,
            rect.y + u16::try_from(row).unwrap_or(0) * core_height,
            column_width.saturating_sub(1),
            core_height,
        );
        if core_height > 1 {
            draw_core_block(canvas, app, cell, drawn, *usage);
        } else {
            draw_core_line(canvas, app, cell, drawn, *usage, compact);
        }
    }
    if show_more {
        let drawn = draw_capacity;
        let column = drawn % columns;
        let row = drawn / columns;
        let cell = Rect::new(
            rect.x + u16::try_from(column).unwrap_or(0) * column_width,
            rect.y + u16::try_from(row).unwrap_or(0) * core_height,
            column_width.saturating_sub(1),
            core_height,
        );
        let remaining = cores.len() - draw_capacity;
        canvas.text(
            cell,
            cell.x,
            cell.y,
            &format!("+{remaining} cores"),
            Style::fg(theme.dim).dim(),
        );
    }
}

fn draw_core_block(canvas: &mut Canvas, app: &App, rect: Rect, index: usize, usage: f32) {
    if rect.is_empty() || rect.w < 10 || rect.h < 2 {
        return;
    }
    let theme = &app.theme;
    let label = format!("C{index}");
    let label_width = 4u16.min(rect.w.saturating_sub(6));
    canvas.text(rect, rect.x, rect.y, &label, Style::fg(theme.dim));
    let percent_text = format!("{:>3}%", usage.round().clamp(0.0, 100.0) as u16);
    let percent_width = 4u16;
    let bar_width = rect
        .w
        .saturating_sub(label_width + percent_width + 1)
        .max(1);
    if bar_width >= 2 {
        crate::ui::widgets::bar(
            canvas,
            Rect::new(rect.x + label_width, rect.y, bar_width, 1),
            usage / 100.0,
            theme.cpu,
            theme,
            app.graph_symbol() == GraphSymbol::Tty,
        );
    }
    canvas.text_aligned(
        Rect::new(
            rect.right().saturating_sub(percent_width),
            rect.y,
            percent_width,
            1,
        ),
        rect.y,
        &percent_text,
        Style::fg(theme.cpu.at((usage / 100.0).clamp(0.0, 1.0))).bold(),
        Align::Right,
    );
    if let Some(history) = app.core_history.get(index) {
        crate::ui::graph::draw(
            canvas,
            Rect::new(rect.x, rect.y + 1, rect.w, rect.h - 1),
            history,
            100.0,
            theme.cpu,
            app.graph_symbol(),
            super::dim(theme),
        );
    }
}

fn draw_core_line(
    canvas: &mut Canvas,
    app: &App,
    rect: Rect,
    index: usize,
    usage: f32,
    compact: bool,
) {
    if rect.is_empty() || rect.w < 10 {
        return;
    }
    let theme = &app.theme;
    let label = format!("C{index}");
    let label_width = 4u16.min(rect.w.saturating_sub(6));
    canvas.text(rect, rect.x, rect.y, &label, Style::fg(theme.dim));
    let percent_text = format!("{:>3}%", usage.round().clamp(0.0, 100.0) as u16);
    let percent_width = 4u16;
    let bar_width = rect
        .w
        .saturating_sub(label_width + percent_width + 1)
        .max(1);
    let bar_rect = Rect::new(rect.x + label_width, rect.y, bar_width, 1);
    if !compact && bar_width >= 6 && app.core_history.get(index).is_some() {
        crate::ui::graph::draw(
            canvas,
            bar_rect,
            &app.core_history[index],
            100.0,
            theme.cpu,
            app.graph_symbol(),
            super::dim(theme),
        );
    } else {
        crate::ui::widgets::bar(
            canvas,
            bar_rect,
            usage / 100.0,
            theme.cpu,
            theme,
            app.graph_symbol() == GraphSymbol::Tty,
        );
    }
    canvas.text_aligned(
        Rect::new(
            rect.right().saturating_sub(percent_width),
            rect.y,
            percent_width,
            1,
        ),
        rect.y,
        &percent_text,
        Style::fg(theme.cpu.at((usage / 100.0).clamp(0.0, 1.0))).bold(),
        Align::Right,
    );
}

fn draw_stats(canvas: &mut Canvas, app: &App, rect: Rect) {
    let theme = &app.theme;
    let Some(snapshot) = app.snapshot.as_ref() else {
        return;
    };
    let cpu = &snapshot.cpu;
    if let Some(split) = cpu.breakdown {
        let parts = [
            (format!("usr {:.0}%", split.user), theme.cpu.at(0.25)),
            (format!("sys {:.0}%", split.system), theme.cpu.at(0.55)),
            (format!("wait {:.0}%", split.wait), theme.warning),
            (format!("idle {:.0}%", split.idle), theme.good),
        ];
        let mut x = rect.x;
        for (index, (text, color)) in parts.iter().enumerate() {
            if index > 0 {
                x = x.saturating_add(canvas.text(rect, x, rect.y, "  ", Style::plain()));
            }
            x = x.saturating_add(canvas.text(rect, x, rect.y, text, Style::fg(*color)));
        }
    } else {
        let text = format!(
            "avg {}  peak {}  load {:.2} {:.2} {:.2}",
            format::percent(app.cpu_history.average() as f32),
            format::percent(app.cpu_history.max() as f32),
            cpu.load[0],
            cpu.load[1],
            cpu.load[2]
        );
        canvas.text_truncated(rect, rect.x, rect.y, &text, super::dim(theme), rect.w);
    }
}
