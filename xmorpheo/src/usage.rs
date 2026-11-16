// SPDX-License-Identifier: GPL-2.0-or-later

//! Names of the HID usages of axes.
//!
//! The configuration selects axes by these names, or by a numeric usage
//! `page:id` in hexadecimal, as [`Usage`] prints it.

use std::fmt;

use crate::curve::AxisKind;
use crate::descriptor::Usage;

const GENERIC_DESKTOP: u16 = 0x01;
const SIMULATION: u16 = 0x02;

/// Known axis usages, their names, and how they rest by default.
const AXES: &[(&str, Usage, AxisKind)] = &[
    ("X", Usage::new(GENERIC_DESKTOP, 0x30), AxisKind::Centered),
    ("Y", Usage::new(GENERIC_DESKTOP, 0x31), AxisKind::Centered),
    ("Z", Usage::new(GENERIC_DESKTOP, 0x32), AxisKind::Centered),
    ("Rx", Usage::new(GENERIC_DESKTOP, 0x33), AxisKind::Centered),
    ("Ry", Usage::new(GENERIC_DESKTOP, 0x34), AxisKind::Centered),
    ("Rz", Usage::new(GENERIC_DESKTOP, 0x35), AxisKind::Centered),
    (
        "Slider",
        Usage::new(GENERIC_DESKTOP, 0x36),
        AxisKind::Unipolar,
    ),
    (
        "Dial",
        Usage::new(GENERIC_DESKTOP, 0x37),
        AxisKind::Unipolar,
    ),
    (
        "Wheel",
        Usage::new(GENERIC_DESKTOP, 0x38),
        AxisKind::Centered,
    ),
    ("Rudder", Usage::new(SIMULATION, 0xba), AxisKind::Centered),
    ("Throttle", Usage::new(SIMULATION, 0xbb), AxisKind::Unipolar),
    (
        "Accelerator",
        Usage::new(SIMULATION, 0xc4),
        AxisKind::Unipolar,
    ),
    ("Brake", Usage::new(SIMULATION, 0xc5), AxisKind::Unipolar),
    ("Clutch", Usage::new(SIMULATION, 0xc6), AxisKind::Unipolar),
    ("Steering", Usage::new(SIMULATION, 0xc8), AxisKind::Centered),
];

/// A usage name that is neither known nor a numeric `page:id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownUsage(pub String);

impl fmt::Display for UnknownUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown usage {:?}", self.0)
    }
}

impl std::error::Error for UnknownUsage {}

/// Looks up a usage by name, or parses a numeric `page:id` in hexadecimal.
pub fn parse(name: &str) -> Result<Usage, UnknownUsage> {
    if let Some(&(_, usage, _)) = AXES.iter().find(|(n, _, _)| *n == name) {
        return Ok(usage);
    }
    let numeric = name.split_once(':').and_then(|(page, id)| {
        let page = u16::from_str_radix(page.trim_start_matches("0x"), 16).ok()?;
        let id = u16::from_str_radix(id.trim_start_matches("0x"), 16).ok()?;
        Some(Usage::new(page, id))
    });
    numeric.ok_or_else(|| UnknownUsage(name.to_string()))
}

/// The name of a known axis usage.
pub fn name(usage: Usage) -> Option<&'static str> {
    AXES.iter()
        .find(|(_, u, _)| *u == usage)
        .map(|(n, _, _)| *n)
}

/// How an axis with this usage rests by default. Unknown usages are
/// centered.
pub fn default_kind(usage: Usage) -> AxisKind {
    AXES.iter()
        .find(|(_, u, _)| *u == usage)
        .map_or(AxisKind::Centered, |(_, _, k)| *k)
}

#[cfg(test)]
mod tests;
