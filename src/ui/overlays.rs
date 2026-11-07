//! Modal overlays: help, process details, and signal confirmation.

use crate::app::{App, Overlay};
use crate::format;
use crate::platform::{ProcessState, SignalChoice};
use crate::theme::Style;
use crate::ui::canvas::{Canvas, Rect};

pub fn draw(canvas: &mut Canvas, app: &App, screen: Rect) {
    match &app.overlay {
        Overlay::None => {}
        Overlay::Help { scroll } => draw_help(canvas, app, screen, *scroll),
        Overlay::ProcessInfo { pid, scroll } => {
            draw_process_info(canvas, app, screen, *pid, *scroll);
        }
        Overlay::ConfirmKill {
            pid, name, signal, ..
        } => draw_kill(canvas, app, screen, *pid, name, *signal),
    }
}

fn dialog(
    canvas: &mut Canvas,
    app: &App,
    screen: Rect,
    width: u16,
    height: u16,
    title: &str,
) -> Option<Rect> {
    let theme = &app.theme;
    if screen.w < 12 || screen.h < 6 {
        return None;
    }
    let width = width.min(screen.w.saturating_sub(2)).max(10);
    let height = height.min(screen.h.saturating_sub(2)).max(4);
    let rect = screen.center(width, height);
    let background = Style {
        fg: theme.foreground.into(),
        bg: theme.background.into(),
        ..Style::plain()
    };
    canvas.fill(rect, background);
    let rounded =
        app.config.rounded_corners && app.graph_symbol() != crate::config::GraphSymbol::Tty;
    let inner = canvas.draw_box(
        rect,
        rounded,
        Style::fg(theme.title),
        Some((title, Style::fg(theme.title).bold())),
    );
    Some(inner)
}

fn draw_help(canvas: &mut Canvas, app: &App, screen: Rect, scroll: u16) {
    let theme = &app.theme;
    let width = (screen.w * 2 / 3)
        .clamp(56, 78)
        .min(screen.w.saturating_sub(2));
    let height = screen.h.saturating_sub(2).clamp(8, 32);
    let Some(inner) = dialog(canvas, app, screen, width, height, "help") else {
        return;
    };
    if inner.is_empty() {
        return;
    }
    let sections: [(&str, &[(&str, &str)]); 6] = [
        (
            "navigation",
            &[
                ("↑ / ↓  j", "move selection"),
                ("PgUp / PgDn", "move one page"),
                ("Home / End  g/G", "first / last process"),
                ("Tab / 0-4", "switch view"),
            ],
        ),
        (
            "views",
            &[
                ("F2", "cycle theme"),
                ("+ / -", "faster / slower refresh"),
                ("s", "pause and resume"),
                ("x", "clear the filter"),
            ],
        ),
        (
            "processes",
            &[
                ("Enter / i", "process details"),
                ("f", "filter"),
                ("c / m / p / n / u", "sort by cpu / mem / pid / name / user"),
                ("r", "reverse sort order"),
                ("t", "tree mode"),
                ("e / w / Space", "expand / collapse children"),
                ("k", "send a signal"),
            ],
        ),
        (
            "network",
            &[
                ("[ / ]", "previous / next interface"),
                ("a", "automatic interface selection"),
            ],
        ),
        (
            "filter",
            &[
                ("Enter", "apply"),
                ("Esc", "cancel"),
                ("Ctrl+U", "clear input"),
            ],
        ),
        (
            "other",
            &[
                ("? / F1", "toggle this help"),
                ("q / Ctrl+C", "quit"),
                ("Esc", "close overlay / return to overview"),
            ],
        ),
    ];

    let content = Rect::new(inner.x, inner.y, inner.w, inner.h.saturating_sub(1));
    let mut lines: Vec<HelpLine> = Vec::new();
    for (index, (section, entries)) in sections.iter().enumerate() {
        if index > 0 {
            lines.push(HelpLine::Blank);
        }
        lines.push(HelpLine::Section(section));
        for (key, description) in *entries {
            lines.push(HelpLine::Entry(key, description));
        }
    }
    let max_scroll = lines.len().saturating_sub(usize::from(content.h));
    let scroll = usize::from(scroll).min(max_scroll);
    for (offset, line) in lines
        .iter()
        .skip(scroll)
        .take(usize::from(content.h))
        .enumerate()
    {
        let y = content.y + offset as u16;
        match line {
            HelpLine::Section(section) => {
                canvas.text(
                    content,
                    content.x,
                    y,
                    &section.to_uppercase(),
                    Style::fg(theme.accent).bold(),
                );
            }
            HelpLine::Entry(key, description) => {
                let key_width = 20u16.min(content.w / 3);
                canvas.text_truncated(
                    content,
                    content.x,
                    y,
                    key,
                    Style::fg(theme.accent).bold(),
                    key_width,
                );
                canvas.text_truncated(
                    content,
                    content.x + key_width + 1,
                    y,
                    description,
                    Style::fg(theme.foreground),
                    content.w.saturating_sub(key_width + 1),
                );
            }
            HelpLine::Blank => {}
        }
    }
    draw_modal_hint(canvas, app, inner, &[("↑↓", "scroll"), ("Esc", "close")]);
}

