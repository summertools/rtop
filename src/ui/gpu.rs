//! GPU panel: utilisation, VRAM, temperature, power, clocks, and history.

use crate::app::App;
use crate::config::GraphSymbol;
use crate::format;
use crate::platform::GpuSnapshot;
use crate::theme::Style;
use crate::ui::canvas::{Align, Canvas, Rect};

pub fn draw(canvas: &mut Canvas, app: &App, rect: Rect, compact: bool) {
    let theme = &app.theme;
    let Some(snapshot) = app.snapshot.as_ref() else {
        let inner = super::panel(canvas, app, rect, "gpu", !compact, None);
        super::placeholder(canvas, app, inner, "collecting GPU data…");
        return;
    };
    if !app.config.show_gpu {
        return;
    }
    if snapshot.gpus.is_empty() {
        let inner = super::panel(
            canvas,
            app,
            rect,
            "gpu",
            !compact,
            Some(("no device", Style::fg(theme.dim))),
        );
        if inner.h >= 2 {
            super::placeholder(canvas, app, inner, "no supported GPU detected");
            canvas.text_aligned(
                Rect::new(inner.x, inner.y + 1, inner.w, 1),
                inner.y + 1,
                "NVIDIA via NVML · AMD/Intel via sysfs",
                Style::fg(theme.dim).dim(),
                Align::Center,
            );
        } else {
            super::placeholder(canvas, app, inner, "no GPU");
        }
        return;
    }

    let count = snapshot.gpus.len();
    let columns = if count > 1 && rect.w >= 100 { 2 } else { 1 };
    let rows = count.div_ceil(columns);
    if count == 1 || rect.h >= 7 * u16::try_from(rows).unwrap_or(u16::MAX) {
        let cell_w = rect.w / u16::try_from(columns).unwrap_or(1);
        let cell_h = rect.h / u16::try_from(rows).unwrap_or(1);
        for (index, gpu) in snapshot.gpus.iter().enumerate() {
            let column = u16::try_from(index % columns).unwrap_or(0);
            let row = u16::try_from(index / columns).unwrap_or(0);
            let x = rect.x + column * cell_w;
            let y = rect.y + row * cell_h;
            let w = if column + 1 == u16::try_from(columns).unwrap_or(1) {
                rect.right().saturating_sub(x)
            } else {
                cell_w
            };
            let h = if row + 1 == u16::try_from(rows).unwrap_or(1) {
                rect.bottom().saturating_sub(y)
            } else {
                cell_h
            };
            draw_card(canvas, app, Rect::new(x, y, w, h), gpu);
        }
    } else {
        draw_list(canvas, app, rect, &snapshot.gpus);
    }
}

