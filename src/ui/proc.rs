//! Process table.

use crate::app::{App, Layout, ProcRow};
use crate::format;
use crate::theme::Style;
use crate::ui::canvas::{Align, Canvas, Rect};

struct Columns {
    pid_x: u16,
    pid_w: u16,
    name_x: u16,
    name_w: u16,
    command_x: u16,
    command_w: u16,
    user: Option<(u16, u16)>,
    mem: Option<(u16, u16)>,
    cpu_x: u16,
    cpu_w: u16,
}

impl Columns {
    fn new(width: u16) -> Self {
        let pid_w = 9;
        let mut name_w = (width * 18 / 100).clamp(8, 20);
        let mut user_w = if width >= 88 {
            10
        } else if width >= 72 {
            8
        } else {
            0
        };
        let mut mem_w = if width >= 64 { 7 } else { 0 };
        let cpu_w = if width >= 60 { 8 } else { 5 };

        loop {
            let table_w = width;
            let right_base = table_w.saturating_sub(cpu_w);
            let mem_x = if mem_w > 0 {
                right_base.saturating_sub(mem_w + 1)
            } else {
                right_base
            };
            let user_x = if user_w > 0 {
                mem_x.saturating_sub(user_w + 1)
            } else {
                mem_x
            };
            let command_x = pid_w + 1 + name_w + 1;
            let command_w = user_x.saturating_sub(command_x + 1);
            if command_w >= 6 || (user_w == 0 && mem_w == 0 && name_w <= 6) {
                return Self {
                    pid_x: 0,
                    pid_w,
                    name_x: pid_w + 1,
                    name_w,
                    command_x,
                    command_w,
                    user: (user_w > 0).then_some((user_x, user_w)),
                    mem: (mem_w > 0).then_some((mem_x, mem_w)),
                    cpu_x: right_base,
                    cpu_w,
                };
            }
            if user_w > 0 {
                user_w = 0;
            } else if mem_w > 0 {
                mem_w = 0;
            } else if name_w > 6 {
                name_w = name_w.saturating_sub(2);
            } else {
                return Self {
                    pid_x: 0,
                    pid_w,
                    name_x: pid_w + 1,
                    name_w,
                    command_x,
                    command_w: command_w.max(1),
                    user: None,
                    mem: None,
                    cpu_x: right_base,
                    cpu_w,
                };
            }
        }
    }
}

pub fn draw(canvas: &mut Canvas, app: &App, rect: Rect, layout: &mut Layout) {
    let theme = &app.theme;
    let Some(snapshot) = app.snapshot.as_ref() else {
        let inner = super::panel(canvas, app, rect, "proc", false, None);
        super::placeholder(canvas, app, inner, "collecting process data…");
        return;
    };
    let visible = app.proc_rows.len();
    let total = snapshot.process_summary.total;
    let mut suffix = format!(
        "{visible}/{total}  {} {}{}",
        app.sort.label(),
        crate::app::sort_arrow(app.sort, app.reverse),
        if app.tree && app.filter.trim().is_empty() {
            "  tree"
        } else {
            ""
        }
    );
    if !app.filter.is_empty() {
        suffix.push_str("  filter");
    }
    let inner = super::panel(
        canvas,
        app,
        rect,
        "proc",
        false,
        Some((&suffix, Style::fg(theme.title).bold())),
    );
    if inner.is_empty() {
        return;
    }

    let needs_scrollbar = app.proc_rows.len() > usize::from(inner.h.saturating_sub(1));
    let table_w = inner.w.saturating_sub(u16::from(needs_scrollbar));
    let columns = Columns::new(table_w);
    let header = Rect::new(inner.x, inner.y, table_w, 1);
    draw_header_row(canvas, app, header, &columns);
    let body = Rect::new(inner.x, inner.y + 1, inner.w, inner.h.saturating_sub(1));
    layout.proc_body = body;
    layout.proc_visible = usize::from(body.h);
    if body.is_empty() {
        return;
    }

    if app.proc_rows.is_empty() {
        let message = if app.snapshot.is_none() {
            "collecting process data…"
        } else if !app.filter.is_empty() {
            "no processes match the filter"
        } else {
            "no processes available"
        };
        super::placeholder(canvas, app, body, message);
        return;
    }

    let visible_rows = usize::from(body.h);
    let max_scroll = app.proc_rows.len().saturating_sub(visible_rows);
    let scroll = app.proc_scroll.min(max_scroll);
    for (offset, row) in app
        .proc_rows
        .iter()
        .skip(scroll)
        .take(visible_rows)
        .enumerate()
    {
        let y = body.y + offset as u16;
        draw_row(
            canvas,
            app,
            Rect::new(inner.x, y, table_w, 1),
            row,
            &columns,
            y,
        );
    }
    if needs_scrollbar {
        crate::ui::widgets::scrollbar(
            canvas,
            Rect::new(inner.right() - 1, body.y, 1, body.h),
            scroll,
            visible_rows,
            app.proc_rows.len(),
            theme,
            app.graph_symbol() == crate::config::GraphSymbol::Tty,
        );
    }
}

