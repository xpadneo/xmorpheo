// SPDX-License-Identifier: GPL-2.0-or-later

use std::fs;
use std::path::PathBuf;
use xmorpheo::sysfs::*;

/// A sysfs tree in a temporary directory.
struct Tree(PathBuf);

impl Tree {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("xmorpheo-sysfs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn device(&self, sysname: &str, uevent: &str, events: &[&str]) -> PathBuf {
        let dir = self.0.join(sysname);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("uevent"), uevent).unwrap();
        fs::write(dir.join("report_descriptor"), [0x05, 0x01]).unwrap();
        for (i, event) in events.iter().enumerate() {
            fs::create_dir_all(dir.join(format!("input/input{i}/{event}"))).unwrap();
        }
        dir
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const EVO_R: &str = "DRIVER=hid-generic\nHID_ID=0003:0000231D:00000200\n\
    HID_NAME=VKB-Sim © Alex Oz 2021  VKBsim Gladiator EVO  R  \nHID_UNIQ=\n";

#[test]
fn sysnames() {
    assert_eq!(
        parse_sysname("0003:231D:0200.000A"),
        Some((3, 0x231d, 0x0200, 10))
    );
    assert_eq!(
        parse_sysname("0005:045E:0B13.0021"),
        Some((5, 0x045e, 0x0b13, 0x21))
    );
    assert_eq!(parse_sysname("0003:231D:0200"), None);
    assert_eq!(parse_sysname("0003:231D.000A"), None);
    assert_eq!(parse_sysname("0003:231D:0200:1.000A"), None);
    assert_eq!(parse_sysname("uevent"), None);
}

#[test]
fn devices() {
    let tree = Tree::new("devices");
    tree.device("0003:231D:0200.000A", EVO_R, &["event12"]);
    tree.device(
        "0005:045E:0B13.0002",
        "HID_NAME=Xbox Wireless Controller\nHID_UNIQ=aa:bb:cc:dd:ee:ff\n",
        &[],
    );
    fs::write(tree.0.join("not-a-device"), "").unwrap();

    let devices = list(&tree.0).unwrap();
    assert_eq!(devices.len(), 2);
    let (xbox, evo) = (&devices[0], &devices[1]);
    assert_eq!((xbox.hid_id, xbox.uniq.as_str()), (2, "aa:bb:cc:dd:ee:ff"));
    assert_eq!(evo.name, "VKB-Sim © Alex Oz 2021  VKBsim Gladiator EVO  R");
    assert_eq!((evo.vendor, evo.product, evo.hid_id), (0x231d, 0x0200, 10));
    assert_eq!(evo.uniq, "");
    assert_eq!(evo.report_descriptor().unwrap(), [0x05, 0x01]);
    assert_eq!(
        evo.event_nodes().unwrap(),
        [PathBuf::from("/dev/input/event12")]
    );
    assert_eq!(xbox.event_nodes().unwrap(), Vec::<PathBuf>::new());
}

#[test]
fn selectors() {
    let tree = Tree::new("selectors");
    let dev = HidDevice::open(&tree.device("0003:231D:0200.000A", EVO_R, &[])).unwrap();
    for sel in [
        "0003:231D:0200.000A",
        "0003:231d:0200.000a",
        "10",
        "231d:0200",
        "0x231D:0x200",
    ] {
        assert!(dev.matches(sel), "{sel}");
    }
    for sel in ["11", "231d:0201", "231d", "0003:231D:0200.000B"] {
        assert!(!dev.matches(sel), "{sel}");
    }
}
