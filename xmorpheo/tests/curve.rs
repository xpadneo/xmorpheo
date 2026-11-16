// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use xmorpheo::curve::*;
use xmorpheo::formula::Formula;

fn formula(src: &str) -> Formula {
    Formula::parse(src, &HashMap::new()).unwrap()
}

/// Fits the formula and checks every raw value against the exact
/// formula. Returns the knots.
fn check(src: &str, kind: AxisKind, min: i32, max: i32) -> Vec<Knot> {
    let f = formula(src);
    let field = Field::new(kind, min, max).unwrap();
    let knots = fit(&f, &field, DEFAULT_TOLERANCE).unwrap();
    let lowest = kind.lowest() as f64 / ONE as f64;
    for raw in min..=max {
        let y = f.eval(field.exact_x(raw)).clamp(lowest, 1.0);
        let exact = field.exact_raw(y);
        let got = eval_raw(&field, &knots, raw);
        assert!(
            (got as f64 - exact).abs() <= 1.0,
            "{src} at {raw}: got {got}, exact {exact}"
        );
        assert!((min..=max).contains(&got));
    }
    println!("{src} on {min}..={max}: {} knots", knots.len());
    knots
}

#[test]
fn identity() {
    for (kind, min, max) in [
        (AxisKind::Centered, 0, 4095),
        (AxisKind::Centered, -32768, 32767),
        (AxisKind::Unipolar, 0, 1023),
        (AxisKind::Unipolar, 0, 65535),
    ] {
        let knots = check("x", kind, min, max);
        assert_eq!(knots.len(), 2, "{kind:?} {min}..={max}");
        let field = Field::new(kind, min, max).unwrap();
        for raw in min..=max {
            assert_eq!(eval_raw(&field, &knots, raw), raw);
        }
    }
}

#[test]
fn expo_12_bit() {
    let knots = check("0.4 * x + 0.6 * pow(x, 3)", AxisKind::Centered, 0, 4095);
    assert!(knots.len() < 128, "{} knots", knots.len());
}

#[test]
fn expo_11_bit() {
    let knots = check("0.25 * x + 0.75 * pow(x, 3)", AxisKind::Centered, 0, 2047);
    assert!(knots.len() < 128, "{} knots", knots.len());
}

#[test]
fn power_with_deadzone_signed_16_bit() {
    check(
        "pow(normalize(abs(x), 0.05), 1.7) * sign(x)",
        AxisKind::Centered,
        -32768,
        32767,
    );
}

#[test]
fn unipolar_power_10_bit() {
    let knots = check("pow(x, 2)", AxisKind::Unipolar, 0, 1023);
    assert!(knots.len() < 64, "{} knots", knots.len());
}

#[test]
fn centered_curve_is_nearly_symmetric() {
    let f = formula("0.4 * x + 0.6 * pow(x, 3)");
    let field = Field::new(AxisKind::Centered, -127, 127).unwrap();
    let knots = fit(&f, &field, DEFAULT_TOLERANCE).unwrap();
    // Rounding may differ by one step between the two directions.
    for raw in 0..=127 {
        let (pos, neg) = (
            eval_raw(&field, &knots, raw),
            eval_raw(&field, &knots, -raw),
        );
        assert!((pos + neg).abs() <= 1, "at {raw}: {pos} and {neg}");
    }
    assert_eq!(eval_raw(&field, &knots, 0), 0);
}

#[test]
fn deadzone_maps_to_center() {
    let knots = check("deadzone(x, 0.1)", AxisKind::Centered, 0, 65535);
    let field = Field::new(AxisKind::Centered, 0, 65535).unwrap();
    // The exact center is 32767.5, so both neighbors are within the
    // bound everywhere in the deadzone.
    for raw in [32768, 32768 + 3000, 32767 - 3000] {
        let rest = eval_raw(&field, &knots, raw);
        assert!(rest == 32767 || rest == 32768, "at {raw}: {rest}");
    }
    assert_eq!(eval_raw(&field, &knots, 0), 0);
    assert_eq!(eval_raw(&field, &knots, 65535), 65535);
}

#[test]
fn outside_the_range_stays_unchanged() {
    let knots = check("x", AxisKind::Unipolar, 10, 20);
    let field = Field::new(AxisKind::Unipolar, 10, 20).unwrap();
    assert_eq!(eval_raw(&field, &knots, 5), 5);
    assert_eq!(eval_raw(&field, &knots, 25), 25);
}

#[test]
fn output_is_clamped() {
    let knots = check("2 * x", AxisKind::Centered, -100, 100);
    let field = Field::new(AxisKind::Centered, -100, 100).unwrap();
    assert_eq!(eval_raw(&field, &knots, 80), 100);
    assert_eq!(eval_raw(&field, &knots, -80), -100);
}

