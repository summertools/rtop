//! Linux-only details layered on top of `sysinfo`.
//!
//! These reads are cheap and optional. A missing file, a container without
//! hwmon, or a restricted `/proc` simply produces `None`; nothing here can
//! stop the sampler.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use super::{BatteryInfo, BatteryState, CpuBreakdown};

const MAX_SMALL_FILE: u64 = 4 * 1024;

/// State kept between Linux samples.
#[derive(Debug)]
pub struct Extras {
    previous_cpu: Option<CpuTimes>,
}

impl Extras {
    #[must_use]
    pub fn new() -> Self {
        Self {
            previous_cpu: read_cpu_times(),
        }
    }

    /// Delta-based CPU split. The first call after construction has a useful
    /// delta only if enough time has passed since [`Extras::new`].
    pub fn cpu_breakdown(&mut self) -> Option<CpuBreakdown> {
        let current = read_cpu_times()?;
        let previous = self.previous_cpu.replace(current);
        let previous = previous?;
        Some(previous.share(current))
    }

    /// Reclaimable page cache, in bytes.
    pub fn cached_memory() -> Option<u64> {
        let text = read_capped(Path::new("/proc/meminfo"), 64 * 1024)?;
        let mut cached = None;
        let mut reclaimable = 0u64;
        let mut shmem = 0u64;
        for line in text.lines() {
            let mut fields = line.split_whitespace();
            let Some(key) = fields.next() else { continue };
            let Some(value) = fields.next().and_then(|value| value.parse::<u64>().ok()) else {
                continue;
            };
            // /proc/meminfo values are in KiB.
            let bytes = value.saturating_mul(1024);
            match key {
                "Cached:" => cached = Some(bytes),
                "SReclaimable:" => reclaimable = bytes,
                "Shmem:" => shmem = bytes,
                _ => {}
            }
        }
        cached.map(|cached| cached.saturating_add(reclaimable).saturating_sub(shmem))
    }
}

impl Default for Extras {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CpuTimes {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

impl CpuTimes {
    fn total(self) -> u64 {
        self.user
            .saturating_add(self.nice)
            .saturating_add(self.system)
            .saturating_add(self.idle)
            .saturating_add(self.iowait)
            .saturating_add(self.irq)
            .saturating_add(self.softirq)
            .saturating_add(self.steal)
    }

    fn delta(self, previous: Self) -> Self {
        let sub = |a: u64, b: u64| a.saturating_sub(b);
        Self {
            user: sub(self.user, previous.user),
            nice: sub(self.nice, previous.nice),
            system: sub(self.system, previous.system),
            idle: sub(self.idle, previous.idle),
            iowait: sub(self.iowait, previous.iowait),
            irq: sub(self.irq, previous.irq),
            softirq: sub(self.softirq, previous.softirq),
            steal: sub(self.steal, previous.steal),
        }
    }

    fn share(self, current: Self) -> CpuBreakdown {
        let delta = current.delta(self);
        let total = delta.total();
        if total == 0 {
            return CpuBreakdown::default();
        }
        let pct = |value: u64| value as f32 * 100.0 / total as f32;
        CpuBreakdown {
            user: pct(delta.user.saturating_add(delta.nice)),
            system: pct(delta
                .system
                .saturating_add(delta.irq)
                .saturating_add(delta.softirq)),
            wait: pct(delta.iowait),
            idle: pct(delta.idle),
        }
    }
}

fn read_cpu_times() -> Option<CpuTimes> {
    let text = read_capped(Path::new("/proc/stat"), 64 * 1024)?;
    let line = text.lines().next()?;
    let mut fields = line.split_whitespace();
    if fields.next()? != "cpu" {
        return None;
    }
    let mut values = [0u64; 8];
    for (index, slot) in values.iter_mut().enumerate() {
        *slot = fields
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let _ = index;
    }
    Some(CpuTimes {
        user: values[0],
        nice: values[1],
        system: values[2],
        idle: values[3],
        iowait: values[4],
        irq: values[5],
        softirq: values[6],
        steal: values[7],
    })
}

/// Interface details from sysfs: link speed, wireless flag, and operstate.
pub fn interface_details(name: &str) -> (Option<u64>, bool, Option<bool>) {
    if !safe_component(name) {
        return (None, false, None);
    }
    let base = Path::new("/sys/class/net").join(name);
    let speed = read_capped(&base.join("speed"), MAX_SMALL_FILE)
        .and_then(|text| text.trim().parse::<u64>().ok())
        .filter(|speed| *speed > 0 && *speed < 1_000_000);
    let wireless = base.join("wireless").exists() || base.join("phy80211").exists();
    let up =
        read_capped(&base.join("operstate"), MAX_SMALL_FILE).and_then(|text| match text.trim() {
            "up" | "unknown" => Some(true),
            "down" | "lowerlayerdown" | "dormant" => Some(false),
            _ => None,
        });
    (speed, wireless, up)
}

/// Read the first usable battery from `/sys/class/power_supply`.
pub fn battery() -> Option<BatteryInfo> {
    let entries = fs::read_dir("/sys/class/power_supply").ok()?;
    let mut candidates: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            read_capped(&path.join("type"), MAX_SMALL_FILE)
                .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("Battery"))
        })
        .collect();
    candidates.sort_by(|a, b| {
        let a_primary = a
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("BAT"));
        let b_primary = b
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("BAT"));
        b_primary.cmp(&a_primary).then_with(|| a.cmp(b))
    });
    let path = candidates.first()?;

    let energy_now = read_number(&path.join("energy_now"));
    let energy_full = read_number(&path.join("energy_full"));
    let charge_now = read_number(&path.join("charge_now"));
    let charge_full = read_number(&path.join("charge_full"));
    let power_now = read_number(&path.join("power_now"));
    let current_now = read_number(&path.join("current_now"));
    let voltage_now = read_number(&path.join("voltage_now"));

    let percent = read_number(&path.join("capacity"))
        .map(|value| value.min(100) as u8)
        .or_else(|| {
            let (now, full) = match (energy_now, energy_full) {
                (Some(now), Some(full)) if full > 0 => (now, full),
                _ => (charge_now?, charge_full?),
            };
            if full == 0 {
                None
            } else {
                Some(((now as f64 * 100.0 / full as f64).round() as u8).min(100))
            }
        })?;

    let state = match read_capped(&path.join("status"), MAX_SMALL_FILE)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "charging" => BatteryState::Charging,
        "discharging" => BatteryState::Discharging,
        "full" => BatteryState::Full,
        "not charging" => BatteryState::NotCharging,
        _ => BatteryState::Unknown,
    };

    let power_w = power_now
        .map(|value| value as f32 / 1_000_000.0)
        .or_else(|| match (current_now, voltage_now) {
            (Some(current), Some(voltage)) => Some((current as f64 * voltage as f64 / 1e12) as f32),
            _ => None,
        })
        .filter(|power| power.is_finite() && *power > 0.0);

    let minutes_remaining = if state == BatteryState::Discharging {
        let hours = match (energy_now, power_now) {
            (Some(energy), Some(power)) if power > 0 => Some(energy as f64 / power as f64),
            _ => match (charge_now, current_now) {
                (Some(charge), Some(current)) if current > 0 => {
                    Some(charge as f64 / current as f64)
                }
                _ => None,
            },
        };
        hours.and_then(|hours| {
            if hours.is_finite() && (0.0..24.0 * 30.0).contains(&hours) {
                Some((hours * 60.0).round() as u32)
            } else {
                None
            }
        })
    } else {
        None
    };

    Some(BatteryInfo {
        percent,
        state,
        minutes_remaining,
        power_w,
    })
}

