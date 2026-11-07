//! Cross-platform system sampling.
//!
//! [`Collector`] wraps [`sysinfo`] so the UI receives one plain, owned
//! snapshot per refresh. Linux adds a few optional details that `sysinfo`
//! does not expose (CPU time breakdown, cached memory, interface speed and
//! wireless state, battery); every other platform simply leaves those fields
//! empty and keeps working.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use sysinfo::{
    Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, Pid, ProcessRefreshKind,
    ProcessStatus, ProcessesToUpdate, RefreshKind, System, Uid, UpdateKind, Users,
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// A point-in-time view of the machine.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub host: HostInfo,
    pub cpu: CpuSnapshot,
    pub memory: MemorySnapshot,
    pub disks: Vec<DiskSnapshot>,
    pub networks: Vec<NetworkSnapshot>,
    pub processes: Vec<ProcessInfo>,
    pub process_summary: ProcessSummary,
    pub sensors: Vec<SensorReading>,
    pub battery: Option<BatteryInfo>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub arch: String,
    pub user: String,
    pub cpu_model: String,
}

#[derive(Clone, Debug, Default)]
pub struct CpuSnapshot {
    pub overall_usage: f32,
    pub cores: Vec<f32>,
    pub frequencies_mhz: Vec<u64>,
    pub average_frequency_mhz: u64,
    pub load: [f64; 3],
    pub uptime_secs: u64,
    /// Optional user/system/idle/wait split (Linux only).
    pub breakdown: Option<CpuBreakdown>,
    /// Temperature of the CPU package, if a sensor is available.
    pub temperature: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CpuBreakdown {
    pub user: f32,
    pub system: f32,
    pub wait: f32,
    pub idle: f32,
}

#[derive(Clone, Debug, Default)]
pub struct MemorySnapshot {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub free: u64,
    /// Reclaimable page cache (Linux only).
    pub cached: Option<u64>,
    pub swap_total: u64,
    pub swap_used: u64,
    pub swap_free: u64,
}

#[derive(Clone, Debug, Default)]
pub struct DiskSnapshot {
    pub name: String,
    pub mount: String,
    pub filesystem: String,
    pub total: u64,
    pub available: u64,
    pub used: u64,
    pub read_rate: f64,
    pub write_rate: f64,
    pub removable: bool,
    pub read_only: bool,
}

#[derive(Clone, Debug, Default)]
pub struct NetworkSnapshot {
    pub name: String,
    pub received_rate: f64,
    pub transmitted_rate: f64,
    pub total_received: u64,
    pub total_transmitted: u64,
    pub speed_mbps: Option<u64>,
    pub wireless: bool,
    pub loopback: bool,
    pub up: Option<bool>,
    pub addresses: Vec<String>,
    pub mac: String,
}

#[derive(Clone, Debug, Default)]
pub struct SensorReading {
    pub label: String,
    pub temperature: f32,
    pub max: Option<f32>,
    pub critical: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl BatteryState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Charging => "charging",
            Self::Discharging => "discharging",
            Self::Full => "full",
            Self::NotCharging => "not charging",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct BatteryInfo {
    pub percent: u8,
    pub state: BatteryState,
    pub minutes_remaining: Option<u32>,
    pub power_w: Option<f32>,
}

#[derive(Clone, Debug, Default)]
pub struct ProcessSummary {
    pub total: usize,
    pub running: usize,
    pub sleeping: usize,
    pub zombie: usize,
    pub threads: usize,
}

#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub command: String,
    pub user: String,
    pub state: ProcessState,
    pub cpu: f32,
    pub memory: u64,
    pub virtual_memory: u64,
    pub threads: Option<u32>,
    pub start_time: u64,
    pub run_time: u64,
    pub exe: Option<String>,
    pub cwd: Option<String>,
    pub read_bytes: u64,
    pub written_bytes: u64,
    pub session: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProcessState {
    #[default]
    Unknown,
    Idle,
    Running,
    Sleeping,
    Stopped,
    Zombie,
    Tracing,
    Dead,
    Wakekill,
    Waking,
    Parked,
    LockBlocked,
    DiskSleep,
}

impl ProcessState {
    #[must_use]
    pub const fn short(self) -> char {
        match self {
            Self::Idle => 'I',
            Self::Running => 'R',
            Self::Sleeping => 'S',
            Self::Stopped => 'T',
            Self::Zombie => 'Z',
            Self::Tracing => 't',
            Self::Dead => 'X',
            Self::Wakekill | Self::Waking => 'W',
            Self::Parked => 'P',
            Self::LockBlocked => 'L',
            Self::DiskSleep => 'D',
            Self::Unknown => '?',
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Sleeping => "sleeping",
            Self::Stopped => "stopped",
            Self::Zombie => "zombie",
            Self::Tracing => "tracing",
            Self::Dead => "dead",
            Self::Wakekill => "wakekill",
            Self::Waking => "waking",
            Self::Parked => "parked",
            Self::LockBlocked => "lock blocked",
            Self::DiskSleep => "disk sleep",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::DiskSleep)
    }
}

impl From<ProcessStatus> for ProcessState {
    fn from(status: ProcessStatus) -> Self {
        match status {
            ProcessStatus::Idle => Self::Idle,
            ProcessStatus::Run => Self::Running,
            ProcessStatus::Sleep => Self::Sleeping,
            ProcessStatus::Stop => Self::Stopped,
            ProcessStatus::Zombie => Self::Zombie,
            ProcessStatus::Tracing => Self::Tracing,
            ProcessStatus::Dead => Self::Dead,
            ProcessStatus::Wakekill => Self::Wakekill,
            ProcessStatus::Waking => Self::Waking,
            ProcessStatus::Parked => Self::Parked,
            ProcessStatus::LockBlocked => Self::LockBlocked,
            ProcessStatus::UninterruptibleDiskSleep => Self::DiskSleep,
            ProcessStatus::Unknown(_) => Self::Unknown,
        }
    }
}

/// Background-friendly sampler.
#[allow(missing_debug_implementations)]
pub struct Collector {
    system: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    user_names: HashMap<Uid, String>,
    last_sample: Instant,
    tick: u64,
    cached_sensors: Vec<SensorReading>,
    cpu_model: String,
    warnings: Vec<String>,
    #[cfg(target_os = "linux")]
    linux: linux::Extras,
}

impl Collector {
    /// Create a collector and take the first internal baseline samples.
    ///
    /// The first snapshot intentionally reports zero rates; the caller should
    /// wait at least [`sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`] and sample again.
    #[must_use]
    pub fn new() -> Self {
        let system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_memory(MemoryRefreshKind::everything())
                .with_cpu(CpuRefreshKind::everything()),
        );
        let mut collector = Self {
            system,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            user_names: HashMap::new(),
            last_sample: Instant::now(),
            tick: 0,
            cached_sensors: Vec::new(),
            cpu_model: String::new(),
            warnings: Vec::new(),
            #[cfg(target_os = "linux")]
            linux: linux::Extras::new(),
        };
        collector.load_users();
        collector.refresh_sensors();
        collector
    }

