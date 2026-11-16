// SPDX-License-Identifier: GPL-2.0-or-later

//! The fuzz and flat of evdev axes.
//!
//! hid-input sets fuzz to `range >> 8` and flat to `range >> 4` for
//! joystick axes. The input core then smooths small changes, which costs
//! modern sensors resolution around the center. For managed devices,
//! xmorpheo sets the fuzz and flat to 0, or to configured values.

use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;

const EV_ABS: u32 = 0x03;
const ABS_CNT: usize = 0x40;

/// `struct input_absinfo`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

const fn ioc(dir: u32, nr: u32, size: usize) -> libc::c_ulong {
    ((dir << 30) | ((size as u32) << 16) | ((b'E' as u32) << 8) | nr) as libc::c_ulong
}

const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

fn eviocgbit(ev: u32, len: usize) -> libc::c_ulong {
    ioc(IOC_READ, 0x20 + ev, len)
}

fn eviocgabs(abs: u32) -> libc::c_ulong {
    ioc(IOC_READ, 0x40 + abs, size_of::<AbsInfo>())
}

fn eviocsabs(abs: u32) -> libc::c_ulong {
    ioc(IOC_WRITE, 0xc0 + abs, size_of::<AbsInfo>())
}

fn check(ret: libc::c_int) -> io::Result<()> {
    if ret < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// The absolute axes of an evdev node and their current settings.
pub fn axes(node: &Path) -> io::Result<Vec<(u16, AbsInfo)>> {
    let file = File::open(node)?;
    let fd = file.as_raw_fd();
    let mut bits = [0u8; ABS_CNT / 8];
    // SAFETY: The buffer has the size passed in the request.
    check(unsafe { libc::ioctl(fd, eviocgbit(EV_ABS, bits.len()), bits.as_mut_ptr()) })?;
    let mut axes = Vec::new();
    for code in 0..ABS_CNT {
        if bits[code / 8] & (1 << (code % 8)) == 0 {
            continue;
        }
        let mut info = AbsInfo::default();
        // SAFETY: AbsInfo has the layout of struct input_absinfo.
        check(unsafe { libc::ioctl(fd, eviocgabs(code as u32), &mut info) })?;
        axes.push((code as u16, info));
    }
    Ok(axes)
}

/// The names of absolute axes, as in `<linux/input-event-codes.h>`.
const ABS_NAMES: &[(&str, u16)] = &[
    ("ABS_X", 0x00),
    ("ABS_Y", 0x01),
    ("ABS_Z", 0x02),
    ("ABS_RX", 0x03),
    ("ABS_RY", 0x04),
    ("ABS_RZ", 0x05),
    ("ABS_THROTTLE", 0x06),
    ("ABS_RUDDER", 0x07),
    ("ABS_WHEEL", 0x08),
    ("ABS_GAS", 0x09),
    ("ABS_BRAKE", 0x0a),
    ("ABS_HAT0X", 0x10),
    ("ABS_HAT0Y", 0x11),
    ("ABS_HAT1X", 0x12),
    ("ABS_HAT1Y", 0x13),
    ("ABS_HAT2X", 0x14),
    ("ABS_HAT2Y", 0x15),
    ("ABS_HAT3X", 0x16),
    ("ABS_HAT3Y", 0x17),
    ("ABS_PRESSURE", 0x18),
    ("ABS_DISTANCE", 0x19),
    ("ABS_TILT_X", 0x1a),
    ("ABS_TILT_Y", 0x1b),
    ("ABS_TOOL_WIDTH", 0x1c),
    ("ABS_VOLUME", 0x20),
    ("ABS_PROFILE", 0x21),
    ("ABS_MISC", 0x28),
];

/// The code of an absolute axis by name, like `ABS_X`.
pub fn abs_code(name: &str) -> Option<u16> {
    ABS_NAMES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// The name of an absolute axis, or its code in hexadecimal.
pub fn abs_name(code: u16) -> String {
    ABS_NAMES
        .iter()
        .find(|(_, c)| *c == code)
        .map_or(format!("ABS_{code:#04x}"), |(n, _)| n.to_string())
}

/// Sets the fuzz and flat of every absolute axis of an evdev node to the
/// values `settings` returns for its code. Returns the codes of the axes
/// and the number of axes changed.
pub fn set_fuzz_and_flat(
    node: &Path,
    settings: impl Fn(u16) -> (i32, i32),
) -> io::Result<(Vec<u16>, usize)> {
    let file = File::open(node)?;
    let fd = file.as_raw_fd();
    let mut codes = Vec::new();
    let mut changed = 0;
    for (code, mut info) in axes(node)? {
        codes.push(code);
        let (fuzz, flat) = settings(code);
        if info.fuzz == fuzz && info.flat == flat {
            continue;
        }
        info.fuzz = fuzz;
        info.flat = flat;
        // SAFETY: AbsInfo has the layout of struct input_absinfo.
        check(unsafe { libc::ioctl(fd, eviocsabs(code as u32), &info) })?;
        changed += 1;
    }
    Ok((codes, changed))
}

#[cfg(test)]
mod tests;
