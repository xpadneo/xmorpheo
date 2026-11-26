// SPDX-License-Identifier: GPL-2.0-or-later

//! Configuration drop-ins, like systemd.
//!
//! The configuration is made of `*.toml` files in several `conf.d`
//! directories, from the highest priority to the lowest:
//!
//! 1. `/etc/xmorpheo/conf.d`, for the administrator, or below the
//!    `SYSCONFDIR` of the build,
//! 2. `/run/xmorpheo/conf.d`, for runtime files,
//! 3. `<libdir>/conf.d`, with the `LIBDIR` of the build,
//! 4. `<prefix>/lib/xmorpheo/conf.d`, next to the installed binary,
//! 5. `/usr/lib/xmorpheo/conf.d`, for packages.
//!
//! The build takes `SYSCONFDIR` and `LIBDIR` from the environment variables
//! `XMORPHEO_SYSCONFDIR` and `XMORPHEO_LIBDIR`, which the `Makefile` sets.
//!
//! A file replaces every file of the same name with a lower priority. An
//! empty file or a symlink to `/dev/null` masks them. The remaining files
//! are read in the lexical order of their names, each with its own
//! constants and tolerance, and merged into one configuration.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::{self, Config, Device};

/// `SYSCONFDIR` of the build, `/etc` by default.
pub const SYSCONFDIR: &str = match option_env!("XMORPHEO_SYSCONFDIR") {
    Some(dir) => dir,
    None => "/etc",
};

/// `LIBDIR` of the build, the `xmorpheo` directory below the library
/// directory of the installation, if the build set one.
pub const LIBDIR: Option<&str> = option_env!("XMORPHEO_LIBDIR");

/// The drop-in directories, from the highest priority to the lowest:
/// `SYSCONFDIR`, `/run`, `LIBDIR` of the build, the `lib` directory next to
/// the binary, and `/usr/lib`. Duplicates are left out.
pub fn dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        Path::new(SYSCONFDIR).join("xmorpheo/conf.d"),
        PathBuf::from("/run/xmorpheo/conf.d"),
    ];
    if let Some(lib) = LIBDIR {
        dirs.push(Path::new(lib).join("conf.d"));
    }
    // <prefix>/bin/xmorpheo has its drop-ins in <prefix>/lib.
    if let Some(prefix) = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.parent()?.to_path_buf()))
    {
        dirs.push(prefix.join("lib/xmorpheo/conf.d"));
    }
    dirs.push(PathBuf::from("/usr/lib/xmorpheo/conf.d"));
    let mut unique: Vec<PathBuf> = Vec::new();
    for dir in dirs {
        if !unique.contains(&dir) {
            unique.push(dir);
        }
    }
    unique
}

/// A drop-in file and what became of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// The file is read.
    Active(PathBuf),
    /// The file is replaced by one of the same name with a higher priority.
    Overridden { path: PathBuf, by: PathBuf },
    /// The file, and all of the same name with a lower priority, is masked
    /// by an empty file or a symlink to `/dev/null`.
    Masked { path: PathBuf, by: PathBuf },
}

/// Finds the drop-in files in `dirs`, from the highest priority to the
/// lowest. Missing directories are skipped. The result is sorted by file
/// name, and for each name by priority.
pub fn scan(dirs: &[PathBuf]) -> io::Result<Vec<Entry>> {
    // For each name: the file that decides, and whether it masks.
    let mut names: Vec<(String, PathBuf, bool)> = Vec::new();
    let mut entries = Vec::new();
    for dir in dirs {
        let list = match fs::read_dir(dir) {
            Ok(list) => list,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(io::Error::new(e.kind(), format!("{}: {e}", dir.display()))),
        };
        let mut files: Vec<(String, PathBuf)> = Vec::new();
        for entry in list {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.ends_with(".toml") && !name.starts_with('.') {
                files.push((name.to_string(), path));
            }
        }
        files.sort();
        for (name, path) in files {
            match names.iter().find(|(n, _, _)| *n == name) {
                Some((_, by, true)) => entries.push(Entry::Masked {
                    path,
                    by: by.clone(),
                }),
                Some((_, by, false)) => entries.push(Entry::Overridden {
                    path,
                    by: by.clone(),
                }),
                None if masks(&path) => {
                    entries.push(Entry::Masked {
                        path: path.clone(),
                        by: path.clone(),
                    });
                    names.push((name, path, true));
                }
                None => {
                    entries.push(Entry::Active(path.clone()));
                    names.push((name, path, false));
                }
            }
        }
    }
    entries.sort_by(|a, b| {
        let key = |e: &Entry| match e {
            Entry::Active(p)
            | Entry::Overridden { path: p, .. }
            | Entry::Masked { path: p, .. } => p.file_name().map(|n| n.to_os_string()),
        };
        key(a).cmp(&key(b))
    });
    Ok(entries)
}