    fn load_users(&mut self) {
        let users = Users::new_with_refreshed_list();
        self.user_names = users
            .list()
            .iter()
            .map(|user| (user.id().clone(), user.name().to_owned()))
            .collect();
    }

    fn refresh_sensors(&mut self) {
        self.components.refresh(true);
        let mut sensors: Vec<SensorReading> = self
            .components
            .list()
            .iter()
            .filter_map(|component| {
                let temperature = component.temperature()?;
                if !temperature.is_finite() || !(-40.0..=200.0).contains(&temperature) {
                    return None;
                }
                Some(SensorReading {
                    label: component.label().to_owned(),
                    temperature,
                    max: component.max().filter(|v| v.is_finite()),
                    critical: component.critical().filter(|v| v.is_finite()),
                })
            })
            .collect();
        sensors.sort_by_key(|a| a.label.to_lowercase());
        sensors.truncate(32);
        self.cached_sensors = sensors;
    }

    /// Take a snapshot. This never panics on missing platform data; callers
    /// run it off the UI thread so a slow `/proc` or WMI query cannot freeze
    /// input handling.
    #[must_use]
    pub fn sample(&mut self) -> Snapshot {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_sample).as_secs_f64().max(0.05);
        self.last_sample = now;

        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage()
                .with_user(UpdateKind::OnlyIfNotSet)
                .with_cmd(UpdateKind::OnlyIfNotSet)
                .with_exe(UpdateKind::OnlyIfNotSet)
                .with_cwd(UpdateKind::OnlyIfNotSet),
        );
        self.disks.refresh(true);
        self.networks.refresh(true);

