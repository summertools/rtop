//! Network panel with download/upload graphs and an interface list.

use std::fmt::Write as _;

use crate::app::{App, Layout};
use crate::format;
use crate::platform::NetworkSnapshot;
use crate::theme::Style;
use crate::ui::canvas::{Align, Canvas, Rect};

pub fn draw(canvas: &mut Canvas, app: &App, rect: Rect, compact: bool, layout: &mut Layout) {
    let theme = &app.theme;
    let Some(snapshot) = app.snapshot.as_ref() else {
        let inner = super::panel(canvas, app, rect, "net", !compact, None);
        super::placeholder(canvas, app, inner, "collecting network data…");
        return;
    };
    let Some(network) = app.selected_network() else {
        let inner = super::panel(canvas, app, rect, "net", !compact, None);
        super::placeholder(canvas, app, inner, "no network interfaces");
        return;
    };
    let suffix = format!(
        "{}  ↓ {}  ↑ {}",
        format::sanitize(&network.name),
        format::rate_short(network.received_rate),
        format::rate_short(network.transmitted_rate)
    );
    let inner = super::panel(
        canvas,
        app,
        rect,
        "net",
        !compact,
        Some((&suffix, Style::fg(theme.title).bold())),
    );
    if inner.is_empty() {
        return;
    }

    let show_interfaces = !compact && inner.w >= 76 && inner.h >= 5;
    let (graph_area, iface_area) = if show_interfaces {
        let list_w = (inner.w / 3).clamp(24, 40);
        let graph_w = inner.w.saturating_sub(list_w + 1);
        (
            Rect::new(inner.x, inner.y, graph_w, inner.h),
            Some(Rect::new(
                inner.x + graph_w + 1,
                inner.y,
                inner.w.saturating_sub(graph_w + 1),
                inner.h,
            )),
        )
    } else {
        (inner, None)
    };

    let history = app.network_history();
    let panes = graph_area.rows(2);
    if let Some(top) = panes.first() {
        draw_direction(
            canvas,
            app,
            *top,
            true,
            network.received_rate,
            network.total_received,
            history.map(|history| &history.rx),
            compact,
        );
    }
    if let Some(bottom) = panes.get(1) {
        draw_direction(
            canvas,
            app,
            *bottom,
            false,
            network.transmitted_rate,
            network.total_transmitted,
            history.map(|history| &history.tx),
            compact,
        );
    }
    if let Some(iface_area) = iface_area {
        draw_interfaces(canvas, app, iface_area, &snapshot.networks, network, layout);
    }
}

fn draw_direction(
    canvas: &mut Canvas,
    app: &App,
    rect: Rect,
    download: bool,
    rate: f64,
    total: u64,
    history: Option<&crate::ui::graph::GraphData>,
    compact: bool,
) {
    if rect.is_empty() {
        return;
    }
    let theme = &app.theme;
    let gradient = if download {
        theme.net_down
    } else {
        theme.net_up
    };
    let symbol = if download { "↓" } else { "↑" };
    let label = if compact {
        symbol.to_owned()
    } else if download {
        format!("{symbol} download")
    } else {
        format!("{symbol} upload")
    };
    let header = Rect::new(rect.x, rect.y, rect.w, 1);
    let mut x = header.x;
    x = x.saturating_add(canvas.text(
        header,
        x,
        header.y,
        &label,
        Style::fg(gradient.at(0.7)).bold(),
    ));
    x = x.saturating_add(canvas.text(header, x, header.y, " ", Style::plain()));
    x = x.saturating_add(canvas.text(
        header,
        x,
        header.y,
        &format::rate(rate),
        Style::fg(theme.foreground).bold(),
    ));
    if header.w >= 48 {
        canvas.text(
            header,
            x.saturating_add(1),
            header.y,
            &format!("({})", format::bit_rate(rate)),
            Style::fg(theme.dim).dim(),
        );
    }
    let total_text = format!("total {}", format::bytes(total));
    if header.w >= 60 {
        canvas.text_aligned(
            header,
            header.y,
            &total_text,
            Style::fg(theme.dim),
            Align::Right,
        );
    }
    if rect.h < 2 {
        return;
    }

    let graph_height = rect.h - 1;
    let gutter = if rect.w >= 30 && graph_height >= 2 {
        6
    } else {
        0
    };
    let graph_rect = Rect::new(
        rect.x,
        rect.y + 1,
        rect.w.saturating_sub(gutter),
        graph_height,
    );
    let history = history.cloned().unwrap_or_default();
    let max = crate::ui::graph::nice_rate_scale(history.max().max(rate).max(1024.0));
    crate::ui::graph::draw(
        canvas,
        graph_rect,
        &history,
        max,
        gradient,
        app.graph_symbol(),
        super::dim(theme),
    );
    if gutter > 0 {
        let scale_rect = Rect::new(rect.right() - gutter, rect.y + 1, gutter, graph_height);
        draw_scale(canvas, app, scale_rect, max);
    }
    // Keep a tiny current-rate marker on the top-left of the graph.
    if graph_rect.w >= 20 && graph_rect.h >= 2 {
        let text = format::rate(rate);
        canvas.text_truncated(
            graph_rect,
            graph_rect.x,
            graph_rect.y,
            &text,
            Style::fg(gradient.at(0.8)),
            18,
        );
    }
}