/// Whether a file masks others: it is empty, or a symlink to `/dev/null`.
fn masks(path: &Path) -> bool {
    if fs::read_link(path).is_ok_and(|target| target == Path::new("/dev/null")) {
        return true;
    }
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == 0)
}

/// Errors while reading drop-ins.
#[derive(Debug)]
pub enum Error {
    Io(PathBuf, io::Error),
    Config(PathBuf, config::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
            Error::Config(path, e) => write!(f, "{}: {e}", path.display()),
        }
    }
}

impl std::error::Error for Error {}

/// Reads one configuration file, and marks its devices with their origin.
pub fn read_file(path: &Path) -> Result<Config, Error> {
    let text = fs::read_to_string(path).map_err(|e| Error::Io(path.to_path_buf(), e))?;
    let mut config = Config::parse(&text).map_err(|e| Error::Config(path.to_path_buf(), e))?;
    for dev in &mut config.devices {
        dev.origin = Some(path.to_path_buf());
    }
    for w in &mut config.warnings {
        *w = format!("{}: {w}", path.display());
    }
    Ok(config)
}

/// Reads and merges the active drop-ins of `entries`, in their order.
pub fn read(entries: &[Entry]) -> Result<Config, Error> {
    let mut merged = Config::default();
    for entry in entries {
        if let Entry::Active(path) = entry {
            let config = read_file(path)?;
            merged.warnings.extend(config.warnings);
            for dev in config.devices {
                merge_device(&mut merged, dev);
            }
        }
    }
    Ok(merged)
}

/// Adds a device entry to a merged configuration. An entry for the same
/// device, by vendor, product, unique ID, and driver, from an earlier file
/// is replaced as a whole, with a warning.
pub fn merge_device(merged: &mut Config, dev: Device) {
    let same = |d: &Device| {
        (d.vendor, d.product, &d.uniq, &d.driver)
            == (dev.vendor, dev.product, &dev.uniq, &dev.driver)
    };
    if let Some(old) = merged.devices.iter_mut().find(|d| same(d)) {
        let origin = |d: &Device| {
            d.origin
                .as_ref()
                .map_or("?".to_string(), |p| p.display().to_string())
        };
        merged.warnings.push(format!(
            "device {:04x}:{:04x} of {} is replaced by {}",
            dev.vendor,
            dev.product,
            origin(old),
            origin(&dev)
        ));
        *old = dev;
    } else {
        merged.devices.push(dev);
    }
}

/// Reads the configuration from a file, or from the drop-ins of one
/// directory, or, without a path, from all drop-in directories.
pub fn load(path: Option<&Path>) -> Result<(Config, Vec<Entry>), Error> {
    let dirs = match path {
        Some(p) if p.is_dir() => vec![p.to_path_buf()],
        Some(p) => {
            let config = read_file(p)?;
            return Ok((config, vec![Entry::Active(p.to_path_buf())]));
        }
        None => dirs(),
    };
    let entries = scan(&dirs).map_err(|e| Error::Io(PathBuf::new(), e))?;
    Ok((read(&entries)?, entries))
}
