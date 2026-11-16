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
