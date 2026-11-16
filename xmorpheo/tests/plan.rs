// SPDX-License-Identifier: GPL-2.0-or-later

use xmorpheo::config::Config;
use xmorpheo::descriptor::Usage;
use xmorpheo::descriptor::parse_descriptor;
use xmorpheo::plan::*;
use xmorpheo::{bpf, usage};

const VKB_EVO: &[u8] = include_bytes!("data/vkb_gladiator_evo.rdesc");
const XBOX: &[u8] = include_bytes!("data/xbox_series_xs_ble_xpadneo.rdesc");

fn vkb_plan(config: &str) -> Result<Plan, Error> {
    let config = Config::parse(config).unwrap();
    plan(&config.devices[0], &parse_descriptor(VKB_EVO).unwrap())
}

fn summary(plan: &Plan) -> Vec<(&'static str, u32, i32, Vec<usize>)> {
    plan.axes
        .iter()
        .map(|a| {
            (
                usage::name(a.usage).unwrap(),
                a.bit_offset,
                a.max,
                a.terms.iter().map(|t| t.curve_index).collect(),
            )
        })
        .collect()
}

#[test]
fn vkb_example() {
    let config = Config::parse(include_str!("../../examples/vkb-gladiator-evo.toml")).unwrap();
    let desc = parse_descriptor(VKB_EVO).unwrap();
    for device in &config.devices {
        let plan = plan(device, &desc).unwrap();
        // X and Y share the stick curve, the twist has its own.
        assert_eq!(
            summary(&plan),
            [
                ("X", 8, 4095, vec![0]),
                ("Y", 24, 4095, vec![0]),
                ("Rz", 40, 2047, vec![1])
            ]
        );
        assert_eq!(plan.curves.len(), 2);
        assert_eq!(plan.curves[1].first, plan.curves[0].count);
        assert_eq!(
            plan.knots.len() as u32,
            plan.curves[0].count + plan.curves[1].count
        );
        for f in &plan.fields {
            assert_eq!((f.report_id, f.report_len, f.bit_size), (1, 64, 16));
        }
        // Each output reads its own field.
        for (i, o) in plan.outputs.iter().enumerate() {
            assert_eq!((o.field as usize, o.nr_terms, o.centered), (i, 1, 1));
            assert_eq!(plan.terms[o.first_term as usize].field as usize, i);
        }
    }
}

#[test]
fn xbox_example() {
    let config =
        Config::parse(include_str!("../../examples/xbox-wireless-controller.toml")).unwrap();
    let plan = plan(&config.devices[0], &parse_descriptor(XBOX).unwrap()).unwrap();
    // All four stick axes share one curve.
    assert_eq!(
        summary(&plan),
        [
            ("X", 8, 65535, vec![0]),
            ("Y", 24, 65535, vec![0]),
            ("Rx", 40, 65535, vec![0]),
            ("Ry", 56, 65535, vec![0])
        ]
    );
    assert_eq!(plan.curves.len(), 1);
    assert!(plan.knots.len() < 512, "{} knots", plan.knots.len());
}

const COMBINED: &str = r#"
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
    kind = "unipolar"
    curve = "0"
"#;

fn combined() -> Plan {
    let config = Config::parse(COMBINED).unwrap();
    plan(&config.devices[0], &parse_descriptor(XBOX).unwrap()).unwrap()
}

#[test]
fn combined_triggers() {
    let plan = combined();
    // Z and Rz are read once each, Z is written from both.
    assert_eq!(plan.fields.len(), 2);
    assert_eq!(
        (plan.fields[0].bit_offset, plan.fields[1].bit_offset),
        (72, 88)
    );
    assert_eq!(plan.outputs.len(), 2);
    assert_eq!((plan.outputs[0].field, plan.outputs[0].nr_terms), (0, 2));
    let z = &plan.axes[0];
    let terms: Vec<_> = z
        .terms
        .iter()
        .map(|t| (t.var.as_str(), t.curve.as_str()))
        .collect();
    assert_eq!(terms, [("l", "(-x)"), ("r", "x")]);
    assert_eq!(z.formula, "(r-l)");
}

