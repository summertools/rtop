//! Application state, navigation, filtering, and actions.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::config::{Config, GraphSymbol, ProcSort};
use crate::format;
use crate::platform::{self, GpuSnapshot, NetworkSnapshot, ProcessInfo, Snapshot};
use crate::theme::Theme;
use crate::ui::canvas::Rect;
use crate::ui::graph::GraphData;

/// Top-level screens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preset {
    #[default]
    Overview,
    Cpu,
    Memory,
    Network,
    Processes,
    Gpu,
}

impl Preset {
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Cpu,
            2 => Self::Memory,
            3 => Self::Network,
            4 => Self::Processes,
            5 => Self::Gpu,
            _ => Self::Overview,
        }
    }

    #[must_use]
    pub const fn index(self) -> u8 {
        match self {
            Self::Overview => 0,
            Self::Cpu => 1,
            Self::Memory => 2,
            Self::Network => 3,
            Self::Processes => 4,
            Self::Gpu => 5,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Overview => "all",
            Self::Cpu => "cpu",
            Self::Memory => "mem",
            Self::Network => "net",
            Self::Processes => "proc",
            Self::Gpu => "gpu",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Overview => Self::Cpu,
            Self::Cpu => Self::Memory,
            Self::Memory => Self::Network,
            Self::Network => Self::Processes,
            Self::Processes => Self::Gpu,
            Self::Gpu => Self::Overview,
        }
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::Overview => Self::Gpu,
            Self::Cpu => Self::Overview,
            Self::Memory => Self::Cpu,
            Self::Network => Self::Memory,
            Self::Processes => Self::Network,
            Self::Gpu => Self::Processes,
        }
    }

    #[must_use]
    pub const fn all() -> [Self; 6] {
        [
            Self::Overview,
            Self::Cpu,
            Self::Memory,
            Self::Network,
            Self::Processes,
            Self::Gpu,
        ]
    }
}

/// A flattened row in the process table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcRow {
    pub pid: u32,
    pub depth: u16,
    pub is_last: bool,
    /// Whether each ancestor at depth 1..depth was the last sibling.
    pub ancestors_last: Vec<bool>,
    pub has_children: bool,
    pub collapsed: bool,
}

/// Cached layout rectangles for mouse hit-testing.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub screen: Rect,
    pub tabs: Vec<(Preset, Rect)>,
    pub proc_body: Rect,
    pub proc_visible: usize,
    pub net_ifaces: Vec<(String, Rect)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub text: String,
    pub level: ToastLevel,
    pub expires: Instant,
}

#[derive(Clone, Debug)]
pub struct NetHistory {
    pub rx: GraphData,
    pub tx: GraphData,
}

impl Default for NetHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl NetHistory {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rx: GraphData::new(512),
            tx: GraphData::new(512),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Overlay {
    #[default]
    None,
    Help {
        scroll: u16,
    },
    ProcessInfo {
        pid: u32,
        scroll: u16,
    },
    ConfirmKill {
        pid: u32,
        name: String,
        start_time: u64,
        signal: usize,
    },
}

impl Overlay {
    #[must_use]
    pub fn is_open(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// The complete interactive application state.
#[derive(Debug, Clone)]
pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub preset: Preset,
    pub snapshot: Option<Snapshot>,
    pub warnings: Vec<String>,
    pub cpu_history: GraphData,
    pub core_history: Vec<GraphData>,
    pub gpu_history: HashMap<String, GraphData>,
    pub memory_history: GraphData,
    pub net_history: HashMap<String, NetHistory>,
    pub selected_pid: Option<u32>,
    pub proc_scroll: usize,
    pub proc_rows: Vec<ProcRow>,
    pub sort: ProcSort,
    pub reverse: bool,
    pub tree: bool,
    pub collapsed: HashSet<u32>,
    pub filter: String,
    pub filter_edit: Option<String>,
    pub net_auto: bool,
    pub net_iface: Option<String>,
    pub paused: bool,
    pub overlay: Overlay,
    pub toast: Option<Toast>,
    pub quit: bool,
    pub layout: Layout,
    pub interval: u64,
    pub read_only: bool,
    clock: String,
    dirty: bool,
    selection_pinned: bool,
    update_handle: Arc<AtomicU64>,
    last_click: Option<(Instant, u32)>,
}

impl App {
    #[must_use]
    pub fn new(
        config: Config,
        theme: Theme,
        warnings: Vec<String>,
        update_handle: Arc<AtomicU64>,
    ) -> Self {
        let interval = config.update_ms.clamp(250, 10_000);
        update_handle.store(interval, Ordering::Relaxed);
        let mut preset = Preset::from_u8(config.preset);
        if preset == Preset::Gpu && !config.show_gpu {
            preset = Preset::Overview;
        }
        let mut app = Self {
            preset,
            sort: config.proc_sort,
            reverse: config.proc_reversed,
            tree: config.proc_tree,
            net_auto: config.net_auto,
            net_iface: if config.net_auto || config.net_iface.is_empty() {
                None
            } else {
                Some(config.net_iface.clone())
            },
            read_only: config.read_only,
            interval,
            config,
            theme,
            snapshot: None,
            warnings,
            cpu_history: GraphData::new(512),
            core_history: Vec::new(),
            gpu_history: HashMap::new(),
            memory_history: GraphData::new(512),
            net_history: HashMap::new(),
            selected_pid: None,
            proc_scroll: 0,
            proc_rows: Vec::new(),
            collapsed: HashSet::new(),
            filter: String::new(),
            filter_edit: None,
            paused: false,
            overlay: Overlay::None,
            toast: None,
            quit: false,
            layout: Layout::default(),
            clock: platform::local_clock(),
            dirty: true,
            selection_pinned: false,
            update_handle,
            last_click: None,
        };
        if let Some(first) = app.warnings.first().cloned() {
            let extra = app.warnings.len().saturating_sub(1);
            let text = if extra > 0 {
                format!("{first} (+{extra} more)")
            } else {
                first
            };
            app.set_toast(text, ToastLevel::Warning);
        }
        app
    }

    #[must_use]
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// Effective graph character set. TTY mode always forces the ASCII-safe
    /// set, even if a file or `--graph-symbol` asked for braille.
    #[must_use]
    pub fn graph_symbol(&self) -> GraphSymbol {
        if self.config.tty_mode {
            GraphSymbol::Tty
        } else {
            self.config.graph_symbol
        }
    }