fn draw_card(canvas: &mut Canvas, app: &App, rect: Rect, gpu: &GpuSnapshot) {
    let theme = &app.theme;
    let title = format!("gpu{}", gpu.index);
    let utilization = gpu
        .utilization
        .map_or_else(|| "--".to_owned(), |value| format!("{value:.0}%"));
    let suffix = format!("{utilization}  {}", gpu.vendor.label());
    let inner = super::panel(
        canvas,
        app,
        rect,
        &title,
        false,
        Some((&suffix, Style::fg(theme.title).bold())),
    );
    if inner.is_empty() {
        return;
    }
    let tty = app.graph_symbol() == GraphSymbol::Tty;
    let mut y = inner.y;
    let name = super::hardware::gpu_name(gpu);
    if y < inner.bottom() {
        let vendor_label = gpu.vendor.label();
        let vendor = if gpu.driver.is_empty() || gpu.driver.eq_ignore_ascii_case(vendor_label) {
            vendor_label.to_owned()
        } else {
            format!("{vendor_label} · {}", format::sanitize(&gpu.driver))
        };
        let vendor_width = u16::try_from(format::width(&vendor))
            .unwrap_or(0)
            .min(inner.w / 3);
        let name_width = inner.w.saturating_sub(vendor_width + 1);
        canvas.text_truncated(
            inner,
            inner.x,
            y,
            &name,
            Style::fg(theme.foreground).bold(),
            name_width,
        );
        if vendor_width > 0 {
            canvas.text_aligned(
                Rect::new(
                    inner.right().saturating_sub(vendor_width),
                    y,
                    vendor_width,
                    1,
                ),
                y,
                &vendor,
                Style::fg(theme.dim).dim(),
                Align::Right,
            );
        }
        y = y.saturating_add(1);
    }

    if y < inner.bottom() {
        draw_util_row(canvas, app, Rect::new(inner.x, y, inner.w, 1), gpu);
        y = y.saturating_add(1);
    }
    if y < inner.bottom() {
        if let (Some(used), Some(total)) = (gpu.memory_used, gpu.memory_total) {
            let value = format!(
                "{}/{}",
                format::bytes_short(used),
                format::bytes_short(total)
            );
            crate::ui::widgets::meter_row(
                canvas,
                Rect::new(inner.x, y, inner.w, 1),
                "vram",
                &value,
                gpu.memory_ratio().unwrap_or(0.0),
                theme.memory,
                theme,
                tty,
            );
            y = y.saturating_add(1);
        }
    }
    if y < inner.bottom() {
        draw_facts(canvas, app, Rect::new(inner.x, y, inner.w, 1), gpu);
        y = y.saturating_add(1);
    }

    let remaining = inner.bottom().saturating_sub(y);
    if remaining >= 2 {
        let header = Rect::new(inner.x, y, inner.w, 1);
        canvas.text(header, header.x, y, "history", Style::fg(theme.dim).dim());
        let peak = app
            .gpu_history
            .get(&gpu.history_key())
            .map_or(0.0, super::graph::GraphData::max);
        canvas.text_aligned(
            header,
            y,
            &format!("peak {peak:.0}%"),
            Style::fg(theme.dim).dim(),
            Align::Right,
        );
        if let Some(history) = app.gpu_history.get(&gpu.history_key()) {
            crate::ui::graph::draw(
                canvas,
                Rect::new(inner.x, y + 1, inner.w, remaining - 1),
                history,
                100.0,
                theme.cpu,
                app.graph_symbol(),
                super::dim(theme),
            );
        }
    }
}

fn draw_util_row(canvas: &mut Canvas, app: &App, rect: Rect, gpu: &GpuSnapshot) {
    let theme = &app.theme;
    if rect.is_empty() || rect.w < 12 {
        return;
    }
    let label_width = 9u16.min(rect.w);
    let percent_width = 5u16;
    let bar_x = rect.x + label_width;
    let bar_w = rect
        .right()
        .saturating_sub(percent_width + 1)
        .saturating_sub(bar_x);
    canvas.text(rect, rect.x, rect.y, "util", Style::fg(theme.dim));
    if bar_w >= 2 {
        crate::ui::widgets::bar(
            canvas,
            Rect::new(bar_x, rect.y, bar_w, 1),
            gpu.utilization_ratio(),
            theme.cpu,
            theme,
            app.graph_symbol() == GraphSymbol::Tty,
        );
    }
    let text = gpu
        .utilization
        .map_or_else(|| "--".to_owned(), format::percent);
    canvas.text_aligned(
        Rect::new(
            rect.right().saturating_sub(percent_width),
            rect.y,
            percent_width,
            1,
        ),
        rect.y,
        &text,
        Style::fg(theme.cpu.at(gpu.utilization_ratio())).bold(),
        Align::Right,
    );
}

