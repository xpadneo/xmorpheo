// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::path::PathBuf;

#[test]
fn vkb_template_parses() {
    let dev = HidDevice {
        sysname: "0003:231D:0200.000A".into(),
        path: PathBuf::new(),
        bus: 3,
        vendor: 0x231d,
        product: 0x0200,
        hid_id: 10,
        name: "VKB-Sim © Alex Oz 2021  VKBsim Gladiator EVO  R".into(),
        uniq: String::new(),
        driver: "hid-generic".into(),
    };
    let text = template(
        &dev,
        include_bytes!("../../tests/data/vkb_gladiator_evo.rdesc"),
    )
    .unwrap();
    for axis in ["X", "Y", "Rz", "Z", "Rx", "Ry"] {
        assert!(text.contains(&format!("# usage = \"{axis}\"\n")), "{axis}");
    }
    assert!(!text.contains("0001:0039"));
    assert!(!text.contains("0000:0000"));
    assert!(text.contains("# driver = \"hid-generic\""));
    // Commented out, the template is a device without axes.
    let config = Config::parse(&text).unwrap();
    assert_eq!(config.devices[0].product, 0x0200);
    assert!(config.devices[0].axes.is_empty());
    // Uncommented, every axis parses.
    let uncommented = text
        .replace("# [[device.axis]]", "[[device.axis]]")
        .replace("# usage", "usage")
        .replace("# curve", "curve");
    let config = Config::parse(&uncommented).unwrap();
    assert_eq!(config.devices[0].axes.len(), 6);
}

#[test]
fn quoting() {
    assert_eq!(quote("a \"b\" \\ c"), "\"a \\\"b\\\" \\\\ c\"");
    assert_eq!(quote("\t"), "\"\\u0009\"");
}
