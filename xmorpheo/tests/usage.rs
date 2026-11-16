// SPDX-License-Identifier: GPL-2.0-or-later

use xmorpheo::curve::AxisKind;
use xmorpheo::descriptor::Usage;
use xmorpheo::usage::*;

#[test]
fn numeric() {
    assert_eq!(parse("0001:0030"), Ok(Usage::new(0x01, 0x30)));
    assert_eq!(parse("0x2:0xbb"), Ok(Usage::new(0x02, 0xbb)));
    // The numeric form round-trips through Display.
    let u = Usage::new(0xff00, 0x0001);
    assert_eq!(parse(&u.to_string()), Ok(u));
}

#[test]
fn unknown() {
    assert_eq!(parse("x"), Err(UnknownUsage("x".to_string())));
    assert_eq!(parse("1:"), Err(UnknownUsage("1:".to_string())));
    assert_eq!(parse("10000:1"), Err(UnknownUsage("10000:1".to_string())));
}

#[test]
fn default_kinds() {
    assert_eq!(default_kind(Usage::new(0x01, 0x30)), AxisKind::Centered);
    assert_eq!(default_kind(Usage::new(0x02, 0xbb)), AxisKind::Unipolar);
    assert_eq!(default_kind(Usage::new(0x01, 0x36)), AxisKind::Unipolar);
    assert_eq!(default_kind(Usage::new(0xff00, 1)), AxisKind::Centered);
}
