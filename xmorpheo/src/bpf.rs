// SPDX-License-Identifier: GPL-2.0-or-later

//! The skeleton of the BPF program, and the conversion of fields, outputs,
//! terms, and curves into its map layout.

#[allow(clippy::all, dead_code, non_camel_case_types, unused_imports)]
mod skel {
    include!(concat!(env!("OUT_DIR"), "/xmorpheo.skel.rs"));
}

pub use skel::*;

use crate::curve::{self, AxisKind, Knot};
use crate::descriptor;

/// Most fields per device, read or written.
pub const MAX_FIELDS: usize = 16;
/// Most outputs per device.
pub const MAX_OUTPUTS: usize = 16;
/// Most terms per device.
pub const MAX_TERMS: usize = 32;
/// Most terms per output.
pub const OUTPUT_TERMS: usize = 4;
/// Most curves on the system.
pub const MAX_CURVES: usize = 64;
/// Size of the knot pool shared by all curves.
pub const MAX_KNOTS: usize = 8192;
/// Largest field, in bits.
pub const MAX_BITS: u32 = 16;

/// The field table entry for a field of the report descriptor.
pub fn field(f: &descriptor::Field, report_len: u16) -> types::xm_field {
    types::xm_field {
        min: f.logical_min,
        max: f.logical_max,
        bit_offset: f.bit_offset,
        report_len,
        report_id: f.report_id,
        bit_size: f.bit_size as u8,
        is_signed: f.is_signed() as u8,
        ..Default::default()
    }
}

/// The output table entry for a field to write.
pub fn output(
    field: u16,
    target: &curve::Field,
    first_term: u16,
    nr_terms: u8,
) -> types::xm_output {
    types::xm_output {
        out_mul: target.out_mul,
        field,
        first_term,
        nr_terms,
        centered: (target.kind == AxisKind::Centered) as u8,
        ..Default::default()
    }
}

/// The term table entry for a curve over a source field.
pub fn term(field: u16, source: &curve::Field, curve: u16) -> types::xm_term {
    types::xm_term {
        in_mul: source.in_mul,
        field,
        curve,
        centered: (source.kind == AxisKind::Centered) as u8,
        ..Default::default()
    }
}

/// The knot map entry for a knot.
pub fn knot(k: &Knot) -> types::xm_knot {
    types::xm_knot {
        x: k.x,
        y: k.y,
        slope: k.slope,
    }
}

/// Reads a field from a report bit by bit, like the BPF program.
pub fn get(f: &types::xm_field, report: &[u8]) -> Option<i32> {
    let size = f.bit_size as u32;
    let end = f.bit_offset + size;
    if size == 0 || size > MAX_BITS || end.div_ceil(8) > report.len() as u32 {
        return None;
    }
    let mut v: u32 = 0;
    for i in 0..size {
        let bit = f.bit_offset + i;
        if report[(bit / 8) as usize] & (1 << (bit % 8)) != 0 {
            v |= 1 << i;
        }
    }
    Some(if f.is_signed != 0 && v & (1 << (size - 1)) != 0 {
        (v | !((1u32 << size) - 1)) as i32
    } else {
        v as i32
    })
}

/// Writes a field into a report bit by bit, keeping the other bits.
pub fn put(f: &types::xm_field, report: &mut [u8], value: i32) {
    for i in 0..f.bit_size as u32 {
        let bit = f.bit_offset + i;
        let (byte, mask) = ((bit / 8) as usize, 1u8 << (bit % 8));
        if (value as u32) & (1 << i) != 0 {
            report[byte] |= mask;
        } else {
            report[byte] &= !mask;
        }
    }
}