        // Temperature providers on Windows can use WMI and are comparatively
        // expensive. Sample them less often; the graph refresh stays snappy.
        let sensor_period = if cfg!(target_os = "linux") { 2 } else { 10 };
        if self.tick % sensor_period == 0 {
            self.refresh_sensors();
        }
        self.tick = self.tick.wrapping_add(1);

        let cpu = self.build_cpu();
        let memory = self.build_memory();
        let disks = self.build_disks(elapsed);
        let networks = self.build_networks(elapsed);
        let (processes, process_summary) = self.build_processes();
        let battery = sample_battery();
        let host = self.build_host(&cpu);

        let mut warnings = std::mem::take(&mut self.warnings);
        if cpu.cores.is_empty() {
            warnings.push("no CPU information is available".to_owned());
        }
        if memory.total == 0 {
            warnings.push("no memory information is available".to_owned());
        }
        if processes.is_empty() {
            warnings.push("no processes could be read (permissions?)".to_owned());
        }
        warnings.sort();
        warnings.dedup();
        warnings.truncate(8);

        Snapshot {
            host,
            cpu,
            memory,
            disks,
            networks,
            processes,
            process_summary,
            sensors: self.cached_sensors.clone(),
            battery,
            warnings,
        }
    }

    fn build_host(&mut self, cpu: &CpuSnapshot) -> HostInfo {
        if self.cpu_model.is_empty() {
            let raw = self
                .system
                .cpus()
                .first()
                .map(|cpu| {
                    let brand = cpu.brand().trim();
                    if brand.is_empty() {
                        cpu.name().trim()
                    } else {
                        brand
                    }
                })
                .filter(|name| !name.is_empty())
                .unwrap_or("unknown CPU")
                .to_owned();
            let generic = raw.eq_ignore_ascii_case("unknown CPU")
                || raw.strip_prefix("cpu").is_some_and(|rest| {
                    rest.is_empty() || rest.chars().all(|c| c.is_ascii_digit())
                });
            self.cpu_model = if generic {
                format!("{} CPU", System::cpu_arch())
            } else {
                raw
            };
        }
        let os = System::long_os_version()
            .or_else(System::os_version)
            .or_else(System::name)
            .unwrap_or_else(|| std::env::consts::OS.to_owned());
        let hostname = System::host_name()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "localhost".to_owned());
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .ok()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "-".to_owned());
        let _ = cpu;
        HostInfo {
            hostname,
            os,
            kernel: System::kernel_version().unwrap_or_else(|| "-".to_owned()),
            arch: System::cpu_arch(),
            user,
            cpu_model: self.cpu_model.clone(),
        }
    }

    fn build_cpu(&mut self) -> CpuSnapshot {
        let cores: Vec<f32> = self
            .system
            .cpus()
            .iter()
            .map(|cpu| {
                let usage = cpu.cpu_usage();
                if usage.is_finite() {
                    usage.clamp(0.0, 100.0)
                } else {
                    0.0
                }
            })
            .collect();
        let frequencies_mhz: Vec<u64> = self
            .system
            .cpus()
            .iter()
            .map(sysinfo::Cpu::frequency)
            .collect();
        let (freq_sum, freq_count) = frequencies_mhz
            .iter()
            .filter(|freq| **freq > 0)
            .fold((0u64, 0u64), |(sum, count), freq| {
                (sum.saturating_add(*freq), count + 1)
            });
        let average_frequency_mhz = freq_sum.checked_div(freq_count).unwrap_or(0);
        let load = System::load_average();
        let breakdown = {
            #[cfg(target_os = "linux")]
            {
                self.linux.cpu_breakdown()
            }
            #[cfg(not(target_os = "linux"))]
            {
                None
            }
        };
        let temperature = pick_cpu_temperature(&self.cached_sensors);
        let overall = self.system.global_cpu_usage();
        let overall_usage = if overall.is_finite() {
            overall.clamp(0.0, 100.0)
        } else {
            0.0
        };
        CpuSnapshot {
            overall_usage,
            cores,
            frequencies_mhz,
            average_frequency_mhz,
            load: [load.one, load.five, load.fifteen],
            uptime_secs: System::uptime(),
            breakdown,
            temperature,
        }
    }

    fn build_memory(&mut self) -> MemorySnapshot {
        let cached = {
            #[cfg(target_os = "linux")]
            {
                linux::Extras::cached_memory()
            }
            #[cfg(not(target_os = "linux"))]
            {
                None
            }
        };
        MemorySnapshot {
            total: self.system.total_memory(),
            used: self.system.used_memory(),
            available: self.system.available_memory(),
            free: self.system.free_memory(),
            cached,
            swap_total: self.system.total_swap(),
            swap_used: self.system.used_swap(),
            swap_free: self.system.free_swap(),
        }
    }

    fn build_disks(&mut self, elapsed: f64) -> Vec<DiskSnapshot> {
        let mut seen = HashSet::new();
        let mut disks: Vec<DiskSnapshot> = self
            .disks
            .list()
            .iter()
            .filter_map(|disk| {
                let total = disk.total_space();
                if total == 0 {
                    return None;
                }
                let mount = disk.mount_point().to_string_lossy().into_owned();
                if !seen.insert(mount.clone()) {
                    return None;
                }
                let available = disk.available_space().min(total);
                let usage = disk.usage();
                Some(DiskSnapshot {
                    name: disk.name().to_string_lossy().into_owned(),
                    mount,
                    filesystem: disk.file_system().to_string_lossy().into_owned(),
                    total,
                    available,
                    used: total.saturating_sub(available),
                    read_rate: usage.read_bytes as f64 / elapsed,
                    write_rate: usage.written_bytes as f64 / elapsed,
                    removable: disk.is_removable(),
                    read_only: disk.is_read_only(),
                })
            })
            .collect();
        disks.sort_by_key(|a| a.mount.to_lowercase());
        // Bind mounts expose the same block device at several paths. Keep the
        // first (shortest after sorting) mount for each device/capacity pair.
        let mut seen_devices: HashSet<(String, u64, u64)> = HashSet::new();
        disks.retain(|disk| {
            disk.name.is_empty()
                || seen_devices.insert((disk.name.clone(), disk.total, disk.available))
        });
        if disks.len() > 64 {
            self.warnings.push(format!(
                "{} disks detected; showing the first 64",
                disks.len()
            ));
            disks.truncate(64);
        }
        disks
    }

    fn build_networks(&mut self, elapsed: f64) -> Vec<NetworkSnapshot> {
        let mut networks: Vec<NetworkSnapshot> = self
            .networks
            .iter()
            .map(|(name, data)| {
                let loopback = is_loopback_name(name);
                let (speed_mbps, wireless, up) = interface_details(name);
                NetworkSnapshot {
                    name: name.clone(),
                    received_rate: data.received() as f64 / elapsed,
                    transmitted_rate: data.transmitted() as f64 / elapsed,
                    total_received: data.total_received(),
                    total_transmitted: data.total_transmitted(),
                    speed_mbps,
                    wireless,
                    loopback,
                    up,
                    addresses: data
                        .ip_networks()
                        .iter()
                        .take(4)
                        .map(ToString::to_string)
                        .collect(),
                    mac: {
                        let mac = data.mac_address();
                        if mac.is_unspecified() {
                            String::new()
                        } else {
                            mac.to_string()
                        }
                    },
                }
            })
            .collect();
        networks.sort_by(|a, b| {
            a.loopback
                .cmp(&b.loopback)
                .then_with(|| {
                    (b.received_rate + b.transmitted_rate)
                        .partial_cmp(&(a.received_rate + a.transmitted_rate))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.name.cmp(&b.name))
        });
        networks.truncate(64);
        networks
    }

    fn build_processes(&mut self) -> (Vec<ProcessInfo>, ProcessSummary) {
        let total_processes = self.system.processes().len();
        let cap = total_processes.min(20_000);
        if total_processes > cap {
            self.warnings.push(format!(
                "{total_processes} processes detected; showing {cap}"
            ));
        }
        let mut summary = ProcessSummary::default();
        let mut processes = Vec::with_capacity(cap);
        for (pid, process) in self.system.processes().iter().take(cap) {
            let state = ProcessState::from(process.status());
            summary.total += 1;
            match state {
                ProcessState::Running => summary.running += 1,
                ProcessState::Sleeping | ProcessState::Idle => summary.sleeping += 1,
                ProcessState::Zombie => summary.zombie += 1,
                _ => {}
            }
            let threads = process_thread_count(pid.as_u32());
            if let Some(threads) = threads {
                summary.threads = summary.threads.saturating_add(threads as usize);
            }
            let user = process
                .user_id()
                .and_then(|uid| self.user_names.get(uid))
                .cloned()
                .unwrap_or_else(|| "-".to_owned());
            let name = process.name().to_string_lossy().into_owned();
            let mut command = process
                .cmd()
                .iter()
                .map(|part| part.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            if command.trim().is_empty() {
                command = process
                    .exe()
                    .map_or_else(|| name.clone(), |path| path.to_string_lossy().into_owned());
            }
            let usage = process.disk_usage();
            processes.push(ProcessInfo {
                pid: pid.as_u32(),
                parent: process.parent().map(Pid::as_u32),
                name: truncate_chars(&name, 256),
                command: truncate_chars(&command, 4096),
                user: truncate_chars(&user, 128),
                state,
                cpu: process.cpu_usage().max(0.0),
                memory: process.memory(),
                virtual_memory: process.virtual_memory(),
                threads,
                start_time: process.start_time(),
                run_time: process.run_time(),
                exe: process
                    .exe()
                    .map(|path| truncate_chars(&path.to_string_lossy(), 1024)),
                cwd: process
                    .cwd()
                    .map(|path| truncate_chars(&path.to_string_lossy(), 1024)),
                read_bytes: usage.total_read_bytes,
                written_bytes: usage.total_written_bytes,
                session: process.session_id().map(Pid::as_u32),
            });
        }
        (processes, summary)
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        value.chars().take(max).collect()
    }
}

fn is_loopback_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "lo" || lower == "lo0" || lower.contains("loopback")
}

