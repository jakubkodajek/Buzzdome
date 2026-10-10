//! `HidBackend` on top of the `hidapi` crate (C hidapi: Windows HID API, macOS
//! IOKit, Linux hidraw). No driver installation is needed on any of them.

use crate::hid::{DeviceInfo, HidBackend, HidDevice};
use crate::DriverError;
use std::ffi::CString;
use std::sync::Mutex;

pub struct HidapiBackend {
    api: Mutex<hidapi::HidApi>,
}

fn err(e: hidapi::HidError) -> DriverError {
    DriverError::Device(e.to_string())
}

impl HidapiBackend {
    pub fn new() -> Result<HidapiBackend, DriverError> {
        let api = hidapi::HidApi::new().map_err(err)?;
        // macOS opens devices exclusively by default; non-exclusive lets the OS
        // keep a keyboard-class receiver and asks the user for Input Monitoring.
        #[cfg(target_os = "macos")]
        api.set_open_exclusive(false);
        Ok(HidapiBackend { api: Mutex::new(api) })
    }
}

impl HidBackend for HidapiBackend {
    fn list(&self) -> Result<Vec<DeviceInfo>, DriverError> {
        let mut api = self.api.lock().unwrap();
        api.refresh_devices().map_err(err)?;
        Ok(api
            .device_list()
            .map(|d| DeviceInfo {
                path: d.path().to_string_lossy().into_owned(),
                vendor_id: d.vendor_id(),
                product_id: d.product_id(),
                manufacturer: d.manufacturer_string().filter(|s| !s.is_empty()).map(str::to_owned),
                product: d.product_string().filter(|s| !s.is_empty()).map(str::to_owned),
                serial_number: d.serial_number().filter(|s| !s.is_empty()).map(str::to_owned),
                usage_page: d.usage_page(),
                usage: d.usage(),
                interface_number: d.interface_number(),
            })
            .collect())
    }

    fn open(&self, path: &str) -> Result<Box<dyn HidDevice>, DriverError> {
        let path = CString::new(path).map_err(|e| DriverError::Device(e.to_string()))?;
        let device = self.api.lock().unwrap().open_path(&path).map_err(err)?;
        Ok(Box::new(Device(device)))
    }
}

struct Device(hidapi::HidDevice);

impl HidDevice for Device {
    fn read_timeout(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, DriverError> {
        self.0.read_timeout(buf, timeout_ms).map_err(err)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, DriverError> {
        self.0.write(data).map_err(err)
    }
}
