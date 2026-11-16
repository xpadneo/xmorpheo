// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const VKB: &str = include_str!("../../../examples/vkb-gladiator-evo.toml");

#[test]
fn configured_devices_attach() {
    let config = Config::parse(VKB).unwrap();
    assert_eq!(
        decide(Some(&config), "0003:231D:0200.000A"),
        Decision::Attach
    );
    assert_eq!(
        decide(Some(&config), "0003:231D:0201.0009"),
        Decision::Attach
    );
}

#[test]
fn everything_else_is_skipped() {
    let config = Config::parse(VKB).unwrap();
    // Another device, a name that is not a HID device, no configuration.
    assert_eq!(decide(Some(&config), "0003:046D:C52B.0003"), Decision::Skip);
    assert_eq!(decide(Some(&config), "hidraw7"), Decision::Skip);
    assert_eq!(decide(None, "0003:231D:0200.000A"), Decision::Skip);
    let empty = Config::parse("").unwrap();
    assert_eq!(decide(Some(&empty), "0003:231D:0200.000A"), Decision::Skip);
}

#[test]
fn unknown_actions_do_nothing() {
    assert!(run("change", "0003:231D:0200.000A").is_ok());
    assert!(remove("not-a-device").is_ok());
}

/// After a failed bind, the previous program stays only for the same
/// descriptor it was planned for.
#[test]
fn keep_previous_program() {
    let desc = [0x05u8, 0x01, 0x09, 0x04];
    assert!(keep_previous(Some(&desc), Some(&desc)));
    assert!(!keep_previous(Some(&desc), Some(&desc[..3])));
    assert!(!keep_previous(None, Some(&desc)));
    assert!(!keep_previous(Some(&desc), None));
    assert!(!keep_previous(None, None));
}

/// Errors of removing a device only matter for devices that the
/// configuration names, or when the configuration cannot be read.
#[test]
fn remove_errors_of_configured_devices() {
    let config = Config::parse(VKB).unwrap();
    assert!(configured(Some(&config), "0003:231D:0200.000A"));
    assert!(!configured(Some(&config), "0003:046D:C52B.0003"));
    assert!(configured(None, "0003:046D:C52B.0003"));
}