    /// Whether the UI should restrict itself to ASCII glyphs.
    #[must_use]
    pub fn ascii(&self) -> bool {
        self.config.tty_mode || self.config.graph_symbol == GraphSymbol::Tty
    }

    pub fn set_layout(&mut self, layout: Layout) {
        let visible = layout.proc_visible.max(1);
        let max_scroll = self.proc_rows.len().saturating_sub(visible);
        self.proc_scroll = self.proc_scroll.min(max_scroll);
        self.layout = layout;
        self.ensure_selection_visible();
    }

    /// Update transient state and return whether a redraw is needed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let clock = platform::local_clock();
        if clock != self.clock {
            self.clock.clone_from(&clock);
            self.dirty = true;
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| now >= toast.expires)
        {
            self.toast = None;
            self.dirty = true;
        }
        self.dirty
    }

    #[must_use]
    pub fn clock(&self) -> &str {
        &self.clock
    }

    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        if self.paused {
            return;
        }
        self.cpu_history.push(f64::from(snapshot.cpu.overall_usage));
        if self.core_history.len() != snapshot.cpu.cores.len() {
            self.core_history
                .resize_with(snapshot.cpu.cores.len(), || GraphData::new(256));
        }
        for (history, usage) in self.core_history.iter_mut().zip(&snapshot.cpu.cores) {
            history.push(f64::from(*usage));
        }
        let gpu_keys: HashSet<String> =
            snapshot.gpus.iter().map(GpuSnapshot::history_key).collect();
        self.gpu_history.retain(|key, _| gpu_keys.contains(key));
        for gpu in &snapshot.gpus {
            self.gpu_history
                .entry(gpu.history_key())
                .or_insert_with(|| GraphData::new(256))
                .push(f64::from(gpu.utilization.unwrap_or(0.0)));
        }
        if snapshot.memory.total > 0 {
            self.memory_history
                .push(snapshot.memory.used as f64 * 100.0 / snapshot.memory.total as f64);
        }

        let names: HashSet<&str> = snapshot
            .networks
            .iter()
            .map(|net| net.name.as_str())
            .collect();
        self.net_history
            .retain(|name, _| names.contains(name.as_str()));
        for net in &snapshot.networks {
            let history = self.net_history.entry(net.name.clone()).or_default();
            history.rx.push(net.received_rate);
            history.tx.push(net.transmitted_rate);
        }
        let chosen = pick_network(&snapshot.networks, self.net_auto, self.net_iface.as_deref());
        self.net_iface = chosen;

        if !snapshot.warnings.is_empty() && snapshot.warnings != self.warnings {
            let text = if snapshot.warnings.len() > 1 {
                format!(
                    "{} (+{} more warnings)",
                    snapshot.warnings[0],
                    snapshot.warnings.len() - 1
                )
            } else {
                snapshot.warnings[0].clone()
            };
            self.set_toast(text, ToastLevel::Warning);
        }
        self.warnings.clone_from(&snapshot.warnings);
        self.snapshot = Some(snapshot);
        self.rebuild_rows();
        self.dirty = true;
    }

    /// Recompute visible process rows after a snapshot or view change.
    pub fn rebuild_rows(&mut self) {
        let alive: HashSet<u32> = if let Some(snapshot) = &self.snapshot {
            snapshot
                .processes
                .iter()
                .map(|process| process.pid)
                .collect()
        } else {
            self.proc_rows.clear();
            return;
        };
        if !self.collapsed.is_empty() {
            self.collapsed.retain(|pid| alive.contains(pid));
        }
        let snapshot = self.snapshot.as_ref().expect("snapshot checked");
        let previous_pid = self.selected_pid;
        self.proc_rows = build_rows(
            &snapshot.processes,
            self.sort,
            self.reverse,
            self.tree && self.filter.trim().is_empty(),
            &self.filter,
            &self.collapsed,
        );
        if self.proc_rows.is_empty() {
            self.selected_pid = None;
            self.proc_scroll = 0;
            return;
        }
        let keep = if self.selection_pinned {
            previous_pid.and_then(|pid| self.proc_rows.iter().position(|row| row.pid == pid))
        } else {
            None
        };
        let index = keep.unwrap_or(0);
        self.selected_pid = Some(self.proc_rows[index].pid);
        if self.selection_pinned {
            self.proc_scroll = self.proc_scroll.min(self.proc_rows.len().saturating_sub(1));
            self.ensure_selection_visible();
        } else {
            self.proc_scroll = 0;
        }
    }

    #[must_use]
    pub fn selected_row(&self) -> Option<usize> {
        let pid = self.selected_pid?;
        self.proc_rows.iter().position(|row| row.pid == pid)
    }

    #[must_use]
    pub fn selected_process(&self) -> Option<&ProcessInfo> {
        let pid = self.selected_pid?;
        self.snapshot
            .as_ref()?
            .processes
            .iter()
            .find(|process| process.pid == pid)
    }