#[test]
fn zero_tolerance_rounds_exactly() {
    let f = formula("0.4 * x + 0.6 * pow(x, 3)");
    let field = Field::new(AxisKind::Centered, 0, 4095).unwrap();
    let knots = fit(&f, &field, 0.0).unwrap();
    let loose = fit(&f, &field, DEFAULT_TOLERANCE).unwrap();
    assert!(knots.len() > loose.len());
    assert!(knots.len() <= MAX_KNOTS, "{} knots", knots.len());
    for raw in 0..=4095 {
        let exact = field.exact_raw(f.eval(field.exact_x(raw)));
        let got = eval_raw(&field, &knots, raw) as f64;
        assert!((got - exact).abs() <= 0.5, "at {raw}: {got}, exact {exact}");
    }
    println!("exact: {} knots, default: {}", knots.len(), loose.len());
}

#[test]
fn combined_triggers() {
    // Two unipolar triggers into one centered axis: x - y.
    let trigger = Field::new(AxisKind::Unipolar, 0, 1023).unwrap();
    let target = Field::new(AxisKind::Centered, 0, 1023).unwrap();
    let half = DEFAULT_TOLERANCE / 2.0;
    let plus = fit_term(&formula("x"), &trigger, &target, half).unwrap();
    let minus = fit_term(&formula("-x"), &trigger, &target, half).unwrap();
    assert_eq!((plus.len(), minus.len()), (2, 2));
    for r in (0..=1023).step_by(7) {
        for l in (0..=1023).step_by(11) {
            let got = eval_sum(&target, 0, &[(&trigger, &plus, r), (&trigger, &minus, l)]);
            let y = (r as f64 - l as f64) / 1023.0;
            let exact = target.exact_raw(y.clamp(-1.0, 1.0));
            assert!(
                (got as f64 - exact).abs() <= 1.0,
                "r {r} l {l}: {got} vs {exact}"
            );
        }
    }
    // Released triggers rest in the middle.
    let rest = eval_sum(&target, 0, &[(&trigger, &plus, 0), (&trigger, &minus, 0)]);
    assert!(rest == 511 || rest == 512);
    // Out of range sources leave the target alone.
    assert_eq!(
        eval_sum(
            &target,
            77,
            &[(&trigger, &plus, 2000), (&trigger, &minus, 0)]
        ),
        77
    );
}

#[test]
fn terms_are_not_clamped() {
    let src = Field::new(AxisKind::Unipolar, 0, 255).unwrap();
    let target = Field::new(AxisKind::Unipolar, 0, 255).unwrap();
    let knots = fit_term(&formula("2 * x - 1"), &src, &target, 0.25).unwrap();
    assert_eq!(eval_term(&src, &knots, 0), Some(-ONE));
    assert_eq!(eval_term(&src, &knots, 255), Some(ONE));
    assert_eq!(eval_term(&src, &knots, 256), None);
}

#[test]
fn errors() {
    assert_eq!(
        Field::new(AxisKind::Centered, 5, 5),
        Err(Error::InvalidRange { min: 5, max: 5 })
    );
    assert_eq!(
        Field::new(AxisKind::Centered, 0, 65536),
        Err(Error::RangeTooLarge { span: 65536 })
    );
    let field = Field::new(AxisKind::Centered, 0, 10).unwrap();
    assert_eq!(
        fit(&formula("x"), &field, -0.1),
        Err(Error::InvalidTolerance(-0.1))
    );
    assert_eq!(
        fit(&formula("pow(x, 0.5)"), &field, DEFAULT_TOLERANCE),
        Err(Error::NotFinite { x: -1.0 })
    );
}

#[test]
fn too_many_knots_suggests_a_tolerance() {
    let f = formula("pow(x, 3)");
    let field = Field::new(AxisKind::Centered, 0, 65535).unwrap();
    let err = fit(&f, &field, 0.0).unwrap_err();
    let Error::TooManyKnots {
        bits,
        tolerance,
        needed,
        suggestion: Some(t),
        ..
    } = &err
    else {
        panic!("{err:?}");
    };
    assert_eq!((*bits, *tolerance), (16, 0.0));
    assert!(*needed > MAX_KNOTS);
    assert!(fit(&f, &field, *t).unwrap().len() <= MAX_KNOTS);
    let msg = err.to_string();
    for part in [
        "pow(x,3.0)",
        "16 bits",
        "tolerance 0",
        "try tolerance =",
        "default 0.5",
    ] {
        assert!(msg.contains(part), "{msg}");
    }
}

#[test]
fn tolerance_not_finite() {
    let f = formula("x");
    let field = Field::new(AxisKind::Centered, 0, 1023).unwrap();
    for t in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN, -0.5] {
        let err = fit(&f, &field, t).unwrap_err();
        assert!(matches!(err, Error::InvalidTolerance(_)), "{t}: {err}");
        assert!(
            err.to_string()
                .contains("must be a finite number of at least 0")
        );
    }
}
