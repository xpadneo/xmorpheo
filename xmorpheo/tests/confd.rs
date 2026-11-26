// SPDX-License-Identifier: GPL-2.0-or-later

use std::fs;
use std::path::{Path, PathBuf};

use xmorpheo::confd::{self, Entry};

/// Drop-in directories in a temporary directory, from the highest priority
/// to the lowest.
struct Dirs {
    root: PathBuf,
    dirs: Vec<PathBuf>,
}

impl Dirs {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("xmorpheo-confd-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let dirs: Vec<PathBuf> = ["etc", "run", "local", "usr"]
            .iter()
            .map(|d| root.join(d))
            .collect();
        for d in &dirs {
            fs::create_dir_all(d).unwrap();
        }
        Self { root, dirs }
    }

    fn write(&self, dir: usize, name: &str, text: &str) -> PathBuf {
        let path = self.dirs[dir].join(name);
        fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn device(product: u16, k: f64) -> String {
    format!(
        "[constants]\nk = {k}\n[[device]]\nvendor = 0x231d\nproduct = {product:#06x}\n\
         [[device.axis]]\nusage = \"X\"\ncurve = \"(1 - k) * x + k * pow(x, 3)\"\n"
    )
}

#[test]
fn files_are_merged_in_name_order() {
    let d = Dirs::new("merge");
    d.write(3, "50-r.toml", &device(0x0200, 0.6));
    d.write(2, "10-l.toml", &device(0x0201, 0.5));
    let entries = confd::scan(&d.dirs).unwrap();
    let config = confd::read(&entries).unwrap();
    let products: Vec<_> = config.devices.iter().map(|d| d.product).collect();
    assert_eq!(products, [0x0201, 0x0200]);
    // Constants stay local to their file.
    let half = |i: usize| config.devices[i].axes[0].curve.eval(&[0.5]);
    assert!((half(0) - 0.3125).abs() < 1e-9);
    assert!((half(1) - 0.275).abs() < 1e-9);
    assert_eq!(
        config.devices[0].origin.as_deref(),
        Some(d.dirs[2].join("10-l.toml").as_path())
    );
    assert!(config.warnings.is_empty());
}

#[test]
fn higher_priority_replaces_files_of_the_same_name() {
    let d = Dirs::new("override");
    let usr = d.write(3, "50-vkb.toml", &device(0x0200, 0.6));
    let etc = d.write(0, "50-vkb.toml", &device(0x0200, 0.75));
    let entries = confd::scan(&d.dirs).unwrap();
    assert_eq!(
        entries,
        [
            Entry::Active(etc.clone()),
            Entry::Overridden { path: usr, by: etc }
        ]
    );
    let config = confd::read(&entries).unwrap();
    assert_eq!(config.devices.len(), 1);
    assert!((config.devices[0].axes[0].curve.eval(&[0.5]) - 0.21875).abs() < 1e-9);
}

#[test]
fn empty_files_and_dev_null_mask() {
    let d = Dirs::new("mask");
    let usr = d.write(3, "50-vkb.toml", &device(0x0200, 0.6));
    let run = d.write(1, "50-vkb.toml", "");
    let local = d.write(2, "60-other.toml", &device(0x0201, 0.6));
    let etc = d.dirs[0].join("60-other.toml");
    std::os::unix::fs::symlink("/dev/null", &etc).unwrap();
    let entries = confd::scan(&d.dirs).unwrap();
    assert_eq!(
        entries,
        [
            Entry::Masked {
                path: run.clone(),
                by: run.clone()
            },
            Entry::Masked { path: usr, by: run },
            Entry::Masked {
                path: etc.clone(),
                by: etc.clone()
            },
            Entry::Masked {
                path: local,
                by: etc
            },
        ]
    );
    assert!(confd::read(&entries).unwrap().devices.is_empty());
}

#[test]
fn the_same_device_twice_takes_the_later_file() {
    let d = Dirs::new("conflict");
    d.write(3, "10-a.toml", &device(0x0200, 0.6));
    let later = d.write(0, "90-b.toml", &device(0x0200, 0.75));
    let config = confd::read(&confd::scan(&d.dirs).unwrap()).unwrap();
    assert_eq!(config.devices.len(), 1);
    assert_eq!(config.devices[0].origin.as_deref(), Some(later.as_path()));
    assert_eq!(config.warnings.len(), 1);
    assert!(
        config.warnings[0].contains("231d:0200"),
        "{}",
        config.warnings[0]
    );
}

/// Entries for different drivers of the same device are kept side by side.
#[test]
fn entries_for_other_drivers_stay() {
    let d = Dirs::new("drivers");
    d.write(3, "10-a.toml", &device(0x0200, 0.6));
    let tmff2 = device(0x0200, 0.75).replace(
        "product = 0x0200\n",
        "product = 0x0200\ndriver = \"tmff2\"\n",
    );
    d.write(0, "90-b.toml", &tmff2);
    let config = confd::read(&confd::scan(&d.dirs).unwrap()).unwrap();
    assert_eq!(config.devices.len(), 2);
    assert!(config.warnings.is_empty(), "{:?}", config.warnings);
}

#[test]
fn other_files_are_ignored() {
    let d = Dirs::new("ignore");
    d.write(0, "README", "not toml");
    d.write(0, ".50-hidden.toml", "not toml");
    d.write(0, "50-vkb.toml.bak", "not toml");
    assert!(confd::scan(&d.dirs).unwrap().is_empty());
}

#[test]
fn errors_name_the_file() {
    let d = Dirs::new("errors");
    let bad = d.write(0, "50-bad.toml", "[[device]]\nvendor = 1\n");
    let err = confd::read(&confd::scan(&d.dirs).unwrap()).unwrap_err();
    assert!(
        err.to_string().starts_with(&bad.display().to_string()),
        "{err}"
    );
}

#[test]
fn load_reads_a_file_or_one_directory() {
    let d = Dirs::new("load");
    let file = d.write(1, "50-vkb.toml", &device(0x0200, 0.6));
    d.write(0, "50-vkb.toml", "");
    // A file is read alone, without drop-ins.
    let (config, _) = confd::load(Some(&file)).unwrap();
    assert_eq!(config.devices.len(), 1);
    // A directory is read alone, without the other directories.
    let (config, entries) = confd::load(Some(&d.dirs[0])).unwrap();
    assert!(config.devices.is_empty());
    assert_eq!(entries.len(), 1);
    let (config, _) = confd::load(Some(&d.dirs[1])).unwrap();
    assert_eq!(config.devices.len(), 1);
}

#[test]
fn missing_directories_are_empty() {
    let missing = Path::new("/nonexistent/xmorpheo/conf.d").to_path_buf();
    assert!(confd::scan(&[missing]).unwrap().is_empty());
}

#[test]
fn default_directories() {
    let dirs = confd::dirs();
    assert_eq!(dirs[0], Path::new("/etc/xmorpheo/conf.d"));
    assert_eq!(dirs[1], Path::new("/run/xmorpheo/conf.d"));
    assert_eq!(dirs.last().unwrap(), Path::new("/usr/lib/xmorpheo/conf.d"));
}
