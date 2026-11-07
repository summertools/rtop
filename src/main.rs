//! `rtop` binary entry point.

use std::io::{self, IsTerminal, Write};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal;

use rtop::app::{App, ToastLevel};
use rtop::cli::Cli;
use rtop::config::Config;
use rtop::format;
use rtop::platform::{self, Collector, Snapshot};
use rtop::term::{self, ColorMode, Screen, TerminalGuard};
use rtop::{Config as PublicConfig, ui};

fn main() {
    if let Err(error) = run() {
        if error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
        {
            return;
        }
        eprintln!("rtop: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    if cli.list_themes {
        list_themes()?;
        return Ok(());
    }

    let (mut config, mut startup_warnings) = Config::load(cli.config.as_deref());
    startup_warnings.extend(cli.apply(&mut config));
    let (theme, theme_warnings) = config.load_theme();
    startup_warnings.extend(theme_warnings);

    if cli.dump {
        return dump(config, theme, &cli, startup_warnings);
    }

    require_terminal()?;
    let mode = ColorMode::detect(config.truecolor, config.tty_mode, cli.no_color);
    if mode == ColorMode::None {
        // A full theme background would become solid reverse video without
        // colour; leave the terminal background alone in that case.
        config.theme_background = false;
    }
    let background = Some(theme.background);

    let update_interval = Arc::new(AtomicU64::new(config.update_ms.clamp(250, 10_000)));
    let stop = Arc::new(AtomicBool::new(false));
    let terminate = Arc::new(AtomicBool::new(false));
    register_termination_signals(&terminate);
    let (sender, receiver) = mpsc::sync_channel::<Snapshot>(2);
    let collector_handle = spawn_collector(sender, Arc::clone(&update_interval), Arc::clone(&stop));

    let mut app = App::new(
        config,
        theme,
        startup_warnings.clone(),
        Arc::clone(&update_interval),
    );

    // Restore the terminal even on a panic. The previous hook still prints the
    // panic message afterwards, now on the normal screen.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        term::restore_terminal_best_effort();
        previous_hook(info);
    }));

    let guard = TerminalGuard::enter({
        // `config.mouse` was moved into the app; read it from the app instead.
        app.config.mouse
    })?;
    let mut screen = Screen::new(mode, background);
    let mut needs_draw = true;
    let mut collector_dead = false;

    let result: Result<(), Box<dyn std::error::Error>> = loop {
        loop {
            match receiver.try_recv() {
                Ok(snapshot) => app.apply_snapshot(snapshot),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !collector_dead {
                        collector_dead = true;
                        app.set_toast(
                            "the system collector stopped; data is now stale",
                            ToastLevel::Error,
                        );
                    }
                    break;
                }
            }
        }
        if app.quit || terminate.load(Ordering::Relaxed) {
            break Ok(());
        }
        app.tick(Instant::now());
        if app.take_dirty() {
            needs_draw = true;
        }
        if needs_draw {
            let (width, height) = terminal::size()?;
            let (canvas, layout) = ui::render(&app, width, height);
            app.set_layout(layout);
            screen.draw(&canvas)?;
            needs_draw = false;
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    app.handle_key(key);
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                Event::Paste(text) => app.handle_paste(&text),
                Event::Resize(_, _) => needs_draw = true,
                _ => {}
            }
        }
    };

    drop(screen);
    drop(guard);
    stop.store(true, Ordering::SeqCst);
    drop(receiver);
    // Do not join: a platform query (WMI/NSS/IOKit) can be slow or stuck, and
    // it must never delay quitting after the terminal has been restored.
    drop(collector_handle);
    for warning in &startup_warnings {
        eprintln!("rtop: warning: {}", format::sanitize_limited(warning, 500));
    }
    result?;
    Ok(())
}

fn register_termination_signals(flag: &Arc<AtomicBool>) {
    use signal_hook::{consts, flag as signal_flag};
    for signal in [consts::SIGTERM, consts::SIGINT] {
        let _ = signal_flag::register(signal, Arc::clone(flag));
    }
    #[cfg(unix)]
    for signal in [consts::SIGHUP, consts::SIGQUIT] {
        let _ = signal_flag::register(signal, Arc::clone(flag));
    }
}