fn draw_header_row(canvas: &mut Canvas, app: &App, rect: Rect, columns: &Columns) {
    let theme = &app.theme;
    let header_style = |active: bool| {
        if active {
            Style::fg(theme.accent).bold()
        } else {
            Style::fg(theme.dim).dim()
        }
    };
    let arrow = crate::app::sort_arrow(app.sort, app.reverse);
    let pid_text = if app.sort == crate::config::ProcSort::Pid {
        format!("PID{arrow}")
    } else {
        "PID".to_owned()
    };
    canvas.text_aligned(
        field(rect, columns.pid_x, columns.pid_w),
        rect.y,
        &pid_text,
        header_style(app.sort == crate::config::ProcSort::Pid),
        Align::Right,
    );
    let name_text = format!(
        "PROGRAM{}",
        if app.sort == crate::config::ProcSort::Name {
            arrow
        } else {
            ""
        }
    );
    canvas.text_truncated(
        field(rect, columns.name_x, columns.name_w),
        rect.x + columns.name_x,
        rect.y,
        &name_text,
        header_style(app.sort == crate::config::ProcSort::Name),
        columns.name_w,
    );
    let command_text = "COMMAND";
    canvas.text_truncated(
        field(rect, columns.command_x, columns.command_w),
        rect.x + columns.command_x,
        rect.y,
        command_text,
        header_style(false),
        columns.command_w,
    );
    if let Some((x, w)) = columns.user {
        let text = format!(
            "USER{}",
            if app.sort == crate::config::ProcSort::User {
                arrow
            } else {
                ""
            }
        );
        canvas.text_truncated(
            field(rect, x, w),
            rect.x + x,
            rect.y,
            &text,
            header_style(app.sort == crate::config::ProcSort::User),
            w,
        );
    }
    if let Some((x, w)) = columns.mem {
        let text = format!(
            "MEM{}",
            if app.sort == crate::config::ProcSort::Memory {
                arrow
            } else {
                ""
            }
        );
        canvas.text_aligned(
            field(rect, x, w),
            rect.y,
            &text,
            header_style(app.sort == crate::config::ProcSort::Memory),
            Align::Right,
        );
    }
    let cpu_text = format!(
        "CPU%{}",
        if app.sort == crate::config::ProcSort::Cpu {
            arrow
        } else {
            ""
        }
    );
    canvas.text_aligned(
        field(rect, columns.cpu_x, columns.cpu_w),
        rect.y,
        &cpu_text,
        header_style(app.sort == crate::config::ProcSort::Cpu),
        Align::Right,
    );
}