enum HelpLine {
    Section(&'static str),
    Entry(&'static str, &'static str),
    Blank,
}

enum InfoLine {
    Section(&'static str),
    KeyValue(&'static str, String),
    Text(String),
    Blank,
}

fn draw_process_info(canvas: &mut Canvas, app: &App, screen: Rect, pid: u32, scroll: u16) {
    let theme = &app.theme;
    let width = 86.min(screen.w.saturating_sub(2)).max(30);
    let height = 30.min(screen.h.saturating_sub(2)).max(8);
    let process = app
        .snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.processes.iter().find(|process| process.pid == pid));
    let title = format!("process {pid}");
    let Some(inner) = dialog(canvas, app, screen, width, height, &title) else {
        return;
    };
    if inner.is_empty() {
        return;
    }
    let Some(process) = process else {
        super::placeholder(canvas, app, inner, "the process has exited");
        draw_modal_hint(canvas, app, inner, &[("Esc", "close")]);
        return;
    };

    let mut lines = vec![
        InfoLine::Section("identity"),
        InfoLine::KeyValue(
            "pid / parent",
            format!(
                "{} / {}",
                process.pid,
                process
                    .parent
                    .map_or_else(|| "-".to_owned(), |parent| parent.to_string())
            ),
        ),
        InfoLine::KeyValue("user", format::sanitize(&process.user)),
        InfoLine::KeyValue(
            "state",
            format!("{} ({})", state_label(process.state), process.state.short()),
        ),
        InfoLine::KeyValue(
            "threads/session",
            format!(
                "{} / {}",
                process
                    .threads
                    .map_or_else(|| "-".to_owned(), |threads| threads.to_string()),
                process
                    .session
                    .map_or_else(|| "-".to_owned(), |session| session.to_string())
            ),
        ),
        InfoLine::KeyValue("running", format::duration_short(process.run_time)),
        InfoLine::Blank,
        InfoLine::Section("resources"),
        InfoLine::KeyValue("cpu", format!("{:.1}%", process.cpu)),
        InfoLine::KeyValue("memory", format::bytes(process.memory)),
        InfoLine::KeyValue("virtual", format::bytes(process.virtual_memory)),
        InfoLine::KeyValue(
            "disk i/o",
            format!(
                "read {} · written {}",
                format::bytes(process.read_bytes),
                format::bytes(process.written_bytes)
            ),
        ),
        InfoLine::Blank,
        InfoLine::Section("command"),
    ];
    let content_width = usize::from(inner.w.saturating_sub(2));
    for line in crate::ui::widgets::wrap_text(&process.command, content_width) {
        lines.push(InfoLine::Text(line));
    }
    if let Some(exe) = &process.exe {
        lines.push(InfoLine::Blank);
        lines.push(InfoLine::Section("executable"));
        for line in crate::ui::widgets::wrap_text(exe, content_width) {
            lines.push(InfoLine::Text(line));
        }
    }
    if let Some(cwd) = &process.cwd {
        lines.push(InfoLine::Blank);
        lines.push(InfoLine::Section("working directory"));
        for line in crate::ui::widgets::wrap_text(cwd, content_width) {
            lines.push(InfoLine::Text(line));
        }
    }

    let content = Rect::new(inner.x, inner.y, inner.w, inner.h.saturating_sub(1));
    let max_scroll = lines.len().saturating_sub(usize::from(content.h));
    let scroll = usize::from(scroll).min(max_scroll);
    for (offset, line) in lines
        .iter()
        .skip(scroll)
        .take(usize::from(content.h))
        .enumerate()
    {
        let y = content.y + offset as u16;
        match line {
            InfoLine::Section(label) => {
                canvas.text(
                    content,
                    content.x,
                    y,
                    &label.to_uppercase(),
                    Style::fg(theme.accent).bold(),
                );
            }
            InfoLine::KeyValue(key, value) => {
                let key_width = 16u16.min(content.w / 3);
                canvas.text_truncated(content, content.x, y, key, Style::fg(theme.dim), key_width);
                canvas.text_truncated(
                    content,
                    content.x + key_width + 1,
                    y,
                    value,
                    Style::fg(theme.foreground),
                    content.w.saturating_sub(key_width + 1),
                );
            }
            InfoLine::Text(text) => {
                canvas.text_truncated(
                    content,
                    content.x,
                    y,
                    text,
                    Style::fg(theme.foreground),
                    content.w,
                );
            }
            InfoLine::Blank => {}
        }
    }
    draw_modal_hint(canvas, app, inner, &[("↑↓", "scroll"), ("Esc", "close")]);
}

fn draw_kill(
    canvas: &mut Canvas,
    app: &App,
    screen: Rect,
    pid: u32,
    name: &str,
    signal_index: usize,
) {
    let theme = &app.theme;
    let width = 64.min(screen.w.saturating_sub(2)).max(30);
    let Some(inner) = dialog(canvas, app, screen, width, 10, "send signal") else {
        return;
    };
    if inner.is_empty() {
        return;
    }
    let choices = crate::platform::signal_choices();
    let selected = choices.get(signal_index).copied().unwrap_or(choices[0]);

    let process_line = format!("{}  {}", pid, format::sanitize(name));
    canvas.text_truncated(
        inner,
        inner.x,
        inner.y,
        &process_line,
        Style::fg(theme.foreground).bold(),
        inner.w,
    );
    canvas.text_truncated(
        inner,
        inner.x,
        inner.y + 1,
        "choose a signal:",
        Style::fg(theme.dim),
        inner.w,
    );

    let mut x = inner.x;
    let y = inner.y + 3;
    for (index, choice) in choices.iter().enumerate() {
        let label = format!(" {} ", choice.label());
        let label_width = u16::try_from(format::width(&label)).unwrap_or(0);
        if x + label_width > inner.right() {
            break;
        }
        let style = if index == signal_index {
            Style {
                fg: theme.background.into(),
                bg: theme.warning.into(),
                bold: true,
                ..Style::plain()
            }
        } else {
            Style::fg(theme.dim)
        };
        canvas.fill(Rect::new(x, y, label_width, 1), style);
        canvas.text(inner, x, y, &label, style);
        x = x.saturating_add(label_width + 2);
    }
    canvas.text_truncated(
        inner,
        inner.x,
        inner.y + 5,
        &format!("{} · {}", selected.label(), selected.description()),
        Style::fg(theme.foreground),
        inner.w,
    );
    let warning = if selected == SignalChoice::Kill {
        "This forcibly stops the process and cannot be undone."
    } else {
        "The process may exit or save its state before stopping."
    };
    canvas.text_truncated(
        inner,
        inner.x,
        inner.y + 6,
        warning,
        Style::fg(if selected == SignalChoice::Kill {
            theme.critical
        } else {
            theme.warning
        }),
        inner.w,
    );
    draw_modal_hint(
        canvas,
        app,
        inner,
        &[("←→", "choose"), ("Enter", "confirm"), ("Esc", "cancel")],
    );
}

fn draw_modal_hint(canvas: &mut Canvas, app: &App, inner: Rect, hints: &[(&str, &str)]) {
    if inner.h == 0 {
        return;
    }
    let y = inner.bottom() - 1;
    let theme = &app.theme;
    let width = hints.iter().fold(0u16, |sum, (key, label)| {
        sum.saturating_add(
            u16::try_from(format::width(key) + 1 + format::width(label) + 2).unwrap_or(0),
        )
    });
    let start = inner.x + inner.w.saturating_sub(width) / 2;
    crate::ui::widgets::key_hints(
        canvas,
        Rect::new(start, y, width.min(inner.w), 1),
        hints,
        theme,
        "  ",
    );
}

fn state_label(state: ProcessState) -> &'static str {
    state.label()
}