fn draw_scale(canvas: &mut Canvas, app: &App, rect: Rect, max: f64) {
    if rect.h < 2 {
        return;
    }
    let mut labels = vec![(0u16, max)];
    if rect.h >= 4 {
        labels.push((rect.h / 2, max / 2.0));
    }
    labels.push((rect.h - 1, 0.0));
    for (offset, value) in labels {
        if offset >= rect.h {
            continue;
        }
        let text = if value <= 0.0 {
            "0".to_owned()
        } else {
            format::rate_short(value)
        };
        canvas.text_aligned(
            Rect::new(rect.x, rect.y + offset, rect.w, 1),
            rect.y + offset,
            &text,
            Style::fg(app.theme.dim).dim(),
            Align::Right,
        );
    }
}

fn draw_interfaces(
    canvas: &mut Canvas,
    app: &App,
    rect: Rect,
    networks: &[NetworkSnapshot],
    selected: &NetworkSnapshot,
    layout: &mut Layout,
) {
    let theme = &app.theme;
    if rect.is_empty() {
        return;
    }
    let header = Rect::new(rect.x, rect.y, rect.w, 1);
    canvas.text(header, header.x, header.y, "interfaces", super::dim(theme));
    let auto = if app.net_auto { "auto" } else { "manual" };
    canvas.text_aligned(
        header,
        header.y,
        auto,
        Style::fg(theme.dim).dim(),
        Align::Right,
    );
    let mut row_y = rect.y + 1;
    let bottom = rect.bottom().saturating_sub(2);
    for network in networks {
        if row_y >= bottom {
            break;
        }
        let row = Rect::new(rect.x, row_y, rect.w, 1);
        let is_selected = network.name == selected.name;
        if is_selected {
            canvas.fill(
                row,
                Style {
                    fg: theme.highlight_fg.into(),
                    bg: theme.highlight_bg.into(),
                    ..Style::plain()
                },
            );
        }
        let marker = if network.loopback {
            "○"
        } else if network.up == Some(false) {
            "◌"
        } else {
            "●"
        };
        let marker_color = if network.loopback || network.up == Some(false) {
            theme.dim
        } else {
            theme.good
        };
        canvas.text(row, row.x, row.y, marker, Style::fg(marker_color));
        let rates = format!(
            "↓{} ↑{}",
            format::rate_short(network.received_rate),
            format::rate_short(network.transmitted_rate)
        );
        let rates_width = u16::try_from(format::width(&rates))
            .unwrap_or(0)
            .min(row.w / 2);
        let name_width = row.w.saturating_sub(rates_width + 4).max(3);
        canvas.text_truncated(
            row,
            row.x + 2,
            row.y,
            &network.name,
            if is_selected {
                Style::fg(theme.highlight_fg).bold()
            } else {
                Style::fg(theme.foreground)
            },
            name_width.min(row.w.saturating_sub(2)),
        );
        canvas.text_aligned(
            row,
            row.y,
            &rates,
            if is_selected {
                Style::fg(theme.highlight_fg)
            } else {
                Style::fg(theme.dim)
            },
            Align::Right,
        );
        layout.net_ifaces.push((network.name.clone(), row));
        row_y = row_y.saturating_add(1);
    }

    // Selected interface details.
    let detail_y = rect.bottom().saturating_sub(2);
    if rect.h >= 6 && detail_y > row_y {
        let detail = Rect::new(rect.x, detail_y, rect.w, 2);
        let mut text = String::new();
        if let Some(address) = selected.addresses.first() {
            text.push_str(&format::sanitize(address));
        }
        if let Some(speed) = selected.speed_mbps {
            if !text.is_empty() {
                text.push_str(" · ");
            }
            if speed >= 1000 {
                let _ = write!(text, "{:.1} Gbps", speed as f64 / 1000.0);
            } else {
                let _ = write!(text, "{speed} Mbps");
            }
        }
        if selected.wireless {
            text.push_str(" · wireless");
        }
        if !selected.mac.is_empty() {
            text.push_str(" · ");
            text.push_str(&format::sanitize(&selected.mac));
        }
        canvas.text_truncated(
            detail,
            detail.x,
            detail.y,
            &text,
            Style::fg(theme.dim),
            detail.w,
        );
        canvas.text_truncated(
            detail,
            detail.x,
            detail.y + 1,
            "[ ] select · a auto",
            Style::fg(theme.dim).dim(),
            detail.w,
        );
    }
}