fn pick_cpu_temperature(sensors: &[SensorReading]) -> Option<f32> {
    let mut best: Option<(u8, f32)> = None;
    for sensor in sensors {
        let label = sensor.label.to_ascii_lowercase();
        let core_sensor =
            label.contains("core") || label.contains("k10temp") || label.contains("coretemp");
        let score =
            if label.contains("package id 0") || label.contains("tctl") || label.contains("tdie") {
                3
            } else if label.contains("cpu") || label.contains("soc") || label.contains("processor")
            {
                2
            } else {
                u8::from(core_sensor)
            };
        if score == 0 {
            continue;
        }
        if best.is_none_or(|(best_score, best_temp)| {
            score > best_score || (score == best_score && sensor.temperature > best_temp)
        }) {
            best = Some((score, sensor.temperature));
        }
    }
    best.map(|(_, temp)| temp)
}

fn process_thread_count(pid: u32) -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        linux::thread_count(pid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

fn interface_details(name: &str) -> (Option<u64>, bool, Option<bool>) {
    #[cfg(target_os = "linux")]
    {
        linux::interface_details(name)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = name;
        (None, false, None)
    }
}

fn sample_battery() -> Option<BatteryInfo> {
    #[cfg(target_os = "linux")]
    {
        linux::battery()
    }
    #[cfg(windows)]
    {
        windows::battery()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        None
    }
}

/// Snapshot used when sampling fails completely (for example a panic in a
/// platform provider). The UI stays alive and shows the warning.
#[must_use]
pub fn unavailable_snapshot(message: impl Into<String>) -> Snapshot {
    Snapshot {
        host: HostInfo::default(),
        cpu: CpuSnapshot::default(),
        memory: MemorySnapshot::default(),
        disks: Vec::new(),
        networks: Vec::new(),
        processes: Vec::new(),
        process_summary: ProcessSummary::default(),
        sensors: Vec::new(),
        battery: None,
        warnings: vec![message.into()],
    }
}

/// A signal the UI is allowed to send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalChoice {
    /// Graceful termination (`SIGTERM` on Unix, `TerminateProcess` on Windows).
    Terminate,
    /// Forced termination (`SIGKILL` on Unix, `TerminateProcess` on Windows).
    Kill,
    /// Interrupt (SIGINT on Unix only).
    Interrupt,
    /// Hangup (SIGHUP on Unix only).
    Hangup,
}