fn read_number(path: &Path) -> Option<u64> {
    read_capped(path, MAX_SMALL_FILE)?
        .trim()
        .parse::<u64>()
        .ok()
}

/// Read the thread count and whether this `/proc` entry is a task (thread)
/// rather than a process. `Tgid` equals the PID for a real process; a thread
/// has the thread-group leader's PID as its `Tgid`.
pub fn thread_info(pid: u32) -> (Option<u32>, bool) {
    if pid == 0 {
        return (None, false);
    }
    let path = format!("/proc/{pid}/status");
    let Some(text) = read_capped(Path::new(&path), 64 * 1024) else {
        return (None, false);
    };
    let mut threads = None;
    let mut is_thread = false;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("Threads:") {
            threads = value.trim().parse::<u32>().ok().filter(|count| *count > 0);
        } else if let Some(value) = line.strip_prefix("Tgid:") {
            is_thread = value.trim().parse::<u32>().is_ok_and(|tgid| tgid != pid);
        }
    }
    (threads, is_thread)
}

/// Reject path components that could escape a sysfs directory.
fn safe_component(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && !name.chars().any(char::is_control)
}

pub(crate) fn read_capped(path: &Path, cap: u64) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(256);
    file.take(cap).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(user: u64, system: u64, idle: u64, iowait: u64) -> CpuTimes {
        CpuTimes {
            user,
            system,
            idle,
            iowait,
            ..CpuTimes::default()
        }
    }

    #[test]
    fn cpu_share_uses_deltas() {
        let previous = times(100, 50, 800, 50);
        let current = times(150, 100, 1600, 150);
        let share = previous.share(current);
        // Delta total: 50 + 50 + 800 + 100 = 1000.
        assert!((share.user - 5.0).abs() < 0.01);
        assert!((share.system - 5.0).abs() < 0.01);
        assert!((share.wait - 10.0).abs() < 0.01);
        assert!((share.idle - 80.0).abs() < 0.01);
    }

    #[test]
    fn cpu_share_handles_no_delta() {
        let value = times(1, 2, 3, 4);
        assert_eq!(value.share(value), CpuBreakdown::default());
    }

    #[test]
    fn safe_component_rejects_traversal() {
        assert!(safe_component("eth0"));
        assert!(safe_component("enp0s1"));
        assert!(!safe_component(".."));
        assert!(!safe_component("a/b"));
        assert!(!safe_component("a\\b"));
        assert!(!safe_component("a\nb"));
    }

    #[test]
    fn battery_is_absent_or_valid() {
        if let Some(battery) = battery() {
            assert!(battery.percent <= 100);
            assert!(!battery.state.label().is_empty());
        }
    }

    #[test]
    fn thread_info_reads_the_current_process() {
        let (count, is_thread) = thread_info(std::process::id());
        assert!(count.expect("own thread count") >= 1);
        assert!(!is_thread);
        assert_eq!(thread_info(0), (None, false));
        // The test harness normally has more than one thread. If so, verify
        // that a task directory is recognised by its differing Tgid.
        let pid = std::process::id();
        if let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/task")) {
            for entry in entries.flatten().take(16) {
                let Ok(tid) = entry.file_name().to_string_lossy().parse::<u32>() else {
                    continue;
                };
                if tid != pid {
                    let (tcount, is_thread) = thread_info(tid);
                    assert!(tcount.unwrap_or(0) >= 1);
                    assert!(is_thread, "task {tid} was not recognised as a thread");
                    return;
                }
            }
        }
    }
}
