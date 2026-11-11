//! Safe hardware model labels shared by the header and the CPU/GPU panels.
//!
//! Model strings come from firmware, sysfs, or NVML, so they are sanitised
//! before they reach a width calculation or the terminal. An empty or
//! whitespace-only name falls back to a stable vendor/PCI description instead
//! of leaving a blank card.

use std::fmt::Write as _;

use crate::format;
use crate::platform::{GpuSnapshot, GpuVendor, HostInfo};

/// Longest model string kept before layout-level truncation. Real hardware
/// names are far shorter; this simply bounds work on malformed input.
const MAX_MODEL_CHARS: usize = 192;

/// Return a cleaned CPU model, or `None` when the platform did not expose one.
#[must_use]
pub(crate) fn cpu_model(host: &HostInfo) -> Option<String> {
    let model = host.cpu_model.trim();
    if model.is_empty() {
        return None;
    }
    let model = format::sanitize_limited(model, MAX_MODEL_CHARS);
    if model.trim().is_empty() {
        None
    } else {
        Some(model)
    }
}

/// Return a cleaned GPU model name with a vendor/PCI fallback.
#[must_use]
pub(crate) fn gpu_name(gpu: &GpuSnapshot) -> String {
    let name = gpu.name.trim();
    if !name.is_empty() {
        let name = format::sanitize_limited(name, MAX_MODEL_CHARS);
        if !name.trim().is_empty() {
            return name;
        }
    }
    let vendor = match gpu.vendor {
        GpuVendor::Unknown | GpuVendor::Other => "GPU".to_owned(),
        vendor => format!("{} GPU", vendor.label()),
    };
    if let Some(pci) = gpu_pci(gpu) {
        return format!("{vendor} {pci}");
    }
    format!("{vendor} {}", gpu.index)
}

/// Return a compact "pci 01:00.0" location label when one is available.
#[must_use]
pub(crate) fn gpu_location(gpu: &GpuSnapshot) -> Option<String> {
    gpu_pci(gpu).map(|pci| format!("pci {pci}"))
}

/// One-line summary for the header: the first device plus a count of any
/// additional devices.
#[must_use]
pub(crate) fn gpu_summary(gpus: &[GpuSnapshot]) -> Option<String> {
    let first = gpus.first()?;
    let mut name = gpu_name(first);
    if gpus.len() > 1 {
        let _ = write!(name, " +{}", gpus.len() - 1);
    }
    Some(name)
}

fn gpu_pci(gpu: &GpuSnapshot) -> Option<String> {
    let pci = gpu.pci.as_deref()?.trim();
    if pci.is_empty() {
        return None;
    }
    let pci = format::sanitize_limited(pci, MAX_MODEL_CHARS);
    if pci.trim().is_empty() {
        None
    } else {
        Some(pci)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(name: &str, pci: Option<&str>) -> GpuSnapshot {
        GpuSnapshot {
            index: 2,
            name: name.to_owned(),
            vendor: GpuVendor::Nvidia,
            pci: pci.map(str::to_owned),
            ..GpuSnapshot::default()
        }
    }

    #[test]
    fn cpu_model_is_sanitised_and_missing_models_are_hidden() {
        let mut host = HostInfo {
            cpu_model: "  Example CPU\u{1b}[31m\u{202e} ".to_owned(),
            ..HostInfo::default()
        };
        let model = cpu_model(&host).expect("model");
        assert!(!model.contains('\u{1b}'));
        assert!(!model.contains('\u{202e}'));
        assert!(model.starts_with("Example CPU"));

        host.cpu_model = " \t\n ".to_owned();
        assert!(cpu_model(&host).is_none());
    }

    #[test]
    fn gpu_names_fall_back_to_pci_then_index() {
        let named = gpu("  RTX 4070\u{1b}[31m ", Some("01:00.0"));
        let name = gpu_name(&named);
        assert!(name.starts_with("RTX 4070"));
        assert!(!name.contains('\u{1b}'));

        let unnamed = gpu("   ", Some("01:00.0"));
        assert_eq!(gpu_name(&unnamed), "nvidia GPU 01:00.0");
        assert_eq!(gpu_location(&unnamed).as_deref(), Some("pci 01:00.0"));

        let no_pci = gpu("\t", None);
        assert_eq!(gpu_name(&no_pci), "nvidia GPU 2");
        assert!(gpu_location(&no_pci).is_none());

        let mut unknown = gpu(" ", Some("0b:00.0"));
        unknown.vendor = GpuVendor::Unknown;
        assert_eq!(gpu_name(&unknown), "GPU 0b:00.0");
    }

    #[test]
    fn gpu_summary_counts_extra_devices() {
        let first = gpu("RTX 4070", Some("01:00.0"));
        let second = gpu("RTX 4070", Some("02:00.0"));
        assert_eq!(
            gpu_summary(std::slice::from_ref(&first)).as_deref(),
            Some("RTX 4070")
        );
        assert_eq!(
            gpu_summary(&[first, second]).as_deref(),
            Some("RTX 4070 +1")
        );
    }
}
