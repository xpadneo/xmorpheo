// SPDX-License-Identifier: GPL-2.0-or-later

//! Loads, attaches, and detaches the BPF program.
//!
//! Each device gets its own instance of the program, because the
//! `struct_ops` map binds to one `hid_id`. The link is pinned to
//! `/sys/fs/bpf/xmorpheo/<hid_id>`, so the program stays attached after
//! the tool exits, until the link is unpinned or the device goes away.
//! Attaching and detaching hold an exclusive lock on the pin directory,
//! so two runs at once, like the udev rule and a manual attach, cannot
//! both attach to the same device.

use std::fmt;
use std::fs;
use std::io;
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};

use libbpf_rs::skel::{OpenSkel, SkelBuilder};
use libbpf_rs::{Link, MapCore, MapFlags};

use crate::bpf::XmorpheoSkelBuilder;
use crate::plan::Plan;

/// Where links are pinned.
pub const PIN_DIR: &str = "/sys/fs/bpf/xmorpheo";

const CAP_SYS_ADMIN: u32 = 21;
const CAP_BPF: u32 = 39;

/// Errors while loading or attaching.
#[derive(Debug)]
pub enum Error {
    MissingCapabilities,
    NoBpffs,
    NoHidBpf,
    AlreadyAttached(PathBuf),
    Io(PathBuf, io::Error),
    Bpf(&'static str, libbpf_rs::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MissingCapabilities => write!(
                f,
                "loading BPF programs needs CAP_BPF and CAP_SYS_ADMIN, run as root"
            ),
            Error::NoBpffs => write!(
                f,
                "no BPF filesystem at /sys/fs/bpf, mount it with: mount -t bpf bpf /sys/fs/bpf"
            ),
            Error::NoHidBpf => write!(
                f,
                "the kernel has no HID-BPF struct_ops (hid_bpf_ops), it needs Linux 6.11 or \
                 newer with CONFIG_HID_BPF"
            ),
            Error::AlreadyAttached(path) => {
                write!(
                    f,
                    "already attached ({}), use --replace or detach first",
                    path.display()
                )
            }
            Error::Io(path, e) if e.kind() == io::ErrorKind::PermissionDenied => {
                write!(f, "{}: {e}, run as root", path.display())
            }
            Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
            Error::Bpf(what, e) => write!(f, "{what}: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// The pin path of the link for a device.
pub fn pin_path(hid_id: u32) -> PathBuf {
    Path::new(PIN_DIR).join(hid_id.to_string())
}

/// Whether a program is attached to a device, or `None` if that cannot be
/// told without root.
pub fn is_attached(hid_id: u32) -> Option<bool> {
    match fs::metadata(pin_path(hid_id)) {
        Ok(_) => Some(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // A missing pin directory or a readable one both mean no link.
            match fs::read_dir(PIN_DIR) {
                Ok(_) => Some(false),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Some(false),
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

/// The effective capabilities from `/proc/self/status`.
fn effective_caps(status: &str) -> u64 {
    status
        .lines()
        .find_map(|l| l.strip_prefix("CapEff:"))
        .and_then(|v| u64::from_str_radix(v.trim(), 16).ok())
        .unwrap_or(0)
}

fn has_bpffs(mounts: &str) -> bool {
    mounts.lines().any(|l| {
        let mut parts = l.split_whitespace();
        let _source = parts.next();
        parts.next() == Some("/sys/fs/bpf") && parts.next() == Some("bpf")
    })
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Checks what loading needs, so failures get a clear message instead of
/// a bare errno.
pub fn preflight() -> Result<(), Error> {
    let status = fs::read_to_string("/proc/self/status").unwrap_or_default();
    let caps = effective_caps(&status);
    if caps & (1 << CAP_BPF) == 0 || caps & (1 << CAP_SYS_ADMIN) == 0 {
        return Err(Error::MissingCapabilities);
    }
    let mounts = fs::read_to_string("/proc/mounts").unwrap_or_default();
    if !has_bpffs(&mounts) {
        return Err(Error::NoBpffs);
    }
    let btf = fs::read("/sys/kernel/btf/vmlinux").unwrap_or_default();
    if !contains(&btf, b"\0hid_bpf_ops\0") {
        return Err(Error::NoHidBpf);
    }
    Ok(())
}

/// Whether a pin exists, telling a missing pin from one that cannot be
/// seen without root.
fn pinned(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Error::Io(path.to_path_buf(), e)),
    }
}

fn bytes<T: Copy>(value: &T) -> &[u8] {
    // SAFETY: The map types are plain repr(C) data without padding bytes
    // that matter to the kernel.
    unsafe { std::slice::from_raw_parts((value as *const T).cast(), size_of::<T>()) }
}

/// Writes the entries of an array map, by index.
fn fill<T: Copy>(map: &impl MapCore, entries: &[T], what: &'static str) -> Result<(), Error> {
    for (i, entry) in entries.iter().enumerate() {
        map.update(&(i as u32).to_ne_bytes(), bytes(entry), MapFlags::ANY)
            .map_err(|e| Error::Bpf(what, e))?;
    }
    Ok(())
}

/// Loads the program for a device, fills its maps, attaches it, and pins
/// the link. Returns the pin path, and whether it replaced a program.
///
/// With `replace`, a program that is already attached is replaced, else
/// that is an error. The new program is loaded first and attached before
/// the old one is detached, so the old one stays when anything fails up
/// to there. The kernel cannot swap the program of a HID-BPF link in
/// place, so for the moment between attaching the new and detaching the
/// old one, both run.
pub fn attach(hid_id: u32, plan: &Plan, replace: bool) -> Result<(PathBuf, bool), Error> {
    let path = pin_path(hid_id);
    // Fail early, before loading; checked again under the lock.
    if !replace && pinned(&path)? {
        return Err(Error::AlreadyAttached(path));
    }

    let mut object = MaybeUninit::uninit();
    let mut open = XmorpheoSkelBuilder::default()
        .open(&mut object)
        .map_err(|e| Error::Bpf("opening the BPF object", e))?;
    let rodata = open
        .maps
        .rodata_data
        .as_deref_mut()
        .expect("the BPF object has read-only data");
    rodata.nr_fields = plan.fields.len() as u32;
    rodata.nr_outputs = plan.outputs.len() as u32;
    open.struct_ops.xmorpheo_mut().hid_id = hid_id as i32;

    let skel = open
        .load()
        .map_err(|e| Error::Bpf("loading the BPF program", e))?;
    fill(&skel.maps.fields, &plan.fields, "filling the field map")?;
    fill(&skel.maps.outputs, &plan.outputs, "filling the output map")?;
    fill(&skel.maps.terms, &plan.terms, "filling the term map")?;
    fill(&skel.maps.curves, &plan.curves, "filling the curve map")?;
    fill(&skel.maps.knots, &plan.knots, "filling the knot map")?;
    let mut skel = skel;

    fs::create_dir_all(PIN_DIR).map_err(|e| Error::Io(PIN_DIR.into(), e))?;
    let _lock = lock(Path::new(PIN_DIR))?;
    let old = pinned(&path)?;
    if old && !replace {
        return Err(Error::AlreadyAttached(path));
    }
    let mut link = skel
        .maps
        .xmorpheo
        .attach_struct_ops()
        .map_err(|e| Error::Bpf("attaching to the HID device", e))?;
    if old {
        unpin(&path)?;
    }
    link.pin(&path)
        .map_err(|e| Error::Bpf("pinning the link", e))?;
    Ok((path, old))
}

/// Detaches the program from a device and removes the pin. Returns false
/// if nothing was attached.
pub fn detach(hid_id: u32) -> Result<bool, Error> {
    let path = pin_path(hid_id);
    if !pinned(&path)? {
        return Ok(false);
    }
    let _lock = lock(Path::new(PIN_DIR))?;
    // Another run may have detached it while this one waited.
    if !pinned(&path)? {
        return Ok(false);
    }
    unpin(&path)?;
    Ok(true)
}

/// Detaches the link pinned at `path` and removes the pin.
fn unpin(path: &Path) -> Result<(), Error> {
    // Detach explicitly where the kernel supports it. Otherwise removing
    // the last reference, the pin, releases the link.
    if let Ok(link) = Link::open(path) {
        let _ = link.detach();
    }
    fs::remove_file(path).map_err(|e| Error::Io(path.into(), e))
}

/// Takes an exclusive lock on a directory, waiting for other holders.
/// The lock lasts until the returned file is dropped.
fn lock(dir: &Path) -> Result<fs::File, Error> {
    use std::os::fd::AsRawFd;

    let file = fs::File::open(dir).map_err(|e| Error::Io(dir.into(), e))?;
    loop {
        // SAFETY: flock only takes the descriptor of the open file.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
            return Ok(file);
        }
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(Error::Io(dir.into(), e));
        }
    }
}

#[cfg(test)]
mod tests;