#[test]
fn combined_triggers_rewrite() {
    let plan = combined();
    // Report 1 of the Xbox controller: Z at bit 72, Rz at bit 88.
    let report = |l: i32, r: i32| {
        let mut report = vec![0u8; 17];
        report[0] = 1;
        bpf::put(&plan.fields[0], &mut report, l);
        bpf::put(&plan.fields[1], &mut report, r);
        report
    };
    for (l, r, z) in [
        (0, 0, 512),
        (1023, 0, 0),
        (0, 1023, 1023),
        (1023, 1023, 512),
    ] {
        let mut rep = report(l, r);
        plan.rewrite(&mut rep);
        let got = (
            bpf::get(&plan.fields[0], &rep),
            bpf::get(&plan.fields[1], &rep),
        );
        assert_eq!(got, (Some(z), Some(0)), "l {l} r {r}");
    }
}

#[test]
fn xbox_combined_triggers_example() {
    let config = Config::parse(include_str!("../../examples/xbox-combined-triggers.toml")).unwrap();
    assert!(config.warnings.is_empty());
    let plan = plan(&config.devices[0], &parse_descriptor(XBOX).unwrap()).unwrap();
    let names: Vec<_> = plan
        .axes
        .iter()
        .map(|a| usage::name(a.usage).unwrap())
        .collect();
    assert_eq!(names, ["X", "Y", "Rx", "Ry", "Z", "Rz"]);
    // The sticks share one curve, the triggers have three small ones.
    assert_eq!(plan.curves.len(), 4);
    assert_eq!(plan.axes[4].terms.len(), 2);
}

#[test]
fn source_in_another_report() {
    let config = Config::parse(
        "[[device]]\nvendor = 1\nproduct = 2\n[[device.axis]]\nusage = \"X\"\n\
         source.a = \"Brake\"\ncurve = \"a\"\n",
    )
    .unwrap();
    assert_eq!(
        plan(&config.devices[0], &parse_descriptor(VKB_EVO).unwrap()).unwrap_err(),
        Error::SourceNotInReport {
            usage: Usage::new(0x01, 0x30),
            source: Usage::new(0x02, 0xc5)
        }
    );
}

#[test]
fn missing_axis() {
    let err = vkb_plan(
        "[[device]]\nvendor = 1\nproduct = 2\n[[device.axis]]\nusage = \"Brake\"\ncurve = \"x\"\n",
    );
    assert_eq!(
        err.unwrap_err(),
        Error::AxisNotFound(Usage::new(0x02, 0xc5))
    );
}

#[test]
fn small_fields_fit() {
    // The hat switch has 4 bits and 8 steps.
    let plan = vkb_plan(
        "[[device]]\nvendor = 1\nproduct = 2\n[[device.axis]]\nusage = \"0001:0039\"\ncurve = \"x\"\nkind = \"unipolar\"\n",
    )
    .unwrap();
    assert_eq!((plan.fields[0].bit_size, plan.fields[0].max), (4, 7));
    assert_eq!(plan.curves[0].count, 2);
}

#[test]
fn t300rs_swap_pedals_example() {
    let config = Config::parse(include_str!("../../examples/t300rs-swap-pedals.toml")).unwrap();
    assert!(config.warnings.is_empty());
    let desc = parse_descriptor(include_bytes!("data/t300rs_pc_mode.rdesc")).unwrap();
    let plan = plan(&config.devices[0], &desc).unwrap();
    // Rz and Y each read the other field, Z is not touched.
    assert_eq!(plan.fields.len(), 2);
    let offsets: Vec<_> = plan
        .fields
        .iter()
        .map(|f| (f.report_id, f.bit_offset))
        .collect();
    assert_eq!(offsets, [(7, 24), (7, 56)]);
    assert_eq!(
        plan.outputs
            .iter()
            .map(|o| plan.terms[o.first_term as usize].field)
            .collect::<Vec<_>>(),
        [1, 0]
    );
}

#[test]
fn too_many_knots_names_the_tolerance_of_the_axis() {
    let config = Config::parse(
        r#"
        [[device]]
        vendor = 0x045e
        product = 0x0b13

        [[device.axis]]
        usage = "X"
        tolerance = 0.02
        source.x = "X"
        source.y = "Y"
        curve = "pow(x, 3) + pow(y, 3)"
        "#,
    )
    .unwrap();
    let err = plan(&config.devices[0], &parse_descriptor(XBOX).unwrap()).unwrap_err();
    let Error::Curve {
        error:
            xmorpheo::curve::Error::TooManyKnots {
                tolerance,
                suggestion: Some(t),
                ..
            },
        ..
    } = &err
    else {
        panic!("{err:?}");
    };
    // Each of the two terms got half of the tolerance of the axis.
    assert_eq!(*tolerance, 0.02);
    assert!(*t > 0.02);
    assert!(
        err.to_string()
            .starts_with("axis X: the curve pow(x,3.0) needs"),
        "{err}"
    );
}
