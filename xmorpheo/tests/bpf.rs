// SPDX-License-Identifier: GPL-2.0-or-later

use std::mem::{offset_of, size_of};
use xmorpheo::bpf::types::{xm_curve, xm_field, xm_knot, xm_output, xm_term};
use xmorpheo::bpf::*;
use xmorpheo::config::Config;
use xmorpheo::curve;
use xmorpheo::descriptor::parse_descriptor;
use xmorpheo::plan::{Plan, plan};

#[repr(C)]
struct HostTables {
    fields: *const xm_field,
    nr_fields: u32,
    outputs: *const xm_output,
    nr_outputs: u32,
    terms: *const xm_term,
    nr_terms: u32,
    curves: *const xm_curve,
    nr_curves: u32,
    knots: *const xm_knot,
    nr_knots: u32,
    report: *mut u8,
    report_len: u32,
}

unsafe extern "C" {
    static xm_host_layout: [usize; 26];
    static xm_host_limits: [u32; 10];
    fn xm_host_field_get(f: *const xm_field, report: *const u8) -> i32;
    fn xm_host_field_put(f: *const xm_field, report: *mut u8, value: i32);
    fn xm_host_rewrite(tables: *const HostTables);
}

/// Rewrites a report with the C code of the BPF program.
fn c_rewrite(plan: &Plan, report: &mut [u8]) {
    let tables = HostTables {
        fields: plan.fields.as_ptr(),
        nr_fields: plan.fields.len() as u32,
        outputs: plan.outputs.as_ptr(),
        nr_outputs: plan.outputs.len() as u32,
        terms: plan.terms.as_ptr(),
        nr_terms: plan.terms.len() as u32,
        curves: plan.curves.as_ptr(),
        nr_curves: plan.curves.len() as u32,
        knots: plan.knots.as_ptr(),
        nr_knots: plan.knots.len() as u32,
        report: report.as_mut_ptr(),
        report_len: report.len() as u32,
    };
    unsafe { xm_host_rewrite(&tables) };
}

#[test]
fn layout_matches_the_c_header() {
    let rust = [
        size_of::<xm_field>(),
        offset_of!(xm_field, min),
        offset_of!(xm_field, max),
        offset_of!(xm_field, bit_offset),
        offset_of!(xm_field, report_len),
        offset_of!(xm_field, report_id),
        offset_of!(xm_field, bit_size),
        offset_of!(xm_field, is_signed),
        size_of::<xm_output>(),
        offset_of!(xm_output, out_mul),
        offset_of!(xm_output, field),
        offset_of!(xm_output, first_term),
        offset_of!(xm_output, nr_terms),
        offset_of!(xm_output, centered),
        size_of::<xm_term>(),
        offset_of!(xm_term, in_mul),
        offset_of!(xm_term, field),
        offset_of!(xm_term, curve),
        offset_of!(xm_term, centered),
        size_of::<xm_curve>(),
        offset_of!(xm_curve, first),
        offset_of!(xm_curve, count),
        size_of::<xm_knot>(),
        offset_of!(xm_knot, x),
        offset_of!(xm_knot, y),
        offset_of!(xm_knot, slope),
    ];
    assert_eq!(rust, unsafe { xm_host_layout });
}

#[test]
fn limits_match_the_c_header() {
    let rust = [
        MAX_FIELDS as u32,
        MAX_OUTPUTS as u32,
        MAX_TERMS as u32,
        OUTPUT_TERMS as u32,
        MAX_CURVES as u32,
        MAX_KNOTS as u32,
        curve::MAX_KNOTS as u32,
        curve::SEARCH_STEPS,
        MAX_BITS,
        curve::FRAC,
    ];
    assert_eq!(rust, unsafe { xm_host_limits });
    assert_eq!(curve::MAX_SPAN, (1 << MAX_BITS) - 1);
}

/// The compiler must keep the rewrite. Folding the zero initializer of
/// a read-only field table once left a program of 32 instructions that
/// only walked the table.
#[test]
fn program_keeps_the_rewrite() {
    use libbpf_rs::skel::SkelBuilder;
    let mut object = std::mem::MaybeUninit::uninit();
    let open = XmorpheoSkelBuilder::default().open(&mut object).unwrap();
    let insns = open.progs.xmorpheo_event.insns();
    // BPF_JMP | BPF_CALL, for helpers, kfuncs, and subprograms.
    let calls = insns.iter().filter(|i| i.code == 0x85).count();
    assert!(insns.len() > 300, "{} instructions", insns.len());
    // hid_bpf_get_data for the report ID and the field windows, and
    // lookups in all five maps.
    assert!(calls >= 10, "{calls} calls");
}

#[test]
fn c_field_access_matches_the_rust_reference() {
    // A deterministic pseudo-random report.
    let mut seed = 0x1234_5678u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let report: Vec<u8> = (0..12).map(|_| next() as u8).collect();
    for size in 1..=16u32 {
        for offset in 0..(12 * 8 - size - 16) {
            for signed in [false, true] {
                let f = xm_field {
                    bit_offset: offset,
                    bit_size: size as u8,
                    is_signed: signed as u8,
                    ..Default::default()
                };
                let got = unsafe { xm_host_field_get(&f, report.as_ptr()) };
                assert_eq!(Some(got), get(&f, &report), "{offset}+{size}");

                let value = next() as i32;
                let mut c = report.clone();
                let mut rust = report.clone();
                unsafe { xm_host_field_put(&f, c.as_mut_ptr(), value) };
                put(&f, &mut rust, value);
                assert_eq!(c, rust, "{offset}+{size}");
            }
        }
    }
}

