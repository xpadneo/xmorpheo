// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn names() {
    assert_eq!(parse("X"), Ok(Usage::new(0x01, 0x30)));
    assert_eq!(parse("Rz"), Ok(Usage::new(0x01, 0x35)));
    assert_eq!(parse("Brake"), Ok(Usage::new(0x02, 0xc5)));
    assert_eq!(name(Usage::new(0x01, 0x36)), Some("Slider"));
    assert_eq!(name(Usage::new(0x00, 0x00)), None);
    for (n, u, _) in AXES {
        assert_eq!(parse(n), Ok(*u));
        assert_eq!(name(*u), Some(*n));
    }
}