impl SignalChoice {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Terminate => {
                if cfg!(windows) {
                    "terminate"
                } else {
                    "SIGTERM"
                }
            }
            Self::Kill => {
                if cfg!(windows) {
                    "terminate"
                } else {
                    "SIGKILL"
                }
            }
            Self::Interrupt => "SIGINT",
            Self::Hangup => "SIGHUP",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Terminate => {
                if cfg!(windows) {
                    "end the process (not graceful)"
                } else {
                    "ask the process to exit"
                }
            }
            Self::Kill => "force the process to exit",
            Self::Interrupt => "send an interrupt",
            Self::Hangup => "send a hangup",
        }
    }
}

/// Signals offered by the kill dialog on this platform.
#[must_use]
pub const fn signal_choices() -> &'static [SignalChoice] {
    #[cfg(windows)]
    {
        &[SignalChoice::Terminate]
    }
    #[cfg(not(windows))]
    {
        &[
            SignalChoice::Terminate,
            SignalChoice::Kill,
            SignalChoice::Interrupt,
            SignalChoice::Hangup,
        ]
    }
}

/// Send a signal to a process after verifying that it is still the process
/// the user selected.
///
/// PID reuse is a real race: the selected PID may exit and be recycled between
/// the snapshot and the confirmation. The start time and name captured in the
/// snapshot are therefore re-checked against a fresh read before anything is
/// sent. PID 1 and the monitor itself are always refused.
pub fn signal_process(
    pid: u32,
    expected_start_time: u64,
    expected_name: &str,
    choice: SignalChoice,
) -> Result<(), String> {
    if pid == 0 {
        return Err("refusing to signal PID 0".to_owned());
    }
    #[cfg(unix)]
    if pid <= 1 {
        return Err("refusing to signal PID 1".to_owned());
    }
    #[cfg(windows)]
    if pid <= 4 {
        return Err("refusing to signal a system process".to_owned());
    }
    if let Ok(current) = sysinfo::get_current_pid() {
        if current.as_u32() == pid {
            return Err("refusing to signal btop-rs itself".to_owned());
        }
    }

    let pid_value = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid_value]), true);
    let Some(process) = system.process(pid_value) else {
        return Err("the process no longer exists".to_owned());
    };
    if !process.exists() {
        return Err("the process no longer exists".to_owned());
    }
    if expected_start_time == 0 {
        return Err("cannot verify the process identity; refresh first".to_owned());
    }
    if process.start_time() != expected_start_time {
        return Err("the process was replaced by another process; refresh first".to_owned());
    }
    let current_name = process.name().to_string_lossy();
    if current_name != expected_name {
        return Err("the process name changed; refresh first".to_owned());
    }

    #[cfg(windows)]
    {
        if choice != SignalChoice::Terminate {
            return Err("only process termination is supported on Windows".to_owned());
        }
        if process.kill() {
            Ok(())
        } else {
            Err("failed to terminate the process (permission denied?)".to_owned())
        }
    }
    #[cfg(not(windows))]
    {
        let signal = match choice {
            SignalChoice::Terminate => sysinfo::Signal::Term,
            SignalChoice::Kill => sysinfo::Signal::Kill,
            SignalChoice::Interrupt => sysinfo::Signal::Interrupt,
            SignalChoice::Hangup => sysinfo::Signal::Hangup,
        };
        if !sysinfo::SUPPORTED_SIGNALS.contains(&signal) {
            return Err("this signal is not supported on this platform".to_owned());
        }
        match process.kill_with(signal) {
            Some(true) => Ok(()),
            Some(false) => Err("failed to send the signal (permission denied?)".to_owned()),
            None => Err("this signal is not supported on this platform".to_owned()),
        }
    }
}

