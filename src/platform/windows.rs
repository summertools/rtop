//! Windows-only battery information.
//!
//! `GetSystemPowerStatus` is a tiny, side-effect-free Win32 call. Failure or
//! a desktop without a battery simply yields `None`.

use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

use super::{BatteryInfo, BatteryState};

pub fn battery() -> Option<BatteryInfo> {
    // SAFETY: `SYSTEM_POWER_STATUS` is a plain-old-data Win32 structure and
    // zeroing it is the documented way to initialise it before the call.
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    // SAFETY: we pass a valid, writable pointer; the call has no other
    // preconditions.
    let ok = unsafe { GetSystemPowerStatus(std::ptr::addr_of_mut!(status)) };
    if ok == 0 {
        return None;
    }
    // 128 means "no system battery"; 255 means "unknown percentage".
    if status.BatteryFlag == 128 || status.BatteryLifePercent == 255 {
        return None;
    }
    let percent = status.BatteryLifePercent.min(100);
    let state = if status.BatteryFlag & 8 != 0 {
        BatteryState::Charging
    } else if status.ACLineStatus == 1 {
        if percent >= 100 {
            BatteryState::Full
        } else {
            BatteryState::NotCharging
        }
    } else if status.ACLineStatus == 0 {
        BatteryState::Discharging
    } else {
        BatteryState::Unknown
    };
    let minutes_remaining = if status.BatteryLifeTime == u32::MAX {
        None
    } else {
        Some(status.BatteryLifeTime / 60)
    };
    Some(BatteryInfo {
        percent,
        state,
        minutes_remaining,
        power_w: None,
    })
}