    /// Whether the latest snapshot contains at least one supported GPU.
    #[must_use]
    pub fn has_gpu(&self) -> bool {
        self.snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.gpus.is_empty())
    }

    #[must_use]
    pub fn selected_network(&self) -> Option<&NetworkSnapshot> {
        let snapshot = self.snapshot.as_ref()?;
        let name = self.net_iface.as_deref()?;
        snapshot
            .networks
            .iter()
            .find(|network| network.name == name)
            .or_else(|| snapshot.networks.first())
    }

    #[must_use]
    pub fn network_history(&self) -> Option<&NetHistory> {
        let name = self.selected_network()?.name.as_str();
        self.net_history.get(name)
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.proc_rows.is_empty() {
            return;
        }
        let current = self.selected_row().unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.proc_rows.len() as isize - 1) as usize;
        self.selected_pid = Some(self.proc_rows[next].pid);
        self.selection_pinned = true;
        self.ensure_selection_visible();
        self.dirty = true;
    }

    pub fn selection_home(&mut self) {
        if let Some(row) = self.proc_rows.first() {
            self.selected_pid = Some(row.pid);
            self.selection_pinned = true;
            self.proc_scroll = 0;
            self.dirty = true;
        }
    }

    pub fn selection_end(&mut self) {
        if let Some(row) = self.proc_rows.last() {
            self.selected_pid = Some(row.pid);
            self.selection_pinned = true;
            self.proc_scroll = self.proc_rows.len().saturating_sub(1);
            self.ensure_selection_visible();
            self.dirty = true;
        }
    }

    pub fn page_selection(&mut self, direction: isize) {
        let page = self.layout.proc_visible.max(1) as isize;
        self.move_selection(direction * page);
    }

    fn ensure_selection_visible(&mut self) {
        let Some(index) = self.selected_row() else {
            return;
        };
        let visible = self.layout.proc_visible.max(1);
        if index < self.proc_scroll {
            self.proc_scroll = index;
        } else if index >= self.proc_scroll.saturating_add(visible) {
            self.proc_scroll = index.saturating_add(1).saturating_sub(visible);
        }
        let max_scroll = self.proc_rows.len().saturating_sub(visible);
        self.proc_scroll = self.proc_scroll.min(max_scroll);
    }

    pub fn scroll_processes(&mut self, delta: isize) {
        if self.proc_rows.is_empty() {
            return;
        }
        let visible = self.layout.proc_visible.max(1);
        let max_scroll = self.proc_rows.len().saturating_sub(visible);
        self.proc_scroll =
            (self.proc_scroll as isize + delta).clamp(0, max_scroll as isize) as usize;
        self.selection_pinned = true;
        // Keep the selected PID visible if it moved off-screen.
        if let Some(index) = self.selected_row() {
            if index < self.proc_scroll {
                self.selected_pid = Some(self.proc_rows[self.proc_scroll].pid);
            } else if index >= self.proc_scroll + visible {
                let target = (self.proc_scroll + visible - 1).min(self.proc_rows.len() - 1);
                self.selected_pid = Some(self.proc_rows[target].pid);
            }
        }
        self.dirty = true;
    }

    pub fn set_preset(&mut self, preset: Preset) {
        if preset == Preset::Gpu && !self.config.show_gpu {
            return;
        }
        self.preset = preset;
        self.dirty = true;
    }

    pub fn cycle_preset(&mut self, forward: bool) {
        let mut next = if forward {
            self.preset.next()
        } else {
            self.preset.previous()
        };
        if next == Preset::Gpu && !self.config.show_gpu {
            next = if forward {
                next.next()
            } else {
                next.previous()
            };
        }
        self.preset = next;
        self.dirty = true;
    }

    pub fn set_sort(&mut self, sort: ProcSort) {
        if self.sort == sort {
            self.reverse = !self.reverse;
        } else {
            self.sort = sort;
            self.reverse = false;
        }
        self.rebuild_rows();
        self.dirty = true;
    }

    /// Cycle the process sort field, like btop's left/right arrow keys.
    /// Picking a different field resets the direction to that field's natural
    /// default; pressing the same field's shortcut still toggles it.
    pub fn cycle_sort(&mut self, forward: bool) {
        let next = if forward {
            self.sort.cycle()
        } else {
            self.sort.previous()
        };
        if next == self.sort {
            return;
        }
        self.sort = next;
        self.reverse = false;
        self.rebuild_rows();
        self.dirty = true;
    }

    pub fn toggle_reverse(&mut self) {
        self.reverse = !self.reverse;
        self.rebuild_rows();
        self.dirty = true;
    }

    pub fn toggle_tree(&mut self) {
        self.tree = !self.tree;
        if !self.tree {
            self.collapsed.clear();
        }
        self.rebuild_rows();
        self.dirty = true;
    }

    pub fn collapse_selected(&mut self) {
        let Some(pid) = self.selected_pid else { return };
        if self
            .proc_rows
            .iter()
            .any(|row| row.pid == pid && row.has_children)
        {
            self.collapsed.insert(pid);
            self.rebuild_rows();
            self.dirty = true;
        }
    }

    pub fn expand_selected(&mut self) {
        let Some(pid) = self.selected_pid else { return };
        if self.collapsed.remove(&pid) {
            self.rebuild_rows();
            self.dirty = true;
        }
    }

    pub fn toggle_collapsed(&mut self) {
        let Some(pid) = self.selected_pid else { return };
        if !self
            .proc_rows
            .iter()
            .any(|row| row.pid == pid && row.has_children)
        {
            return;
        }
        if !self.collapsed.remove(&pid) {
            self.collapsed.insert(pid);
        }
        self.rebuild_rows();
        self.dirty = true;
    }

    pub fn begin_filter(&mut self) {
        self.filter_edit = Some(self.filter.clone());
        self.dirty = true;
    }

    pub fn filter_push(&mut self, text: &str) {
        let Some(buffer) = self.filter_edit.as_mut() else {
            return;
        };
        for ch in text.chars() {
            if buffer.chars().count() >= 128 {
                break;
            }
            if !ch.is_control() {
                buffer.push(ch);
            }
        }
        self.dirty = true;
    }

    pub fn filter_backspace(&mut self) {
        if let Some(buffer) = self.filter_edit.as_mut() {
            buffer.pop();
            self.dirty = true;
        }
    }

    pub fn filter_clear(&mut self) {
        if let Some(buffer) = self.filter_edit.as_mut() {
            buffer.clear();
            self.dirty = true;
        }
    }

    pub fn filter_commit(&mut self) {
        if let Some(buffer) = self.filter_edit.take() {
            buffer.trim().clone_into(&mut self.filter);
            self.selected_pid = None;
            self.selection_pinned = false;
            self.proc_scroll = 0;
            self.rebuild_rows();
            self.dirty = true;
        }
    }

    pub fn filter_cancel(&mut self) {
        self.filter_edit = None;
        self.dirty = true;
    }

    pub fn clear_filter(&mut self) {
        if !self.filter.is_empty() {
            self.filter.clear();
            self.selected_pid = None;
            self.selection_pinned = false;
            self.proc_scroll = 0;
            self.rebuild_rows();
            self.dirty = true;
        }
    }

    pub fn next_network(&mut self, delta: isize) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        if snapshot.networks.is_empty() {
            return;
        }
        let current = snapshot
            .networks
            .iter()
            .position(|net| Some(net.name.as_str()) == self.net_iface.as_deref())
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(snapshot.networks.len() as isize) as usize;
        self.net_auto = false;
        self.net_iface = Some(snapshot.networks[next].name.clone());
        self.dirty = true;
    }

    pub fn toggle_network_auto(&mut self) {
        self.net_auto = !self.net_auto;
        if self.net_auto {
            if let Some(snapshot) = &self.snapshot {
                self.net_iface = pick_network(&snapshot.networks, true, None);
            }
        }
        self.dirty = true;
    }

    pub fn select_network(&mut self, name: &str) {
        self.net_auto = false;
        self.net_iface = Some(name.to_owned());
        self.dirty = true;
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        let message = if self.paused {
            "updates paused"
        } else {
            "updates resumed"
        };
        self.set_toast(message, ToastLevel::Info);
        self.dirty = true;
    }

    pub fn faster(&mut self) {
        let step = if self.interval > 2_000 { 1_000 } else { 250 };
        self.interval = self.interval.saturating_sub(step).max(250);
        self.apply_interval();
    }

    pub fn slower(&mut self) {
        let step = if self.interval >= 2_000 { 1_000 } else { 250 };
        self.interval = self.interval.saturating_add(step).min(10_000);
        self.apply_interval();
    }

    fn apply_interval(&mut self) {
        self.config.update_ms = self.interval;
        self.update_handle.store(self.interval, Ordering::Relaxed);
        self.set_toast(
            format!("refresh interval: {} ms", self.interval),
            ToastLevel::Info,
        );
        self.dirty = true;
    }

    pub fn cycle_theme(&mut self) {
        let names = Theme::builtin_names();
        let current = names
            .iter()
            .position(|name| name.eq_ignore_ascii_case(&self.theme.name))
            .unwrap_or(0);
        let next = (current + 1) % names.len();
        if let Some(theme) = Theme::builtin(names[next]) {
            self.theme = theme;
            self.set_toast(format!("theme: {}", names[next]), ToastLevel::Info);
            self.dirty = true;
        }
    }

    pub fn open_help(&mut self) {
        self.overlay = Overlay::Help { scroll: 0 };
        self.dirty = true;
    }

    pub fn close_overlay(&mut self) {
        self.overlay = Overlay::None;
        self.dirty = true;
    }

    pub fn open_process_info(&mut self) {
        if let Some(pid) = self.selected_pid {
            self.selection_pinned = true;
            self.overlay = Overlay::ProcessInfo { pid, scroll: 0 };
            self.dirty = true;
        }
    }

    pub fn open_signal_dialog(&mut self) {
        if self.read_only {
            self.set_toast(
                "read-only mode: signalling is disabled",
                ToastLevel::Warning,
            );
            return;
        }
        let Some(process) = self.selected_process() else {
            return;
        };
        let pid = process.pid;
        let name = process.name.clone();
        let start_time = process.start_time;
        self.selection_pinned = true;
        self.overlay = Overlay::ConfirmKill {
            pid,
            name,
            start_time,
            signal: 0,
        };
        self.dirty = true;
    }

    pub fn signal_dialog_next(&mut self, delta: isize) {
        if let Overlay::ConfirmKill { signal, .. } = &mut self.overlay {
            let count = platform::signal_choices().len().max(1) as isize;
            *signal = (*signal as isize + delta).rem_euclid(count) as usize;
            self.dirty = true;
        }
    }

    pub fn confirm_signal(&mut self) {
        let Overlay::ConfirmKill {
            pid,
            name,
            start_time,
            signal,
        } = self.overlay.clone()
        else {
            return;
        };
        let choices = platform::signal_choices();
        let choice = choices.get(signal).copied().unwrap_or(choices[0]);
        match platform::signal_process(pid, start_time, &name, choice) {
            Ok(()) => {
                self.overlay = Overlay::None;
                self.set_toast(
                    format!(
                        "{} sent to {} ({pid})",
                        choice.label(),
                        format::sanitize(&name)
                    ),
                    ToastLevel::Info,
                );
            }
            Err(error) => {
                self.overlay = Overlay::None;
                self.set_toast(error, ToastLevel::Error);
            }
        }
        self.dirty = true;
    }

    pub fn scroll_overlay(&mut self, delta: i16) {
        match &mut self.overlay {
            Overlay::Help { scroll } | Overlay::ProcessInfo { scroll, .. } => {
                *scroll = scroll.saturating_add_signed(delta);
                self.dirty = true;
            }
            _ => {}
        }
    }

    pub fn set_toast(&mut self, text: impl Into<String>, level: ToastLevel) {
        let text = format::sanitize_limited(&text.into(), 200);
        let duration = match level {
            ToastLevel::Info => Duration::from_secs(3),
            ToastLevel::Warning => Duration::from_secs(5),
            ToastLevel::Error => Duration::from_secs(7),
        };
        self.toast = Some(Toast {
            text,
            level,
            expires: Instant::now() + duration,
        });
        self.dirty = true;
    }

    /// Handle a key press. Returns `true` if the application should redraw.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'C'))
        {
            self.quit = true;
            return true;
        }

        if self.overlay.is_open() {
            self.handle_overlay_key(key);
            return self.dirty;
        }

        if self.filter_edit.is_some() {
            self.handle_filter_key(key);
            return self.dirty;
        }

        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') | KeyCode::F(1) => self.open_help(),
            KeyCode::F(2) => self.cycle_theme(),
            KeyCode::Tab => self.cycle_preset(true),
            KeyCode::BackTab => self.cycle_preset(false),
            KeyCode::Char('0') => self.set_preset(Preset::Overview),
            KeyCode::Char('1') => self.set_preset(Preset::Cpu),
            KeyCode::Char('2') => self.set_preset(Preset::Memory),
            KeyCode::Char('3') => self.set_preset(Preset::Network),
            KeyCode::Char('4') => self.set_preset(Preset::Processes),
            KeyCode::Char('5') if self.config.show_gpu => self.set_preset(Preset::Gpu),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::PageUp => self.page_selection(-1),
            KeyCode::PageDown => self.page_selection(1),
            KeyCode::Home | KeyCode::Char('g') => self.selection_home(),
            KeyCode::End | KeyCode::Char('G') => self.selection_end(),
            KeyCode::Enter | KeyCode::Char('i') => self.open_process_info(),
            KeyCode::Char('f') => self.begin_filter(),
            KeyCode::Char('c') => self.set_sort(ProcSort::Cpu),
            KeyCode::Char('m') => self.set_sort(ProcSort::Memory),
            KeyCode::Char('p') => self.set_sort(ProcSort::Pid),
            KeyCode::Char('n') => self.set_sort(ProcSort::Name),
            KeyCode::Char('u') => self.set_sort(ProcSort::User),
            KeyCode::Char('r') => self.toggle_reverse(),
            KeyCode::Char('t') => self.toggle_tree(),
            KeyCode::Left => self.cycle_sort(false),
            KeyCode::Right => self.cycle_sort(true),
            KeyCode::Char('e') => self.expand_selected(),
            KeyCode::Char('w') => self.collapse_selected(),
            KeyCode::Char(' ') => self.toggle_collapsed(),
            KeyCode::Char('k') => self.open_signal_dialog(),
            KeyCode::Char('[') => self.next_network(-1),
            KeyCode::Char(']') => self.next_network(1),
            KeyCode::Char('a') => self.toggle_network_auto(),
            KeyCode::Char('+' | '=') => self.faster(),
            KeyCode::Char('-' | '_') => self.slower(),
            KeyCode::Char('s') => self.toggle_pause(),
            KeyCode::Char('x') => self.clear_filter(),
            KeyCode::Esc => {
                if !self.filter.is_empty() {
                    self.clear_filter();
                } else if self.preset != Preset::Overview {
                    self.set_preset(Preset::Overview);
                }
            }
            _ => {}
        }
        self.dirty
    }

    fn handle_filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.filter_cancel(),
            KeyCode::Enter => self.filter_commit(),
            KeyCode::Backspace => self.filter_backspace(),
            KeyCode::Delete => self.filter_clear(),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter_clear();
            }
            KeyCode::Char(ch) => self.filter_push(&ch.to_string()),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            _ => {}
        }
    }

    fn handle_overlay_key(&mut self, key: KeyEvent) {
        let overlay = self.overlay.clone();
        match overlay {
            Overlay::Help { .. } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | '?') | KeyCode::F(1) => {
                    self.close_overlay();
                }
                KeyCode::Up | KeyCode::Char('k') => self.scroll_overlay(-1),
                KeyCode::Down | KeyCode::Char('j') => self.scroll_overlay(1),
                KeyCode::PageUp => self.scroll_overlay(-10),
                KeyCode::PageDown => self.scroll_overlay(10),
                _ => {}
            },
            Overlay::ProcessInfo { .. } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'i') | KeyCode::Enter => {
                    self.close_overlay();
                }
                KeyCode::Up | KeyCode::Char('k') => self.scroll_overlay(-1),
                KeyCode::Down | KeyCode::Char('j') => self.scroll_overlay(1),
                KeyCode::PageUp => self.scroll_overlay(-10),
                KeyCode::PageDown => self.scroll_overlay(10),
                _ => {}
            },
            Overlay::ConfirmKill { .. } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'n') => self.close_overlay(),
                KeyCode::Left | KeyCode::BackTab => self.signal_dialog_next(-1),
                KeyCode::Right | KeyCode::Tab => self.signal_dialog_next(1),
                KeyCode::Enter | KeyCode::Char('y') => self.confirm_signal(),
                _ => {}
            },
            Overlay::None => {}
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        if self.filter_edit.is_some() {
            self.filter_push(text);
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.overlay.is_open() {
            return;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((preset, _)) = self
                    .layout
                    .tabs
                    .iter()
                    .find(|(_, rect)| rect.contains(mouse.column, mouse.row))
                {
                    self.set_preset(*preset);
                    return;
                }
                if self.layout.proc_body.contains(mouse.column, mouse.row)
                    && !self.proc_rows.is_empty()
                {
                    let row_in_view =
                        usize::from(mouse.row.saturating_sub(self.layout.proc_body.y));
                    let index = self.proc_scroll.saturating_add(row_in_view);
                    if let Some(row) = self.proc_rows.get(index) {
                        let pid = row.pid;
                        let is_double = self.last_click.is_some_and(|(when, last_pid)| {
                            last_pid == pid && when.elapsed() < Duration::from_millis(450)
                        });
                        self.selected_pid = Some(pid);
                        self.selection_pinned = true;
                        self.last_click = Some((Instant::now(), pid));
                        if is_double {
                            self.open_process_info();
                        }
                        self.dirty = true;
                    }
                    return;
                }
                let clicked_network = self
                    .layout
                    .net_ifaces
                    .iter()
                    .find(|(_, rect)| rect.contains(mouse.column, mouse.row))
                    .map(|(name, _)| name.clone());
                if let Some(name) = clicked_network {
                    self.select_network(&name);
                }
            }
            MouseEventKind::ScrollUp if self.layout.proc_body.contains(mouse.column, mouse.row) => {
                self.scroll_processes(-3);
            }
            MouseEventKind::ScrollDown
                if self.layout.proc_body.contains(mouse.column, mouse.row) =>
            {
                self.scroll_processes(3);
            }
            _ => {}
        }
    }
}