/// Local wall clock as `HH:MM:SS`.
#[must_use]
pub fn local_clock() -> String {
    #[cfg(unix)]
    {
        // SAFETY: `time` and `localtime_r` receive valid pointers, and `tm` is
        // zero-initialised plain data owned by this function.
        unsafe {
            let now = libc::time(std::ptr::null_mut());
            if now == -1 {
                return "--:--:--".to_owned();
            }
            let mut tm: libc::tm = std::mem::zeroed();
            if libc::localtime_r(std::ptr::addr_of!(now), std::ptr::addr_of_mut!(tm)).is_null() {
                return "--:--:--".to_owned();
            }
            format!(
                "{:02}:{:02}:{:02}",
                tm.tm_hour.clamp(0, 23),
                tm.tm_min.clamp(0, 59),
                tm.tm_sec.clamp(0, 59)
            )
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::SYSTEMTIME;
        use windows_sys::Win32::System::SystemInformation::GetLocalTime;
        // SAFETY: `SYSTEMTIME` is plain data and the pointer is valid.
        unsafe {
            let mut time: SYSTEMTIME = std::mem::zeroed();
            GetLocalTime(std::ptr::addr_of_mut!(time));
            format!("{:02}:{:02}:{:02}", time.wHour, time.wMinute, time.wSecond)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        "--:--:--".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_detection_is_portable() {
        assert!(is_loopback_name("lo"));
        assert!(is_loopback_name("lo0"));
        assert!(is_loopback_name("Loopback Pseudo-Interface 1"));
        assert!(!is_loopback_name("eth0"));
        assert!(!is_loopback_name("en0"));
    }

    #[test]
    fn cpu_temperature_prefers_package() {
        let sensors = vec![
            SensorReading {
                label: "coretemp Core 0".to_owned(),
                temperature: 50.0,
                max: None,
                critical: None,
            },
            SensorReading {
                label: "coretemp Package id 0".to_owned(),
                temperature: 61.0,
                max: None,
                critical: None,
            },
        ];
        assert_eq!(pick_cpu_temperature(&sensors), Some(61.0));
        assert_eq!(pick_cpu_temperature(&[]), None);
    }

    #[test]
    fn process_state_codes_are_stable() {
        assert_eq!(ProcessState::Running.short(), 'R');
        assert_eq!(ProcessState::Zombie.short(), 'Z');
        assert_eq!(ProcessState::DiskSleep.short(), 'D');
        assert!(ProcessState::Running.is_active());
    }

    #[test]
    fn collector_produces_a_usable_snapshot() {
        let mut collector = Collector::new();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let snapshot = collector.sample();
        assert!(!snapshot.host.hostname.is_empty());
        assert!(!snapshot.cpu.cores.is_empty());
        assert!(snapshot.memory.total > 0);
        assert!(!snapshot.processes.is_empty());
        assert!(snapshot.processes.iter().all(|p| p.pid > 0));
    }

    #[test]
    fn signalling_refuses_dangerous_pids() {
        assert!(signal_process(0, 0, "none", SignalChoice::Terminate).is_err());
        #[cfg(unix)]
        assert!(signal_process(1, 0, "init", SignalChoice::Terminate).is_err());
        let current = sysinfo::get_current_pid().unwrap().as_u32();
        assert!(signal_process(current, 0, "self", SignalChoice::Terminate).is_err());
    }
}
