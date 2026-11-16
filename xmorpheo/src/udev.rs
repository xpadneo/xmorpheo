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
    // run() logs the error itself.
    let (Err(_), Some((_, _, _, hid_id))) = (&result, sysfs::parse_sysname(sysname)) else {
        return result;
    };
    // A program planned for the same descriptor stays, for example after
    // a configuration error. A new driver may come with a different
    // descriptor, so otherwise the program must not stay.
    let current = std::fs::read(
        Path::new(sysfs::HID_DEVICES)
            .join(sysname)
            .join("report_descriptor"),
    );
    if loader::is_attached(hid_id) == Some(true)
        && keep_previous(
            loader::attached_descriptor(hid_id).as_deref(),
            current.ok().as_deref(),
        )
    {
        log(
            libc::LOG_INFO,
            &format!("{sysname}: kept the previous program, descriptor unchanged"),
        );
        return result;
    }
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
    result
}

/// Whether the previous program of a device stays after a failed bind:
/// only if the descriptor it was planned for is known and the same as the
/// current one.
fn keep_previous(planned_for: Option<&[u8]>, current: Option<&[u8]>) -> bool {
    matches!((planned_for, current), (Some(a), Some(b)) if a == b)
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
    let raw = dev.report_descriptor()?;
    let desc = descriptor::parse_descriptor(&raw)?;
    let plan = plan::plan(entry, &desc)?;
    let (pin, replaced) = loader::attach(dev.hid_id, &plan, &raw, true)?;
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
    // The rule runs for every HID device. A pin left by a device that the
    // configuration no longer names is still removed, but errors only
    // matter for devices that it names.
    let config = devices::read_config(None).ok();
    match loader::detach(hid_id) {
        Ok(true) => log(libc::LOG_INFO, &format!("{sysname}: removed the pin")),
        Ok(false) => {}
        Err(e) if configured(config.as_ref(), sysname) => return Err(e.into()),
        Err(_) => {}
    }
    Ok(())
}

/// Whether the configuration may name a device, or cannot be read.
fn configured(config: Option<&Config>, sysname: &str) -> bool {
    config.is_none_or(|c| decide(Some(c), sysname) == Decision::Attach)
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