fn require_terminal() -> io::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "interactive mode needs a terminal on stdin and stdout; try --dump for a plain frame",
        ));
    }
    if std::env::var("TERM").is_ok_and(|term| term.eq_ignore_ascii_case("dumb")) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "TERM=dumb is not supported; use a UTF-8 capable terminal or --dump",
        ));
    }
    Ok(())
}

fn spawn_collector(
    sender: mpsc::SyncSender<Snapshot>,
    update: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let Ok(mut collector) = std::panic::catch_unwind(AssertUnwindSafe(Collector::new)) else {
            let _ = sender.send(platform::unavailable_snapshot(
                "the system collector failed to start",
            ));
            return;
        };
        // Establish CPU and process baselines, then wait a moment so the first
        // visible snapshot already contains meaningful percentages.
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| collector.sample()));
        if wait_interruptible(Duration::from_millis(250), &stop) {
            return;
        }
        let mut last: Option<Snapshot> = None;
        loop {
            let sampled = std::panic::catch_unwind(AssertUnwindSafe(|| collector.sample()));
            if let Ok(snapshot) = sampled {
                last = Some(snapshot.clone());
                if sender.send(snapshot).is_err() {
                    break;
                }
            } else {
                let mut snapshot = last
                    .clone()
                    .unwrap_or_else(|| platform::unavailable_snapshot("collector error"));
                snapshot.warnings =
                    vec!["the collector recovered from an internal error; retrying".to_owned()];
                if sender.send(snapshot).is_err() {
                    break;
                }
                if let Ok(fresh) = std::panic::catch_unwind(AssertUnwindSafe(Collector::new)) {
                    collector = fresh;
                }
                if wait_interruptible(Duration::from_secs(2), &stop) {
                    break;
                }
                continue;
            }
            if wait_refresh(&update, &stop) {
                break;
            }
        }
    })
}

/// Wait until the current refresh interval elapses. The interval is re-read on
/// every slice so `+`/`-` takes effect during an existing wait.
fn wait_refresh(update: &AtomicU64, stop: &AtomicBool) -> bool {
    let start = Instant::now();
    loop {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        let interval = Duration::from_millis(update.load(Ordering::Relaxed).clamp(250, 10_000));
        let elapsed = start.elapsed();
        if elapsed >= interval {
            return false;
        }
        thread::sleep(
            interval
                .saturating_sub(elapsed)
                .min(Duration::from_millis(100)),
        );
    }
}

/// Sleep in small slices so quitting never waits for a long refresh interval.
fn wait_interruptible(duration: Duration, stop: &AtomicBool) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(remaining.min(Duration::from_millis(100)));
    }
    stop.load(Ordering::Relaxed)
}

fn dump(
    config: PublicConfig,
    theme: rtop::theme::Theme,
    cli: &Cli,
    warnings: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    for warning in &warnings {
        eprintln!("rtop: warning: {}", format::sanitize_limited(warning, 500));
    }
    let mut collector = Collector::new();
    // Two samples make CPU and per-process percentages meaningful.
    let _ = collector.sample();
    thread::sleep(Duration::from_millis(300));
    let snapshot = collector.sample();
    let update = Arc::new(AtomicU64::new(config.update_ms.clamp(250, 10_000)));
    let mut app = App::new(config, theme, warnings, update);
    app.apply_snapshot(snapshot);
    let width = cli.width.clamp(20, 400);
    let height = cli.height.clamp(8, 200);
    let (canvas, _) = ui::render(&app, width, height);
    let mut stdout = io::stdout().lock();
    match writeln!(stdout, "{}", canvas.plain()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(Box::new(error)),
    }
}

fn list_themes() -> io::Result<()> {
    let mut out = io::stdout().lock();
    writeln!(out, "Built-in themes:")?;
    for name in rtop::theme::Theme::builtin_names() {
        writeln!(out, "  {name}")?;
    }
    if let Some(dir) = Config::theme_dir() {
        writeln!(out, "\nUser themes in {}:", dir.display())?;
        let mut found = false;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut names: Vec<String> = entries
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "theme") {
                        path.file_stem()
                            .map(|stem| stem.to_string_lossy().into_owned())
                            .filter(|name| rtop::config::valid_theme_name(name))
                    } else {
                        None
                    }
                })
                .collect();
            names.sort();
            for name in names {
                writeln!(out, "  {name}")?;
                found = true;
            }
        }
        if !found {
            writeln!(out, "  (none)")?;
        }
    }
    out.flush()
}
