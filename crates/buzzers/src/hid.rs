//! The small slice of USB-HID the receiver needs, behind traits so the
//! receiver logic can run against fakes in tests.

use crate::DriverError;
use serde::{Deserialize, Serialize};

/// A HID interface as the OS lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// OS-specific path used to open this interface.
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub interface_number: i32,
}

impl DeviceInfo {
    pub fn vid_pid(&self) -> String {
        format!("{:04x}:{:04x}", self.vendor_id, self.product_id)
    }

    /// Keyboards and mice are owned by the OS and cannot be read by apps.
    pub fn is_keyboard_or_mouse(&self) -> bool {
        self.usage_page == 0x01 && matches!(self.usage, 0x02 | 0x06)
    }
}

/// Which receiver to use. Stable across unplugging and restarts, unlike the path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSelector {
    pub vendor_id: u16,
    pub product_id: u16,
    #[serde(default)]
    pub usage_page: Option<u16>,
    #[serde(default)]
    pub interface_number: Option<i32>,
}

impl DeviceSelector {
    pub fn for_device(d: &DeviceInfo) -> DeviceSelector {
        DeviceSelector {
            vendor_id: d.vendor_id,
            product_id: d.product_id,
            usage_page: Some(d.usage_page),
            interface_number: Some(d.interface_number),
        }
    }

    pub fn matches(&self, d: &DeviceInfo) -> bool {
        d.vendor_id == self.vendor_id
            && d.product_id == self.product_id
            // Some platforms report usage page 0 when unknown; do not let that block a match.
            && self.usage_page.is_none_or(|u| d.usage_page == 0 || d.usage_page == u)
            && self.interface_number.is_none_or(|i| d.interface_number == i)
    }
}

pub trait HidBackend: Send + Sync {
    fn list(&self) -> Result<Vec<DeviceInfo>, DriverError>;
    fn open(&self, path: &str) -> Result<Box<dyn HidDevice>, DriverError>;
}

/// An open device. Used from a single I/O thread only.
pub trait HidDevice: Send {
    /// Reads one input report into `buf`. Returns 0 when nothing arrived within `timeout_ms`.
    fn read_timeout(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, DriverError>;
    /// Writes one output report. The first byte is the report ID (0 if the device has none).
    fn write(&mut self, data: &[u8]) -> Result<usize, DriverError>;
}

/// udev rule that lets the logged-in user open the receiver on Linux without root.
pub fn udev_rule(vendor_id: Option<u16>, product_id: Option<u16>) -> String {
    let vid = vendor_id.map_or("VVVV".to_string(), |v| format!("{v:04x}"));
    let pid = product_id.map_or("PPPP".to_string(), |p| format!("{p:04x}"));
    format!(
        "# Save as /etc/udev/rules.d/70-quiz-show.rules, then run\n\
         #   sudo udevadm control --reload-rules && sudo udevadm trigger\n\
         # and plug the receiver in again.\n\
         KERNEL==\"hidraw*\", ATTRS{{idVendor}}==\"{vid}\", ATTRS{{idProduct}}==\"{pid}\", MODE=\"0660\", TAG+=\"uaccess\"\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(usage_page: u16, interface_number: i32) -> DeviceInfo {
        DeviceInfo {
            path: "p".into(),
            vendor_id: 0x10c4,
            product_id: 0xea80,
            manufacturer: None,
            product: None,
            serial_number: None,
            usage_page,
            usage: 1,
            interface_number,
        }
    }

    #[test]
    fn selector_matches_vid_pid_and_optional_fields() {
        let sel = DeviceSelector::for_device(&device(0xff00, 0));
        assert!(sel.matches(&device(0xff00, 0)));
        assert!(sel.matches(&device(0, 0)));
        assert!(!sel.matches(&device(0x01, 0)));
        assert!(!sel.matches(&device(0xff00, 1)));
        let loose = DeviceSelector { usage_page: None, interface_number: None, ..sel };
        assert!(loose.matches(&device(0x01, 3)));
    }

    #[test]
    fn udev_rule_uses_lowercase_hex() {
        let rule = udev_rule(Some(0x10C4), Some(0xEA80));
        assert!(rule.contains("ATTRS{idVendor}==\"10c4\""));
        assert!(rule.contains("ATTRS{idProduct}==\"ea80\""));
        assert!(udev_rule(None, None).contains("VVVV"));
    }
}