fn pick_network(networks: &[NetworkSnapshot], auto: bool, current: Option<&str>) -> Option<String> {
    if networks.is_empty() {
        return None;
    }
    if !auto {
        if let Some(current) = current {
            if networks.iter().any(|net| net.name == current) {
                return Some(current.to_owned());
            }
        }
    }
    let mut candidates: Vec<&NetworkSnapshot> = networks
        .iter()
        .filter(|net| !net.loopback && net.up != Some(false))
        .collect();
    if candidates.is_empty() {
        candidates = networks.iter().filter(|net| !net.loopback).collect();
    }
    if candidates.is_empty() {
        candidates = networks.iter().collect();
    }
    candidates
        .into_iter()
        .max_by(|a, b| {
            (a.received_rate + a.transmitted_rate)
                .partial_cmp(&(b.received_rate + b.transmitted_rate))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.name.cmp(&a.name))
        })
        .map(|net| net.name.clone())
}

/// Direction indicator for a process sort: descending or ascending.
#[must_use]
pub fn sort_arrow(sort: ProcSort, reverse: bool) -> &'static str {
    let descending = match sort {
        ProcSort::Cpu | ProcSort::Memory => !reverse,
        ProcSort::Pid | ProcSort::Name | ProcSort::User => reverse,
    };
    if descending { "↓" } else { "↑" }
}