fn draw_facts(canvas: &mut Canvas, app: &App, rect: Rect, gpu: &GpuSnapshot) {
    let theme = &app.theme;
    let mut facts: Vec<(String, Style)> = Vec::new();
    if let Some(temp) = gpu.temperature {
        facts.push((
            format!("{temp:.0}°C"),
            Style::fg(crate::ui::graph::temperature_color(temp, theme.temp)).bold(),
        ));
    }
    if let Some(power) = gpu.power_w {
        facts.push((format!("{power:.0} W"), Style::fg(theme.warning)));
    }
    if let Some(clock) = gpu.core_clock_mhz {
        facts.push((format!("core {clock} MHz"), Style::fg(theme.foreground)));
    }
    if let Some(clock) = gpu.memory_clock_mhz {
        facts.push((format!("mem {clock} MHz"), Style::fg(theme.dim)));
    }
    if let Some(fan) = gpu.fan_percent {
        facts.push((format!("fan {fan:.0}%"), Style::fg(theme.dim)));
    }
    if let Some(location) = super::hardware::gpu_location(gpu) {
        facts.push((location, Style::fg(theme.dim).dim()));
    }
    if facts.is_empty() {
        canvas.text(
            rect,
            rect.x,
            rect.y,
            "monitoring driver does not expose extra sensors",
            Style::fg(theme.dim).dim(),
        );
        return;
    }
    // Drop lower-priority facts instead of clipping a value halfway through.
    let mut width: u16 = facts
        .iter()
        .map(|(text, _)| u16::try_from(format::width(text)).unwrap_or(0))
        .sum::<u16>()
        .saturating_add(3 * u16::try_from(facts.len().saturating_sub(1)).unwrap_or(0));
    while width > rect.w && facts.len() > 1 {
        if let Some((text, _)) = facts.pop() {
            width = width.saturating_sub(
                u16::try_from(format::width(&text))
                    .unwrap_or(0)
                    .saturating_add(3),
            );
        }
    }
    let mut x = rect.x;
    for (index, (text, style)) in facts.iter().enumerate() {
        if index > 0 {
            x = x.saturating_add(canvas.text(rect, x, rect.y, " · ", Style::fg(theme.dim).dim()));
        }
        x = x.saturating_add(canvas.text(rect, x, rect.y, text, *style));
    }
}

fn draw_list(canvas: &mut Canvas, app: &App, rect: Rect, gpus: &[GpuSnapshot]) {
    let theme = &app.theme;
    let suffix = format!(
        "{} device{}",
        gpus.len(),
        if gpus.len() == 1 { "" } else { "s" }
    );
    let inner = super::panel(
        canvas,
        app,
        rect,
        "gpu",
        false,
        Some((&suffix, Style::fg(theme.title).bold())),
    );
    if inner.is_empty() || inner.h < 2 {
        return;
    }
    let header = Rect::new(inner.x, inner.y, inner.w, 1);
    canvas.text(
        header,
        header.x,
        inner.y,
        "DEVICE",
        Style::fg(theme.dim).dim(),
    );
    canvas.text_aligned(
        header,
        inner.y,
        "UTIL  TEMP",
        Style::fg(theme.dim).dim(),
        Align::Right,
    );
    let available = usize::from(inner.h.saturating_sub(1));
    let show_more = gpus.len() > available && available > 0;
    let list_rows = if show_more {
        available.saturating_sub(1)
    } else {
        available
    };
    for (offset, gpu) in gpus.iter().take(list_rows).enumerate() {
        let y = inner.y + 1 + u16::try_from(offset).unwrap_or(0);
        let row = Rect::new(inner.x, y, inner.w, 1);
        let util_width = 5u16.min(row.w);
        let temp_width = 5u16.min(row.w.saturating_sub(util_width));
        let name_width = row.w.saturating_sub(util_width + temp_width + 2).max(3);
        let name = super::hardware::gpu_name(gpu);
        canvas.text_truncated(
            row,
            row.x,
            y,
            &name,
            Style::fg(theme.foreground).bold(),
            name_width,
        );
        let util = gpu
            .utilization
            .map_or_else(|| "--".to_owned(), format::percent);
        canvas.text_aligned(
            Rect::new(
                row.right().saturating_sub(util_width + temp_width + 1),
                y,
                util_width,
                1,
            ),
            y,
            &util,
            Style::fg(theme.cpu.at(gpu.utilization_ratio())).bold(),
            Align::Right,
        );
        if let Some(temp) = gpu.temperature {
            canvas.text_aligned(
                Rect::new(row.right().saturating_sub(temp_width), y, temp_width, 1),
                y,
                &format!("{temp:.0}°"),
                Style::fg(crate::ui::graph::temperature_color(temp, theme.temp)),
                Align::Right,
            );
        }
    }
    if show_more {
        let y = inner.bottom().saturating_sub(1);
        canvas.text(
            Rect::new(inner.x, y, inner.w, 1),
            inner.x,
            y,
            &format!("+{} more", gpus.len() - list_rows),
            Style::fg(theme.dim).dim(),
        );
    }
}
