// SPDX-License-Identifier: GPL-2.0-or-later

//! `xmorpheo udev`, run by the udev rule for every HID device.
//!
//! The rule runs `xmorpheo udev bind <sysname>` when a driver binds to a
//! HID device, because only then the report descriptor in sysfs has the
//! fixups of the driver and the evdev nodes exist. `xmorpheo udev remove
//! <sysname>` removes the pin of a device that went away. Devices that the
//! configuration does not name return at once, without loading anything.
//!
//! Messages go to syslog, so they show up in the journal.

use std::error::Error;
use std::ffi::CString;
use std::path::Path;

use xmorpheo::config::Config;
use xmorpheo::descriptor;
use xmorpheo::loader;
use xmorpheo::plan;
use xmorpheo::sysfs::{self, HidDevice};

use crate::devices;

fn log(priority: libc::c_int, msg: &str) {
    let ident = c"xmorpheo";
    let msg = CString::new(msg.replace('\0', " ")).unwrap_or_default();
    // SAFETY: Both strings are NUL terminated, and the format consumes
    // exactly one string argument.
    unsafe {
        libc::openlog(ident.as_ptr(), libc::LOG_PID, libc::LOG_DAEMON);
        libc::syslog(priority, c"%s".as_ptr(), msg.as_ptr());
    }
}

/// What to do for a device that a driver bound to.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Not a HID device name, or no configuration entry: do nothing.
    Skip,
    Attach,
}

/// Decides from the device name alone whether the configuration may name
/// the device, before anything else is read. The unique ID is checked
/// later, when the device is opened.
pub fn decide(config: Option<&Config>, sysname: &str) -> Decision {
    let (Some(config), Some((_, vendor, product, _))) = (config, sysfs::parse_sysname(sysname))
    else {
        return Decision::Skip;
    };
    if config
        .devices
        .iter()
        .any(|d| d.vendor == vendor && d.product == product)
    {
        Decision::Attach
    } else {
        Decision::Skip
    }
}

fn bind(sysname: &str) -> Result<(), Box<dyn Error>> {
    let result = attach(sysname);
    if result.is_err() {
        // A new driver may come with a different descriptor, so a program
        // planned for the previous one must not stay.
        if let Some((_, _, _, hid_id)) = sysfs::parse_sysname(sysname) {
            match loader::detach(hid_id) {
                Ok(true) => log(
                    libc::LOG_INFO,
                    &format!("{sysname}: detached the previous program"),
                ),
                Ok(false) => {}
                Err(e) => log(
                    libc::LOG_ERR,
                    &format!("{sysname}: detaching the previous program: {e}"),
                ),
            }
        }
    }
    result
}

/// Plans the device and loads its program first, then replaces a program
/// that is already attached, so the device goes without curves only for a
/// moment.
fn attach(sysname: &str) -> Result<(), Box<dyn Error>> {
    let config = devices::read_config(None)?;
    if decide(Some(&config), sysname) == Decision::Skip {
        return Ok(());
    }
    let dev = HidDevice::open(&Path::new(sysfs::HID_DEVICES).join(sysname))?;
    let Some(entry) = config.find(dev.vendor, dev.product, &dev.uniq) else {
        return Ok(());
    };

    loader::preflight()?;
    let desc = descriptor::parse_descriptor(&dev.report_descriptor()?)?;
    let plan = plan::plan(entry, &desc)?;
    let (pin, replaced) = loader::attach(dev.hid_id, &plan, true)?;
    let axes = devices::set_evdev(&dev, entry);
    log(
        libc::LOG_INFO,
        &format!(
            "{sysname}: {} {} fields with {} knots, pinned at {}, evdev: {axes}",
            if replaced {
                "replaced, now"
            } else {
                "attached"
            },
            plan.fields.len(),
            plan.knots.len(),
            pin.display()
        ),
    );
    Ok(())
}

fn remove(sysname: &str) -> Result<(), Box<dyn Error>> {
    let Some((_, _, _, hid_id)) = sysfs::parse_sysname(sysname) else {
        return Ok(());
    };
    if loader::detach(hid_id)? {
        log(libc::LOG_INFO, &format!("{sysname}: removed the pin"));
    }
    Ok(())
}

/// Runs `xmorpheo udev <action> <sysname>`. Errors are logged, and the
/// result is the exit status for udev.
pub fn run(action: &str, sysname: &str) -> Result<(), Box<dyn Error>> {
    let result = match action {
        "bind" => bind(sysname),
        "remove" => remove(sysname),
        _ => Ok(()),
    };
    if let Err(e) = &result {
        log(libc::LOG_ERR, &format!("{sysname}: {action}: {e}"));
    }
    result
}

#[cfg(test)]
mod tests;