fn draw_row(canvas: &mut Canvas, app: &App, rect: Rect, row: &ProcRow, columns: &Columns, y: u16) {
    let theme = &app.theme;
    let Some(process) = app.snapshot.as_ref().and_then(|snapshot| {
        snapshot
            .processes
            .iter()
            .find(|process| process.pid == row.pid)
    }) else {
        return;
    };
    let selected = app.selected_pid == Some(row.pid);
    let selection_style = Style {
        fg: theme.highlight_fg.into(),
        bg: theme.highlight_bg.into(),
        ..Style::plain()
    };
    if selected {
        canvas.fill(rect, selection_style);
    }
    let base = if selected {
        Style::fg(theme.highlight_fg)
    } else {
        Style::fg(theme.foreground)
    };
    let dim = if selected {
        Style::fg(theme.highlight_fg)
    } else {
        Style::fg(theme.dim)
    };
    let pid_style = if selected { base } else { dim };
    let state_style = Style::fg(if selected {
        theme.highlight_fg
    } else {
        super::state_color(theme, process.state)
    })
    .bold();

    // PID plus a coloured state letter.
    let pid_text = process.pid.to_string();
    let state_text = process.state.short().to_string();
    let pid_width = columns.pid_w.saturating_sub(2);
    canvas.text(
        field(rect, columns.pid_x, columns.pid_w),
        rect.x + columns.pid_x,
        y,
        &state_text,
        state_style,
    );
    canvas.text_aligned(
        field(rect, columns.pid_x + 2, pid_width),
        y,
        &pid_text,
        pid_style,
        Align::Right,
    );

    // Program with tree connector.
    let name_rect = field(rect, columns.name_x, columns.name_w);
    let mut prefix_x = name_rect.x;
    let mut prefix_width = 0u16;
    if row.depth > 0 {
        for last in row
            .ancestors_last
            .iter()
            .take(usize::from(row.depth.saturating_sub(1)))
        {
            if prefix_width + 2 > columns.name_w {
                break;
            }
            let text = if *last { "  " } else { "│ " };
            canvas.text(name_rect, prefix_x, y, text, dim);
            prefix_x = prefix_x.saturating_add(2);
            prefix_width = prefix_width.saturating_add(2);
        }
        if prefix_width + 2 <= columns.name_w {
            let branch = if row.is_last { "└─" } else { "├─" };
            canvas.text(name_rect, prefix_x, y, branch, dim);
            prefix_x = prefix_x.saturating_add(2);
            prefix_width = prefix_width.saturating_add(2);
        }
    }
    if row.has_children && prefix_width + 2 <= columns.name_w {
        let marker = if row.collapsed { "▸ " } else { "▾ " };
        canvas.text(
            name_rect,
            prefix_x,
            y,
            marker,
            Style::fg(theme.accent).bold(),
        );
        prefix_x = prefix_x.saturating_add(2);
        prefix_width = prefix_width.saturating_add(2);
    }
    let name_width = columns.name_w.saturating_sub(prefix_width);
    if name_width > 0 {
        canvas.text_truncated(
            name_rect,
            prefix_x,
            y,
            &process.name,
            base.bold(),
            name_width,
        );
    }

    // Command.
    let command = normalize_command(&process.name, &process.command);
    canvas.text_truncated(
        field(rect, columns.command_x, columns.command_w),
        rect.x + columns.command_x,
        y,
        &command,
        dim,
        columns.command_w,
    );

    if let Some((x, w)) = columns.user {
        canvas.text_truncated(field(rect, x, w), rect.x + x, y, &process.user, dim, w);
    }
    if let Some((x, w)) = columns.mem {
        canvas.text_aligned(
            field(rect, x, w),
            y,
            &format::bytes_short(process.memory),
            dim,
            Align::Right,
        );
    }

    // CPU bar and numeric value.
    let cpu_rect = field(rect, columns.cpu_x, columns.cpu_w);
    let percent_text = if process.cpu < 10.0 {
        format!("{:.1}", process.cpu)
    } else {
        format!("{:.0}", process.cpu.min(99_999.0))
    };
    let numeric_w = 4u16.min(columns.cpu_w);
    let bar_w = columns.cpu_w.saturating_sub(numeric_w + 1);
    if bar_w >= 2 {
        crate::ui::widgets::bar(
            canvas,
            Rect::new(cpu_rect.x, y, bar_w, 1),
            process.cpu / 100.0,
            theme.cpu,
            theme,
            app.graph_symbol() == crate::config::GraphSymbol::Tty,
        );
    }
    canvas.text_aligned(
        field(rect, columns.cpu_x + columns.cpu_w - numeric_w, numeric_w),
        y,
        &percent_text,
        Style::fg(if selected {
            theme.highlight_fg
        } else {
            theme.foreground
        }),
        Align::Right,
    );
}

fn field(rect: Rect, x: u16, width: u16) -> Rect {
    Rect::new(
        rect.x.saturating_add(x),
        rect.y,
        width.min(rect.w.saturating_sub(x)),
        1,
    )
}

fn normalize_command(name: &str, command: &str) -> String {
    let command = format::sanitize(command);
    if command.trim().is_empty() || command.trim() == name {
        name.to_owned()
    } else {
        command
    }
}
