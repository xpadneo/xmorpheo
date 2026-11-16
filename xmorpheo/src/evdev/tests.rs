// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn requests_match_the_kernel_headers() {
    // From <linux/input.h> on x86_64.
    assert_eq!(size_of::<AbsInfo>(), 24);
    assert_eq!(eviocgabs(0), 0x8018_4540);
    assert_eq!(eviocsabs(0), 0x4018_45c0);
    assert_eq!(eviocgabs(5), 0x8018_4545);
    assert_eq!(eviocgbit(EV_ABS, 8), 0x8008_4523);
}

#[test]
fn names() {
    assert_eq!(abs_code("ABS_RZ"), Some(5));
    assert_eq!(abs_code("ABS_HAT0Y"), Some(0x11));
    assert_eq!(abs_code("abs_x"), None);
    assert_eq!(abs_name(0x0a), "ABS_BRAKE");
    assert_eq!(abs_name(0x30), "ABS_0x30");
    for (name, code) in ABS_NAMES {
        assert_eq!(abs_code(name), Some(*code));
    }
}