/// Build the flattened process list. Kept free-standing so it can be tested
/// with synthetic process data and without a system.
pub(crate) fn build_rows(
    processes: &[ProcessInfo],
    sort: ProcSort,
    reverse: bool,
    tree: bool,
    filter: &str,
    collapsed: &HashSet<u32>,
) -> Vec<ProcRow> {
    let tokens: Vec<String> = filter
        .split_whitespace()
        .take(8)
        .map(str::to_lowercase)
        .collect();
    let matches = |process: &ProcessInfo| -> bool {
        if tokens.is_empty() {
            return true;
        }
        let haystack = format!(
            "{} {} {} {} {}",
            process.name.to_lowercase(),
            process.command.to_lowercase(),
            process.user.to_lowercase(),
            process.pid,
            process.state.label()
        );
        tokens.iter().all(|token| haystack.contains(token))
    };
    // The process table is keyed by PID. Keep a single row even if a platform
    // backend ever returns the same PID twice (for example a task exposed both
    // in /proc and /proc/<pid>/task).
    let mut unique = HashSet::with_capacity(processes.len());
    let mut indices: Vec<usize> = processes
        .iter()
        .enumerate()
        .filter(|(_, process)| unique.insert(process.pid))
        .filter(|(_, process)| matches(process))
        .map(|(index, _)| index)
        .collect();
    sort_indices(&mut indices, processes, sort, reverse);

    if !tree {
        return indices
            .into_iter()
            .map(|index| ProcRow {
                pid: processes[index].pid,
                depth: 0,
                is_last: true,
                ancestors_last: Vec::new(),
                has_children: false,
                collapsed: false,
            })
            .collect();
    }

    let by_pid: HashMap<u32, usize> = indices
        .iter()
        .map(|index| (processes[*index].pid, *index))
        .collect();
    let mut children: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut roots: Vec<usize> = Vec::new();
    for index in &indices {
        let process = &processes[*index];
        match process.parent {
            Some(parent) if parent != process.pid && by_pid.contains_key(&parent) => {
                children.entry(parent).or_default().push(*index);
            }
            _ => roots.push(*index),
        }
    }
    sort_indices(&mut roots, processes, sort, reverse);
    for siblings in children.values_mut() {
        sort_indices(siblings, processes, sort, reverse);
    }

    let mut rows = Vec::with_capacity(indices.len());
    let mut visited: HashSet<u32> = HashSet::with_capacity(indices.len());
    let mut stack: Vec<(usize, u16, bool, Vec<bool>)> = roots
        .iter()
        .rev()
        .map(|index| (*index, 0u16, true, Vec::new()))
        .collect();

    while let Some((index, depth, is_last, ancestors_last)) = stack.pop() {
        let process = &processes[index];
        if !visited.insert(process.pid) {
            continue;
        }
        let has_children = children
            .get(&process.pid)
            .is_some_and(|list| !list.is_empty());
        let is_collapsed = has_children && collapsed.contains(&process.pid);
        rows.push(ProcRow {
            pid: process.pid,
            depth,
            is_last,
            ancestors_last: ancestors_last.clone(),
            has_children,
            collapsed: is_collapsed,
        });
        if is_collapsed || depth >= 32 {
            continue;
        }
        if let Some(siblings) = children.get(&process.pid) {
            for (position, child) in siblings.iter().enumerate().rev() {
                let child_last = position + 1 == siblings.len();
                let mut child_ancestors = ancestors_last.clone();
                if depth > 0 {
                    child_ancestors.push(is_last);
                }
                stack.push((*child, depth + 1, child_last, child_ancestors));
            }
        }
    }

    // Cycles or an unusually deep tree can leave processes unvisited. Append
    // them as roots so nothing silently disappears. Descendants hidden by a
    // collapsed ancestor are intentional omissions and must stay hidden.
    if rows.len() < indices.len() {
        for index in indices {
            if visited.contains(&processes[index].pid) {
                continue;
            }
            if hidden_by_collapsed_ancestor(&processes[index], processes, &by_pid, collapsed) {
                continue;
            }
            rows.push(ProcRow {
                pid: processes[index].pid,
                depth: 0,
                is_last: true,
                ancestors_last: Vec::new(),
                has_children: children
                    .get(&processes[index].pid)
                    .is_some_and(|list| !list.is_empty()),
                collapsed: false,
            });
        }
    }
    rows
}