/// Reports with every value of every planned source field, and pseudo
/// random other bits, rewritten by the C code and the Rust reference.
fn same_as_rust(plan: &Plan, len: usize, report_id: u8) {
    let mut seed = 0x9e37_79b9u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for round in 0..2000 {
        let mut report: Vec<u8> = (0..len).map(|_| next() as u8).collect();
        report[0] = report_id;
        // Sweep the first field through its range, plus a bit beyond.
        let f = &plan.fields[0];
        let span = (f.max - f.min + 5) as u32;
        put(f, &mut report, f.min - 2 + (round % span) as i32);
        let mut c = report.clone();
        let mut rust = report.clone();
        c_rewrite(plan, &mut c);
        plan.rewrite(&mut rust);
        assert_eq!(c, rust, "round {round}, report {report:02x?}");
    }
}

#[test]
fn c_rewrite_matches_rust_for_the_vkb_example() {
    let config = Config::parse(include_str!("../../examples/vkb-gladiator-evo.toml")).unwrap();
    let desc = parse_descriptor(include_bytes!("data/vkb_gladiator_evo.rdesc")).unwrap();
    let plan = plan(&config.devices[0], &desc).unwrap();
    same_as_rust(&plan, 64, 1);
}

#[test]
fn c_rewrite_of_the_combined_triggers_example() {
    let config = Config::parse(include_str!("../../examples/xbox-combined-triggers.toml")).unwrap();
    let desc = parse_descriptor(include_bytes!("data/xbox_series_xs_ble_xpadneo.rdesc")).unwrap();
    let plan = plan(&config.devices[0], &desc).unwrap();
    same_as_rust(&plan, 17, 1);
    // Corners of the triggers: Z at bit 72, Rz at bit 88.
    let z = xm_field {
        bit_offset: 72,
        bit_size: 10,
        ..Default::default()
    };
    let rz = xm_field {
        bit_offset: 88,
        bit_size: 10,
        ..Default::default()
    };
    for (l, r, want) in [
        (0, 0, 512),
        (1023, 0, 0),
        (0, 1023, 1023),
        (1023, 1023, 512),
    ] {
        let mut report = vec![0u8; 17];
        report[0] = 1;
        // Sticks at rest.
        for offset in [1, 3, 5, 7] {
            report[offset] = 0x00;
            report[offset + 1] = 0x80;
        }
        put(&z, &mut report, l);
        put(&rz, &mut report, r);
        let mut rust = report.clone();
        c_rewrite(&plan, &mut report);
        plan.rewrite(&mut rust);
        assert_eq!(report, rust);
        assert_eq!(
            (get(&z, &report), get(&rz, &report)),
            (Some(want), Some(0)),
            "l {l} r {r}"
        );
    }
}

#[test]
fn c_rewrite_matches_rust_for_combined_triggers() {
    let config = Config::parse(
        r#"
        [[device]]
        vendor = 0x045e
        product = 0x0b13

        [[device.axis]]
        usage = "Z"
        kind = "centered"
        source.l = { usage = "Z", kind = "unipolar" }
        source.r = { usage = "Rz", kind = "unipolar" }
        curve = "r - l"

        [[device.axis]]
        usage = "Rz"
        curve = "0"
        kind = "unipolar"

        [[device.axis]]
        usage = "X"
        curve = "pow(abs(x), 1.5) * sign(x)"
        "#,
    )
    .unwrap();
    let desc = parse_descriptor(include_bytes!("data/xbox_series_xs_ble_xpadneo.rdesc")).unwrap();
    let plan = plan(&config.devices[0], &desc).unwrap();
    same_as_rust(&plan, 17, 1);
    // A shorter report stays untouched.
    let mut report = vec![1u8; 16];
    let copy = report.clone();
    c_rewrite(&plan, &mut report);
    assert_eq!(report, copy);
    // A longer report is rewritten like the HID core reads it.
    same_as_rust(&plan, 20, 1);
}

#[test]
fn c_rewrite_swaps_the_t300rs_pedals_exactly() {
    let config = Config::parse(include_str!("../../examples/t300rs-swap-pedals.toml")).unwrap();
    let desc = parse_descriptor(include_bytes!("data/t300rs_pc_mode.rdesc")).unwrap();
    let plan = plan(&config.devices[0], &desc).unwrap();
    let len = desc.input_report_len(7).unwrap() as usize;
    same_as_rust(&plan, len, 7);
    // The wheel pads its reports to the 64 bytes of its endpoint.
    same_as_rust(&plan, 64, 7);
    let brake = xm_field {
        bit_offset: 24,
        bit_size: 16,
        ..Default::default()
    };
    let gas = xm_field {
        bit_offset: 40,
        bit_size: 16,
        ..Default::default()
    };
    let clutch = xm_field {
        bit_offset: 56,
        bit_size: 16,
        ..Default::default()
    };
    for (b, c, len) in [
        (0, 0, len),
        (1023, 0, len),
        (0, 1023, len),
        (300, 700, len),
        (1, 1022, len),
        (512, 511, len),
        (1023, 0, 64),
        (300, 700, 64),
    ] {
        let mut report = vec![0u8; len];
        report[0] = 7;
        put(&brake, &mut report, b);
        put(&gas, &mut report, 400);
        put(&clutch, &mut report, c);
        c_rewrite(&plan, &mut report);
        assert_eq!(
            (
                get(&brake, &report),
                get(&gas, &report),
                get(&clutch, &report)
            ),
            (Some(c), Some(400), Some(b)),
            "brake {b} clutch {c} len {len}"
        );
    }
}
