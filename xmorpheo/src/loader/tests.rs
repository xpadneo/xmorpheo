// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn capabilities() {
    let root = "Name:\tcat\nCapInh:\t0000000000000000\nCapEff:\t000001ffffffffff\n";
    let caps = effective_caps(root);
    assert!(caps & (1 << CAP_BPF) != 0 && caps & (1 << CAP_SYS_ADMIN) != 0);
    assert_eq!(effective_caps("CapEff:\t0000000000000000\n"), 0);
    assert_eq!(effective_caps(""), 0);
}

#[test]
fn bpffs() {
    assert!(has_bpffs(
        "sysfs /sys sysfs rw 0 0\nbpf /sys/fs/bpf bpf rw,nosuid 0 0\n"
    ));
    assert!(!has_bpffs("sysfs /sys sysfs rw 0 0\n"));
    assert!(!has_bpffs("none /sys/fs/bpf tmpfs rw 0 0\n"));
}

#[test]
fn pin_paths() {
    assert_eq!(pin_path(10), PathBuf::from("/sys/fs/bpf/xmorpheo/10"));
}

#[test]
fn btf_search() {
    assert!(contains(b"x\0hid_bpf_ops\0y", b"\0hid_bpf_ops\0"));
    assert!(!contains(b"x\0hid_bpf_ops_x\0", b"\0hid_bpf_ops\0"));
}

#[test]
fn lock_is_exclusive() {
    use std::os::fd::AsRawFd;

    let dir = std::env::temp_dir().join(format!("xmorpheo-lock-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let held = lock(&dir).unwrap();
    // A second open file description cannot take the lock meanwhile.
    let other = fs::File::open(&dir).unwrap();
    let try_lock = || unsafe { libc::flock(other.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    assert_eq!(try_lock(), -1);
    drop(held);
    assert_eq!(try_lock(), 0);
    drop(other);
    fs::remove_dir(&dir).unwrap();
}

#[test]
fn already_attached_suggests_replace() {
    let msg = Error::AlreadyAttached(pin_path(7)).to_string();
    assert!(msg.contains("/sys/fs/bpf/xmorpheo/7"), "{msg}");
    assert!(msg.contains("--replace"), "{msg}");
}

/// Pins in memory, each with the program it holds, with failures to
/// inject.
#[derive(Default)]
struct FakePins {
    pinned: std::collections::BTreeMap<PathBuf, &'static str>,
    fail_pin: bool,
    fail_unpin: bool,
    fail_rename: bool,
}

impl Pins for FakePins {
    fn exists(&self, path: &Path) -> bool {
        self.pinned.contains_key(path)
    }
    fn pin_new(&mut self, path: &Path) -> io::Result<()> {
        if self.fail_pin {
            return Err(io::Error::other("pin failed"));
        }
        self.pinned.insert(path.into(), "new");
        Ok(())
    }
    fn unpin(&mut self, path: &Path) -> io::Result<()> {
        if self.fail_unpin && self.pinned.get(path) == Some(&"old") {
            return Err(io::Error::other("unpin failed"));
        }
        self.pinned.remove(path);
        Ok(())
    }
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        if self.fail_rename {
            return Err(io::Error::other("rename failed"));
        }
        let p = self.pinned.remove(from).unwrap();
        self.pinned.insert(to.into(), p);
        Ok(())
    }
}

fn with_old() -> FakePins {
    let mut p = FakePins::default();
    p.pinned.insert(pin_path(7), "old");
    p
}

fn programs(p: &FakePins) -> Vec<(&str, &str)> {
    p.pinned
        .iter()
        .map(|(k, v)| (k.file_name().unwrap().to_str().unwrap(), *v))
        .collect()
}

#[test]
fn replace_pins_the_new_program() {
    let mut p = with_old();
    install(&mut p, &pin_path(7), true).unwrap();
    assert_eq!(programs(&p), [("7", "new")]);

    let mut p = FakePins::default();
    install(&mut p, &pin_path(7), false).unwrap();
    assert_eq!(programs(&p), [("7", "new")]);
}

#[test]
fn replace_keeps_the_old_program_when_pinning_fails() {
    let mut p = with_old();
    p.fail_pin = true;
    assert!(install(&mut p, &pin_path(7), true).is_err());
    assert_eq!(programs(&p), [("7", "old")]);
}

#[test]
fn replace_keeps_the_old_program_when_unpinning_it_fails() {
    let mut p = with_old();
    p.fail_unpin = true;
    assert!(install(&mut p, &pin_path(7), true).is_err());
    assert_eq!(programs(&p), [("7", "old")]);
}

#[test]
fn replace_says_where_the_new_program_stays_when_renaming_fails() {
    let mut p = with_old();
    p.fail_rename = true;
    let err = install(&mut p, &pin_path(7), true).unwrap_err();
    assert_eq!(programs(&p), [("7.new", "new")]);
    let msg = err.to_string();
    assert!(msg.contains("/sys/fs/bpf/xmorpheo/7.new"), "{msg}");

    // The next run under the lock moves it into place.
    p.fail_rename = false;
    recover(&mut p, &pin_path(7)).unwrap();
    assert_eq!(programs(&p), [("7", "new")]);
}

#[test]
fn recover_drops_a_temporary_pin_next_to_a_program() {
    let mut p = with_old();
    p.pinned.insert(pin_path(7).with_extension("new"), "new");
    recover(&mut p, &pin_path(7)).unwrap();
    assert_eq!(programs(&p), [("7", "old")]);
}