fn hidden_by_collapsed_ancestor(
    process: &ProcessInfo,
    processes: &[ProcessInfo],
    by_pid: &HashMap<u32, usize>,
    collapsed: &HashSet<u32>,
) -> bool {
    let mut parent = process.parent;
    let mut hops = 0;
    while let Some(pid) = parent {
        if collapsed.contains(&pid) {
            return true;
        }
        if hops >= 64 {
            return false;
        }
        let Some(index) = by_pid.get(&pid) else {
            return false;
        };
        parent = processes[*index].parent;
        hops += 1;
    }
    false
}

fn sort_indices(indices: &mut [usize], processes: &[ProcessInfo], sort: ProcSort, reverse: bool) {
    indices.sort_by(|a, b| {
        let a = &processes[*a];
        let b = &processes[*b];
        let order = match sort {
            ProcSort::Cpu => b
                .cpu
                .partial_cmp(&a.cpu)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.pid.cmp(&b.pid)),
            ProcSort::Memory => b.memory.cmp(&a.memory).then_with(|| a.pid.cmp(&b.pid)),
            ProcSort::Pid => a.pid.cmp(&b.pid),
            ProcSort::Name => a
                .name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.pid.cmp(&b.pid)),
            ProcSort::User => a
                .user
                .to_lowercase()
                .cmp(&b.user.to_lowercase())
                .then_with(|| a.pid.cmp(&b.pid)),
        };
        if reverse { order.reverse() } else { order }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{ProcessInfo, ProcessState};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn process(pid: u32, parent: Option<u32>, name: &str, cpu: f32, memory: u64) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent,
            name: name.to_owned(),
            command: format!("{name} --flag"),
            user: "tester".to_owned(),
            state: ProcessState::Sleeping,
            cpu,
            memory,
            virtual_memory: memory * 2,
            threads: Some(1),
            start_time: 100,
            run_time: 10,
            exe: None,
            cwd: None,
            read_bytes: 0,
            written_bytes: 0,
            session: None,
        }
    }

    #[test]
    fn flat_rows_sort_by_cpu() {
        let processes = vec![
            process(1, None, "init", 1.0, 10),
            process(2, None, "busy", 50.0, 10),
        ];
        let rows = build_rows(&processes, ProcSort::Cpu, false, false, "", &HashSet::new());
        assert_eq!(
            rows.iter().map(|row| row.pid).collect::<Vec<_>>(),
            vec![2, 1]
        );
    }

    #[test]
    fn duplicate_pids_are_collapsed_to_one_row() {
        let processes = vec![
            process(7, None, "worker", 10.0, 10),
            process(7, None, "worker-task", 10.0, 10),
            process(8, None, "other", 1.0, 10),
        ];
        let rows = build_rows(&processes, ProcSort::Pid, false, false, "", &HashSet::new());
        assert_eq!(
            rows.iter().map(|row| row.pid).collect::<Vec<_>>(),
            vec![7, 8]
        );
    }

    #[test]
    fn disabled_gpu_view_is_skipped_by_tab_cycling() {
        let theme = crate::theme::Theme::builtin("default").unwrap();
        let config = Config {
            show_gpu: false,
            ..Config::default()
        };
        let mut app = App::new(config, theme, Vec::new(), Arc::new(AtomicU64::new(1000)));
        for _ in 0..12 {
            app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
            assert_ne!(app.preset, Preset::Gpu);
        }
        for _ in 0..12 {
            app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE));
            assert_ne!(app.preset, Preset::Gpu);
        }
    }

    #[test]
    fn left_and_right_cycle_the_process_sort() {
        let theme = crate::theme::Theme::builtin("default").unwrap();
        let mut app = App::new(
            Config::default(),
            theme,
            Vec::new(),
            Arc::new(AtomicU64::new(1000)),
        );
        assert_eq!(app.sort, ProcSort::Cpu);
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.sort, ProcSort::Memory);
        app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.sort, ProcSort::Cpu);
        app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.sort, ProcSort::User);
    }

    #[test]
    fn reverse_flips_the_order() {
        let processes = vec![
            process(1, None, "one", 1.0, 10),
            process(2, None, "two", 2.0, 10),
        ];
        let rows = build_rows(&processes, ProcSort::Pid, true, false, "", &HashSet::new());
        assert_eq!(
            rows.iter().map(|row| row.pid).collect::<Vec<_>>(),
            vec![2, 1]
        );
    }

    #[test]
    fn tree_rows_carry_depth_and_last_flags() {
        let processes = vec![
            process(1, None, "root", 0.0, 1),
            process(2, Some(1), "child-a", 0.0, 1),
            process(3, Some(1), "child-b", 0.0, 1),
            process(4, Some(2), "grandchild", 0.0, 1),
        ];
        let rows = build_rows(&processes, ProcSort::Pid, false, true, "", &HashSet::new());
        assert_eq!(
            rows.iter()
                .map(|row| (row.pid, row.depth))
                .collect::<Vec<_>>(),
            vec![(1, 0), (2, 1), (4, 2), (3, 1)]
        );
        assert!(!rows[1].is_last);
        assert!(rows[3].is_last);
        assert_eq!(rows[2].ancestors_last, vec![false]);
    }

    #[test]
    fn collapsed_nodes_hide_children() {
        let processes = vec![
            process(1, None, "root", 0.0, 1),
            process(2, Some(1), "child", 0.0, 1),
        ];
        let mut collapsed = HashSet::new();
        collapsed.insert(1);
        let rows = build_rows(&processes, ProcSort::Pid, false, true, "", &collapsed);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].collapsed);
    }

    #[test]
    fn filter_tokens_are_anded() {
        let processes = vec![
            process(1, None, "alpha", 0.0, 1),
            process(2, None, "beta", 0.0, 1),
        ];
        let rows = build_rows(
            &processes,
            ProcSort::Pid,
            false,
            false,
            "beta tester",
            &HashSet::new(),
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pid, 2);
    }

    #[test]
    fn cycles_do_not_loop_forever() {
        let processes = vec![
            process(1, Some(2), "one", 0.0, 1),
            process(2, Some(1), "two", 0.0, 1),
        ];
        let rows = build_rows(&processes, ProcSort::Pid, false, true, "", &HashSet::new());
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn presets_cycle_in_both_directions() {
        assert_eq!(Preset::Overview.next(), Preset::Cpu);
        assert_eq!(Preset::Overview.previous(), Preset::Gpu);
        for preset in Preset::all() {
            assert_eq!(preset.next().previous(), preset);
            assert_eq!(Preset::from_u8(preset.index()), preset);
        }
    }

    fn test_app(read_only: bool) -> App {
        let config = Config {
            read_only,
            ..Config::default()
        };
        let theme = crate::theme::Theme::builtin("default").unwrap();
        let mut app = App::new(config, theme, Vec::new(), Arc::new(AtomicU64::new(1_000)));
        let mut snapshot = crate::platform::unavailable_snapshot("test");
        snapshot.warnings.clear();
        snapshot.cpu.overall_usage = 20.0;
        snapshot.cpu.cores = vec![10.0, 30.0];
        snapshot.memory.total = 1024 * 1024;
        snapshot.memory.used = 512 * 1024;
        snapshot.memory.available = 512 * 1024;
        snapshot.processes = vec![
            process(10, None, "alpha", 5.0, 100),
            process(20, Some(10), "bravo", 20.0, 200),
            process(30, Some(10), "charlie", 0.0, 50),
        ];
        snapshot.process_summary.total = 3;
        snapshot.networks = vec![
            crate::platform::NetworkSnapshot {
                name: "eth0".to_owned(),
                received_rate: 1024.0,
                transmitted_rate: 512.0,
                loopback: false,
                up: Some(true),
                ..crate::platform::NetworkSnapshot::default()
            },
            crate::platform::NetworkSnapshot {
                name: "lo".to_owned(),
                received_rate: 4096.0,
                transmitted_rate: 4096.0,
                loopback: true,
                up: Some(true),
                ..crate::platform::NetworkSnapshot::default()
            },
        ];
        app.apply_snapshot(snapshot);
        app
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn first_snapshot_selects_the_first_row() {
        let app = test_app(false);
        assert_eq!(app.proc_rows.len(), 3);
        assert_eq!(app.selected_pid, Some(20));
        assert_eq!(app.cpu_history.len(), 1);
        assert_eq!(app.core_history.len(), 2);
    }

    #[test]
    fn filter_editing_commits_and_clears() {
        let mut app = test_app(false);
        app.handle_key(key(KeyCode::Char('f')));
        assert!(app.filter_edit.is_some());
        for ch in "bravo".chars() {
            app.handle_key(key(KeyCode::Char(ch)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.filter, "bravo");
        assert_eq!(app.proc_rows.len(), 1);
        assert_eq!(app.selected_pid, Some(20));
        app.handle_key(key(KeyCode::Esc));
        assert!(app.filter.is_empty());
        assert_eq!(app.proc_rows.len(), 3);
    }

    #[test]
    fn filter_cancel_restores_the_previous_filter() {
        let mut app = test_app(false);
        app.handle_key(key(KeyCode::Char('f')));
        app.handle_key(key(KeyCode::Char('x')));
        app.handle_key(key(KeyCode::Esc));
        assert!(app.filter.is_empty());
        assert!(app.filter_edit.is_none());
    }

    #[test]
    fn tree_toggle_rebuilds_rows() {
        let mut app = test_app(false);
        app.handle_key(key(KeyCode::Char('t')));
        assert!(app.tree);
        let depths: Vec<u16> = app.proc_rows.iter().map(|row| row.depth).collect();
        assert!(depths.contains(&1));
        app.handle_key(key(KeyCode::Char('t')));
        assert!(!app.tree);
        assert!(app.proc_rows.iter().all(|row| row.depth == 0));
    }

    #[test]
    fn help_overlay_opens_and_closes() {
        let mut app = test_app(false);
        app.handle_key(key(KeyCode::Char('?')));
        assert!(matches!(app.overlay, Overlay::Help { .. }));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.overlay, Overlay::None));
    }

    #[test]
    fn kill_dialog_opens_for_writable_session() {
        let mut app = test_app(false);
        app.handle_key(key(KeyCode::Char('k')));
        assert!(matches!(app.overlay, Overlay::ConfirmKill { .. }));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.overlay, Overlay::None));
    }

    #[test]
    fn read_only_session_refuses_to_open_kill_dialog() {
        let mut app = test_app(true);
        app.handle_key(key(KeyCode::Char('k')));
        assert!(matches!(app.overlay, Overlay::None));
        assert!(
            app.toast
                .as_ref()
                .is_some_and(|toast| toast.text.contains("read-only"))
        );
    }

    #[test]
    fn ctrl_c_always_quits() {
        let mut app = test_app(false);
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.quit);
    }

    #[test]
    fn unpinned_selection_follows_the_top_process() {
        let mut app = test_app(false);
        assert_eq!(app.selected_pid, Some(20));
        let mut second = app.snapshot.clone().unwrap();
        for process in &mut second.processes {
            match process.pid {
                10 => process.cpu = 90.0,
                20 => process.cpu = 1.0,
                _ => {}
            }
        }
        app.apply_snapshot(second);
        assert_eq!(app.selected_pid, Some(10));
        assert_eq!(app.proc_scroll, 0);
    }

    #[test]
    fn pinned_selection_tracks_the_process_across_refreshes() {
        let mut app = test_app(false);
        app.move_selection(1);
        let pinned = app.selected_pid.expect("selection");
        let pinned_index = app.selected_row().expect("row");
        assert!(pinned_index > 0);
        let mut second = app.snapshot.clone().unwrap();
        for process in &mut second.processes {
            process.cpu = f32::from(process.pid as u8);
        }
        app.apply_snapshot(second);
        assert_eq!(app.selected_pid, Some(pinned));
    }

    #[test]
    fn layout_clamps_scroll_and_keeps_selection_visible() {
        let mut app = test_app(false);
        app.proc_scroll = 99;
        app.layout.proc_visible = 2;
        app.set_layout(Layout {
            proc_visible: 2,
            ..Layout::default()
        });
        assert!(app.proc_scroll <= 1);
        if let Some(index) = app.selected_row() {
            assert!(index >= app.proc_scroll);
            assert!(index < app.proc_scroll + 2);
        }
    }

    #[test]
    fn sort_arrows_describe_the_actual_order() {
        use crate::config::ProcSort;
        // CPU and memory default to descending; identity sorts ascend.
        assert_eq!(sort_arrow(ProcSort::Cpu, false), "↓");
        assert_eq!(sort_arrow(ProcSort::Cpu, true), "↑");
        assert_eq!(sort_arrow(ProcSort::Pid, false), "↑");
        assert_eq!(sort_arrow(ProcSort::Pid, true), "↓");
    }

    #[test]
    fn network_history_accumulates_for_each_interface() {
        let mut app = test_app(false);
        assert_eq!(app.net_iface.as_deref(), Some("eth0"));
        assert_eq!(app.net_history.get("eth0").unwrap().rx.len(), 1);
        let mut second = app.snapshot.clone().unwrap();
        for network in &mut second.networks {
            network.received_rate *= 2.0;
        }
        app.apply_snapshot(second);
        assert_eq!(app.net_history.get("eth0").unwrap().rx.len(), 2);
        assert_eq!(app.net_history.get("lo").unwrap().tx.len(), 2);
    }

    #[test]
    fn manual_interface_selection_survives_refreshes() {
        let mut app = test_app(false);
        app.select_network("lo");
        assert!(!app.net_auto);
        let snapshot = app.snapshot.clone().unwrap();
        app.apply_snapshot(snapshot);
        assert_eq!(app.net_iface.as_deref(), Some("lo"));
        app.toggle_network_auto();
        assert!(app.net_auto);
        assert_eq!(app.net_iface.as_deref(), Some("eth0"));
    }

    #[test]
    fn tty_mode_forces_ascii_glyphs() {
        let config = Config {
            tty_mode: true,
            graph_symbol: crate::config::GraphSymbol::Braille,
            ..Config::default()
        };
        let app = App::new(
            config,
            crate::theme::Theme::builtin("default").unwrap(),
            Vec::new(),
            Arc::new(AtomicU64::new(1000)),
        );
        assert_eq!(app.graph_symbol(), crate::config::GraphSymbol::Tty);
        assert!(app.ascii());
    }
}
