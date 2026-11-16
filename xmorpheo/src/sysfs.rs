// SPDX-License-Identifier: GPL-2.0-or-later

//! HID devices in sysfs.
//!
//! Every HID device has a directory `<bus>:<vendor>:<product>.<id>` in
//! `/sys/bus/hid/devices`, with the numbers in hexadecimal. The last number
//! is the `hid_id` that HID-BPF programs attach to.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Where the kernel lists HID devices.
pub const HID_DEVICES: &str = "/sys/bus/hid/devices";

/// A HID device as sysfs shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HidDevice {
    /// Directory name, like `0003:231D:0200.000A`.
    pub sysname: String,
    pub path: PathBuf,
    pub bus: u16,
    pub vendor: u16,
    pub product: u16,
    pub hid_id: u32,
    /// `HID_NAME` from the uevent.
    pub name: String,
    /// `HID_UNIQ` from the uevent, usually the serial number. May be empty.
    pub uniq: String,
}

/// Splits a device directory name into bus, vendor, product, and `hid_id`.
pub fn parse_sysname(name: &str) -> Option<(u16, u16, u16, u32)> {
    let (ids, id) = name.split_once('.')?;
    let mut parts = ids.split(':');
    let bus = u16::from_str_radix(parts.next()?, 16).ok()?;
    let vendor = u16::from_str_radix(parts.next()?, 16).ok()?;
    let product = u16::from_str_radix(parts.next()?, 16).ok()?;
    if parts.next().is_some() {
        return None;
    }
    let id = u32::from_str_radix(id, 16).ok()?;
    Some((bus, vendor, product, id))
}

fn uevent_value<'a>(uevent: &'a str, key: &str) -> Option<&'a str> {
    uevent
        .lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
}

impl HidDevice {
    /// Reads a device from its sysfs directory.
    pub fn open(path: &Path) -> io::Result<Self> {
        let sysname = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let (bus, vendor, product, hid_id) = parse_sysname(&sysname).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: not a HID device directory", path.display()),
            )
        })?;
        let uevent = fs::read_to_string(path.join("uevent"))?;
        Ok(Self {
            sysname,
            path: path.to_path_buf(),
            bus,
            vendor,
            product,
            hid_id,
            name: uevent_value(&uevent, "HID_NAME")
                .unwrap_or_default()
                .trim()
                .to_string(),
            uniq: uevent_value(&uevent, "HID_UNIQ")
                .unwrap_or_default()
                .to_string(),
        })
    }

    /// The report descriptor, after the fixups of the HID driver.
    pub fn report_descriptor(&self) -> io::Result<Vec<u8>> {
        fs::read(self.path.join("report_descriptor"))
    }

    /// The evdev nodes of the device, like `/dev/input/event11`.
    pub fn event_nodes(&self) -> io::Result<Vec<PathBuf>> {
        let mut nodes = Vec::new();
        let input = self.path.join("input");
        let Ok(inputs) = fs::read_dir(&input) else {
            return Ok(nodes);
        };
        for entry in inputs {
            let entry = entry?;
            for sub in fs::read_dir(entry.path())? {
                let name = sub?.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("event") {
                    nodes.push(Path::new("/dev/input").join(&*name));
                }
            }
        }
        nodes.sort();
        Ok(nodes)
    }

    /// Whether a selector from the command line names this device: its
    /// directory name, its `hid_id` in decimal, or `vendor:product` in
    /// hexadecimal.
    pub fn matches(&self, selector: &str) -> bool {
        if selector.eq_ignore_ascii_case(&self.sysname) {
            return true;
        }
        if selector.parse::<u32>().ok() == Some(self.hid_id) {
            return true;
        }
        if let Some((v, p)) = selector.split_once(':') {
            let v = u16::from_str_radix(v.trim_start_matches("0x"), 16).ok();
            let p = u16::from_str_radix(p.trim_start_matches("0x"), 16).ok();
            return v == Some(self.vendor) && p == Some(self.product);
        }
        false
    }
}

/// Lists the HID devices below `root`, usually [`HID_DEVICES`], sorted by
/// `hid_id`. Directories that are not HID devices are skipped.
pub fn list(root: &Path) -> io::Result<Vec<HidDevice>> {
    let mut devices = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if let Ok(dev) = HidDevice::open(&path) {
            devices.push(dev);
        }
    }
    devices.sort_by_key(|d| d.hid_id);
    Ok(devices)
}
