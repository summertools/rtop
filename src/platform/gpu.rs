//! Best-effort GPU discovery and sampling.
//!
//! Linux GPUs are read from the sysfs DRM/hwmon interfaces (AMD and Intel,
//! plus static NVIDIA information from the kernel driver). On Linux and
//! Windows, NVIDIA devices are enriched through NVML when the driver library
//! is present. Every probe is optional: a missing driver, permission error or
//! unsupported vendor simply produces no GPU entry, never a panic or a shell
//! command.

#![cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]

use std::collections::HashSet;

use super::GpuSnapshot;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use super::GpuVendor;

#[cfg(target_os = "linux")]
mod sysfs {
    use std::path::{Path, PathBuf};

    use super::super::{GpuSnapshot, GpuVendor};
    use super::{normalize_pci, pci_key};

    const MAX_GPU_FILES: u64 = 4096;
    const MAX_GPUS: usize = 16;

    pub fn sample() -> Vec<GpuSnapshot> {
        let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
            return Vec::new();
        };
        let mut cards: Vec<(u32, PathBuf)> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                let name = name.to_str()?;
                let number = name.strip_prefix("card")?;
                if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                Some((number.parse().ok()?, entry.path()))
            })
            .collect();
        cards.sort_by_key(|(number, _)| *number);
        cards.truncate(MAX_GPUS);

        let mut out = Vec::with_capacity(cards.len());
        for (number, card) in cards {
            if let Some(mut gpu) = read_card(&card) {
                gpu.index = number;
                out.push(gpu);
            }
        }
        out
    }

    fn read_card(card: &Path) -> Option<GpuSnapshot> {
        let device = card.join("device");
        let vendor_id = read_hex(&device.join("vendor"))?;
        let class = read_hex(&device.join("class")).unwrap_or(0x03_00_00);
        // Only display (0x0300xx) and 3D controller (0x0302xx) PCI classes.
        if (class >> 16) & 0xff != 0x03 {
            return None;
        }
        let vendor = match vendor_id {
            0x10de => GpuVendor::Nvidia,
            0x1002 => GpuVendor::Amd,
            0x8086 => GpuVendor::Intel,
            0x106b => GpuVendor::Apple,
            _ => GpuVendor::Other,
        };
        let pci = std::fs::read_link(&device)
            .ok()
            .and_then(|path| path.file_name().map(std::ffi::OsStr::to_owned))
            .and_then(|name| name.to_str().map(str::to_owned))
            .and_then(|name| normalize_pci(&name));
        let driver = read_uevent_value(&device.join("uevent"), "DRIVER").unwrap_or_default();
        let name = read_name(&device, vendor, pci.as_deref())
            .unwrap_or_else(|| fallback_name(vendor, pci.as_deref()));

        let utilization = read_f32(&device.join("gpu_busy_percent"))
            .filter(|value| (0.0..=100.0).contains(value));
        let memory_used = read_u64(&device.join("mem_info_vram_used"))
            .or_else(|| read_u64(&device.join("mem_info_vis_vram_used")))
            .or_else(|| read_u64(&device.join("lmem_used_bytes")));
        let memory_total = read_u64(&device.join("mem_info_vram_total"))
            .or_else(|| read_u64(&device.join("mem_info_vis_vram_total")))
            .or_else(|| read_u64(&device.join("lmem_total_bytes")))
            .filter(|total| *total > 0);
        let memory_used = memory_used
            .zip(memory_total)
            .map(|(used, total)| used.min(total))
            .or(memory_used);

        let (temperature, power_w, fan_percent) = read_hwmon(&device.join("hwmon"));
        let core_clock_mhz = read_u64(&device.join("gt_cur_freq_mhz"))
            .or_else(|| read_u64(&device.join("rps/cur_freq_mhz")))
            .or_else(|| read_dpm_clock(&device.join("pp_dpm_sclk")))
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0 && *value < 20_000);
        let memory_clock_mhz = read_dpm_clock(&device.join("pp_dpm_mclk"))
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0 && *value < 20_000);

        Some(GpuSnapshot {
            index: 0,
            name: truncate(name, 128),
            vendor,
            driver: truncate(driver, 64),
            pci,
            utilization,
            memory_used,
            memory_total,
            temperature,
            power_w,
            core_clock_mhz,
            memory_clock_mhz,
            fan_percent,
        })
    }

    fn read_name(device: &Path, vendor: GpuVendor, pci: Option<&str>) -> Option<String> {
        for file in ["product_name", "board_name", "device_name"] {
            if let Some(name) = read_text(&device.join(file)) {
                let name = name.trim();
                if !name.is_empty() {
                    return Some(name.to_owned());
                }
            }
        }
        if vendor == GpuVendor::Nvidia {
            if let Some(pci) = pci {
                if let Some(name) = nvidia_model(pci) {
                    return Some(name);
                }
            }
        }
        None
    }

    fn nvidia_model(pci: &str) -> Option<String> {
        let dir = std::fs::read_dir("/proc/driver/nvidia/gpus").ok()?;
        let wanted = pci_key(pci)?;
        for entry in dir.flatten().take(16) {
            let name = entry.file_name();
            if pci_key(name.to_str()?).as_deref() != Some(wanted.as_str()) {
                continue;
            }
            let information = entry.path().join("information");
            let text = read_text(&information)?;
            for line in text.lines() {
                if let Some(model) = line.strip_prefix("Model:") {
                    let model = model.trim();
                    if !model.is_empty() {
                        return Some(model.to_owned());
                    }
                }
            }
        }
        None
    }

    fn fallback_name(vendor: GpuVendor, pci: Option<&str>) -> String {
        let vendor = match vendor {
            GpuVendor::Nvidia => "NVIDIA",
            GpuVendor::Amd => "AMD",
            GpuVendor::Intel => "Intel",
            GpuVendor::Apple => "Apple",
            GpuVendor::Other | GpuVendor::Unknown => "GPU",
        };
        pci.map_or_else(
            || format!("{vendor} GPU"),
            |pci| format!("{vendor} GPU {pci}"),
        )
    }

    fn read_hwmon(dir: &Path) -> (Option<f32>, Option<f32>, Option<f32>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return (None, None, None);
        };
        let mut sensors: Vec<PathBuf> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                let name = name.to_str()?;
                let number = name.strip_prefix("hwmon")?;
                if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                Some(entry.path())
            })
            .collect();
        sensors.sort();
        for sensor in sensors.into_iter().take(4) {
            let temperature = read_millis(&sensor.join("temp1_input"))
                .filter(|value| (0.0..=150.0).contains(value));
            let power = read_microwatts(&sensor.join("power1_average"))
                .or_else(|| read_microwatts(&sensor.join("power1_input")))
                .filter(|value| (0.0..=5000.0).contains(value));
            let fan = read_f32(&sensor.join("pwm1"))
                .filter(|value| (0.0..=255.0).contains(value))
                .map(|value| (value * 100.0 / 255.0).clamp(0.0, 100.0));
            if temperature.is_some() || power.is_some() || fan.is_some() {
                return (temperature, power, fan);
            }
        }
        (None, None, None)
    }

    fn read_millis(path: &Path) -> Option<f32> {
        read_f32(path).map(|value| value / 1000.0)
    }

    fn read_microwatts(path: &Path) -> Option<f32> {
        read_f32(path).map(|value| value / 1_000_000.0)
    }

    fn read_dpm_clock(path: &Path) -> Option<u64> {
        let text = read_text(path)?;
        // amdgpu format: "0: 500Mhz\n1: 1200Mhz *"
        for line in text.lines().take(64) {
            let line = line.trim();
            if !line.ends_with('*') {
                continue;
            }
            let number = line
                .split_whitespace()
                .find_map(|part| part.strip_suffix("Mhz"))?;
            return number.parse().ok();
        }
        None
    }

    fn read_uevent_value(path: &Path, key: &str) -> Option<String> {
        let text = read_text(path)?;
        for line in text.lines().take(128) {
            if let Some(value) = line
                .strip_prefix(key)
                .and_then(|rest| rest.strip_prefix('='))
            {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_owned());
                }
            }
        }
        None
    }

    fn read_hex(path: &Path) -> Option<u32> {
        let value = read_text(path)?;
        let value = value.trim().trim_start_matches("0x");
        u32::from_str_radix(value, 16).ok()
    }

    fn read_f32(path: &Path) -> Option<f32> {
        let value = read_text(path)?;
        value.trim().parse::<f32>().ok().filter(|v| v.is_finite())
    }

    fn read_u64(path: &Path) -> Option<u64> {
        let value = read_text(path)?;
        value.trim().parse::<u64>().ok()
    }

    fn read_text(path: &Path) -> Option<String> {
        super::super::linux::read_capped(path, MAX_GPU_FILES)
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    fn truncate(value: String, max: usize) -> String {
        if value.chars().count() <= max {
            value
        } else {
            value.chars().take(max).collect()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn synthetic_amd_card_is_parsed_with_bounded_reads() {
            let dir = tempfile::tempdir().unwrap();
            let card = dir.path().join("card0");
            let device = card.join("device");
            let hwmon = device.join("hwmon/hwmon3");
            std::fs::create_dir_all(&hwmon).unwrap();
            std::fs::write(device.join("vendor"), "0x1002\n").unwrap();
            std::fs::write(device.join("class"), "0x030000\n").unwrap();
            std::fs::write(device.join("product_name"), "Radeon RX 7800 XT\n").unwrap();
            std::fs::write(device.join("uevent"), "DRIVER=amdgpu\n").unwrap();
            std::fs::write(device.join("gpu_busy_percent"), "37\n").unwrap();
            std::fs::write(device.join("mem_info_vram_used"), "1073741824\n").unwrap();
            std::fs::write(device.join("mem_info_vram_total"), "17179869184\n").unwrap();
            std::fs::write(device.join("pp_dpm_sclk"), "0: 500Mhz\n1: 1800Mhz *\n").unwrap();
            std::fs::write(device.join("pp_dpm_mclk"), "0: 1000Mhz *\n").unwrap();
            std::fs::write(hwmon.join("temp1_input"), "55000\n").unwrap();
            std::fs::write(hwmon.join("power1_average"), "120000000\n").unwrap();
            std::fs::write(hwmon.join("pwm1"), "128\n").unwrap();

            let gpu = read_card(&card).expect("parsed card");
            assert_eq!(gpu.vendor, GpuVendor::Amd);
            assert_eq!(gpu.name, "Radeon RX 7800 XT");
            assert_eq!(gpu.driver, "amdgpu");
            assert_eq!(gpu.utilization, Some(37.0));
            assert_eq!(gpu.memory_used, Some(1_073_741_824));
            assert_eq!(gpu.memory_total, Some(17_179_869_184));
            assert_eq!(gpu.core_clock_mhz, Some(1800));
            assert_eq!(gpu.memory_clock_mhz, Some(1000));
            assert_eq!(gpu.temperature, Some(55.0));
            assert_eq!(gpu.power_w, Some(120.0));
            assert!((gpu.fan_percent.unwrap() - 50.2).abs() < 0.1);
        }

        #[test]
        fn non_display_pci_classes_are_ignored() {
            let dir = tempfile::tempdir().unwrap();
            let card = dir.path().join("card0");
            let device = card.join("device");
            std::fs::create_dir_all(&device).unwrap();
            std::fs::write(device.join("vendor"), "0x8086\n").unwrap();
            std::fs::write(device.join("class"), "0x060000\n").unwrap();
            assert!(read_card(&card).is_none());
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
mod nvml {
    use super::super::{GpuSnapshot, GpuVendor};
    use nvml_wrapper::Nvml;
    use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};

    pub fn init() -> Option<Nvml> {
        #[cfg(windows)]
        {
            // Load the vendor library from the protected system directory
            // instead of relying on the Windows DLL search order, which can
            // include the current working directory.
            let root = std::env::var_os("SystemRoot")
                .unwrap_or_else(|| std::ffi::OsString::from("C:\\Windows"));
            let path = std::path::PathBuf::from(root)
                .join("System32")
                .join("nvml.dll");
            if !path.is_file() {
                return None;
            }
            let mut builder = Nvml::builder();
            builder.lib_path(path.as_os_str());
            builder.init().ok()
        }
        #[cfg(not(windows))]
        {
            Nvml::init().ok()
        }
    }

    pub fn sample(nvml: &Nvml) -> Vec<GpuSnapshot> {
        let count = nvml.device_count().unwrap_or(0).min(16);
        let mut out = Vec::with_capacity(count as usize);
        for index in 0..count {
            let Ok(device) = nvml.device_by_index(index) else {
                continue;
            };
            let name = device
                .name()
                .unwrap_or_else(|_| format!("NVIDIA GPU {index}"));
            let utilization = device
                .utilization_rates()
                .ok()
                .map(|usage| f32::from(u16::try_from(usage.gpu.min(100)).unwrap_or(100)));
            let memory = device.memory_info().ok();
            let temperature = device
                .temperature(TemperatureSensor::Gpu)
                .ok()
                .map(|value| f32::from(u16::try_from(value.min(200)).unwrap_or(200)));
            let power_w = device
                .power_usage()
                .ok()
                .map(|milliwatts| milliwatts as f32 / 1000.0)
                .filter(|value| (0.0..=5000.0).contains(value));
            let core_clock_mhz = device
                .clock_info(Clock::Graphics)
                .or_else(|_| device.clock_info(Clock::SM))
                .ok()
                .filter(|value| *value > 0);
            let memory_clock_mhz = device
                .clock_info(Clock::Memory)
                .ok()
                .filter(|value| *value > 0);
            let fan_percent = device
                .fan_speed(0)
                .ok()
                .map(|value| (value as f32).clamp(0.0, 100.0));
            let pci = device
                .pci_info()
                .ok()
                .and_then(|info| super::normalize_pci(&info.bus_id));
            out.push(GpuSnapshot {
                index,
                name: super::truncate(name, 128),
                vendor: GpuVendor::Nvidia,
                driver: String::new(),
                pci,
                utilization,
                memory_used: memory.as_ref().map(|info| info.used),
                memory_total: memory.as_ref().map(|info| info.total),
                temperature,
                power_w,
                core_clock_mhz,
                memory_clock_mhz,
                fan_percent,
            });
        }
        out
    }
}

/// Persistent GPU sampler. NVML is initialised once; systems without a
/// compatible driver quietly fall back to sysfs-only information.
#[derive(Debug, Default)]
pub struct GpuCollector {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    nvml: Option<nvml_wrapper::Nvml>,
}

impl GpuCollector {
    #[must_use]
    pub fn new() -> Self {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            Self { nvml: nvml::init() }
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Self {}
        }
    }

    /// Sample all discoverable GPUs. The result is deduplicated by PCI
    /// address and sorted so history stays attached to the same device.
    #[cfg_attr(
        not(any(target_os = "linux", target_os = "windows")),
        allow(clippy::unused_self)
    )]
    pub fn sample(&mut self) -> Vec<GpuSnapshot> {
        let mut gpus: Vec<GpuSnapshot> = Vec::new();
        #[cfg(target_os = "linux")]
        gpus.extend(sysfs::sample());
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        if let Some(nvml) = &self.nvml {
            merge_nvml(&mut gpus, &nvml::sample(nvml));
        }

        let mut seen = HashSet::with_capacity(gpus.len());
        gpus.retain(|gpu| {
            let key = gpu
                .pci
                .clone()
                .unwrap_or_else(|| format!("{}:{}", gpu.name, gpu.index));
            seen.insert(key)
        });
        gpus.sort_by(|a, b| {
            a.pci
                .cmp(&b.pci)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        gpus.truncate(16);
        for (index, gpu) in gpus.iter_mut().enumerate() {
            gpu.index = u32::try_from(index).unwrap_or(0);
        }
        gpus
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn merge_nvml(gpus: &mut Vec<GpuSnapshot>, fresh: &[GpuSnapshot]) {
    for nvidia in fresh {
        let existing = nvidia.pci.as_ref().and_then(|pci| {
            gpus.iter_mut()
                .find(|gpu| gpu.pci.as_deref() == Some(pci.as_str()))
        });
        if let Some(gpu) = existing {
            gpu.vendor = GpuVendor::Nvidia;
            if !nvidia.name.is_empty() {
                gpu.name.clone_from(&nvidia.name);
            }
            if !nvidia.driver.is_empty() {
                gpu.driver.clone_from(&nvidia.driver);
            }
            gpu.utilization = nvidia.utilization.or(gpu.utilization);
            gpu.memory_used = nvidia.memory_used.or(gpu.memory_used);
            gpu.memory_total = nvidia.memory_total.or(gpu.memory_total);
            gpu.temperature = nvidia.temperature.or(gpu.temperature);
            gpu.power_w = nvidia.power_w.or(gpu.power_w);
            gpu.core_clock_mhz = nvidia.core_clock_mhz.or(gpu.core_clock_mhz);
            gpu.memory_clock_mhz = nvidia.memory_clock_mhz.or(gpu.memory_clock_mhz);
            gpu.fan_percent = nvidia.fan_percent.or(gpu.fan_percent);
        } else {
            gpus.push(nvidia.clone());
        }
    }
}

/// Normalise PCI bus identifiers from sysfs (`0000:01:00.0`) and NVML
/// (`00000000:01:00.0`) to `01:00.0`, so the same physical device is merged
/// instead of appearing twice.
#[must_use]
pub fn normalize_pci(value: &str) -> Option<String> {
    let key = pci_key(value)?;
    Some(key)
}

fn pci_key(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    let mut parts = value.split(':').rev();
    let device = parts.next()?;
    let bus = parts.next()?;
    if !is_hex(bus, 2) || !is_device(device) {
        return None;
    }
    Some(format!("{bus}:{device}"))
}

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_device(value: &str) -> bool {
    let Some((slot, function)) = value.split_once('.') else {
        return false;
    };
    is_hex(slot, 2) && is_hex(function, 1)
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn truncate(value: String, max: usize) -> String {
    if value.chars().count() <= max {
        value
    } else {
        value.chars().take(max).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_identifiers_are_normalised() {
        assert_eq!(normalize_pci("0000:01:00.0").as_deref(), Some("01:00.0"));
        assert_eq!(
            normalize_pci("00000000:0A:1F.3").as_deref(),
            Some("0a:1f.3")
        );
        assert_eq!(normalize_pci("not-a-pci-id"), None);
        assert_eq!(normalize_pci(""), None);
        assert_eq!(normalize_pci("0000:01:00"), None);
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn nvml_fields_merge_into_the_matching_sysfs_device() {
        let sysfs = GpuSnapshot {
            index: 0,
            name: "NVIDIA GPU 01:00.0".to_owned(),
            vendor: GpuVendor::Nvidia,
            pci: Some("01:00.0".to_owned()),
            temperature: Some(50.0),
            ..GpuSnapshot::default()
        };
        let nvml = GpuSnapshot {
            index: 0,
            name: "NVIDIA GeForce RTX 4090".to_owned(),
            vendor: GpuVendor::Nvidia,
            pci: Some("01:00.0".to_owned()),
            utilization: Some(68.0),
            memory_used: Some(1024),
            memory_total: Some(2048),
            ..GpuSnapshot::default()
        };
        let mut gpus = vec![sysfs];
        merge_nvml(&mut gpus, &[nvml]);
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 4090");
        assert_eq!(gpus[0].utilization, Some(68.0));
        assert_eq!(gpus[0].temperature, Some(50.0));
    }

    #[test]
    fn gpu_history_keys_prefer_stable_pci_addresses() {
        let mut gpu = GpuSnapshot {
            index: 3,
            name: "Example GPU".to_owned(),
            ..GpuSnapshot::default()
        };
        assert_eq!(gpu.history_key(), "Example GPU:3");
        gpu.pci = Some("01:00.0".to_owned());
        assert_eq!(gpu.history_key(), "01:00.0");
    }

    #[test]
    fn collector_without_a_gpu_returns_an_empty_or_valid_list() {
        let mut collector = GpuCollector::new();
        let gpus = collector.sample();
        assert!(gpus.len() <= 16);
        for (index, gpu) in gpus.iter().enumerate() {
            assert_eq!(gpu.index, index as u32);
            assert!(!gpu.name.is_empty());
            if let Some(used) = gpu.memory_used {
                if let Some(total) = gpu.memory_total {
                    assert!(used <= total);
                }
            }
            if let Some(utilization) = gpu.utilization {
                assert!((0.0..=100.0).contains(&utilization));
            }
        }
    }
}
