// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use xmorpheo::formula::Formula;
use xmorpheo::formula::*;

fn parse(src: &str) -> Formula {
    Formula::parse(src, &HashMap::new()).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

#[test]
fn precedence_and_unary_minus() {
    let f = parse("1 + 2 * x - -x / 4");
    assert!(close(f.eval(2.0), 1.0 + 4.0 + 0.5));
    assert!(close(parse("-(x + 1) * 2").eval(1.0), -4.0));
}

#[test]
fn ets2_steering_curve() {
    // From the ETS2 steering mix, with c_steer_func = 1.4 and
    // c_steer_dz = 0.05.
    let constants = HashMap::from([
        ("c_steer_dz".to_string(), 0.05),
        ("c_steer_func".to_string(), 1.4),
    ]);
    let f = Formula::parse(
        "pow(normalize(abs(x), c_steer_dz), (1.0 + c_steer_func * 0.5)) * sign(x)",
        &constants,
    )
    .unwrap();
    assert_eq!(f.eval(0.0), 0.0);
    assert_eq!(f.eval(0.04), 0.0);
    assert!(close(f.eval(1.0), 1.0));
    assert!(close(f.eval(-1.0), -1.0));
    let half = 0.525; // halfway between the deadzone and full
    assert!(close(f.eval(half), 0.5f64.powf(1.7)));
    assert!(close(f.eval(-half), -(0.5f64.powf(1.7))));
}

#[test]
fn functions() {
    assert_eq!(parse("sign(x)").eval(0.0), 0.0);
    assert_eq!(parse("clamp(x, -0.5, 0.5)").eval(0.9), 0.5);
    assert_eq!(parse("min(x, 0.2)").eval(0.9), 0.2);
    assert_eq!(parse("max(x, 0.2)").eval(0.1), 0.2);
    assert_eq!(parse("sel(0, 1, 2)").eval(0.0), 2.0);
    assert_eq!(parse("sel(x, 1, 2)").eval(0.5), 1.0);
    assert_eq!(parse("normalize(x, 0.2)").eval(0.1), 0.0);
    assert!(close(parse("normalize(x, 0.2)").eval(0.6), 0.5));
    assert!(close(parse("deadzone(x, 0.2)").eval(-0.6), -0.5));
    assert_eq!(parse("deadzone(x, 0.2)").eval(0.15), 0.0);
}

#[test]
fn canonical_text_ignores_spacing_and_constant_names() {
    let constants = HashMap::from([("e".to_string(), 1.7)]);
    let a = Formula::parse("pow(abs(x),e)*sign(x)", &constants).unwrap();
    let b = parse("pow( abs(x), 1.7 ) * sign( x )");
    assert_eq!(a.to_string(), b.to_string());
    assert_eq!(a.to_string(), "(pow(abs(x),1.7)*sign(x))");
}

#[test]
fn errors() {
    let empty = HashMap::new();
    let err = |s| Formula::parse(s, &empty).unwrap_err();
    assert_eq!(err("x +"), Error::UnexpectedEnd);
    assert_eq!(err("x $ 1"), Error::UnexpectedChar { pos: 2, ch: '$' });
    assert_eq!(err("x 1"), Error::UnexpectedToken { pos: 2 });
    assert_eq!(
        err("y"),
        Error::UnknownName {
            pos: 0,
            name: "y".into()
        }
    );
    assert_eq!(
        err("memory(x, 1)"),
        Error::UnknownFunction {
            pos: 0,
            name: "memory".into()
        }
    );
    assert_eq!(
        err("pow(x)"),
        Error::WrongArity {
            pos: 0,
            name: "pow".into(),
            expected: 2,
            got: 1
        }
    );
}

fn multi(src: &str, vars: &[&str]) -> MultiFormula {
    MultiFormula::parse(src, &HashMap::new(), vars).unwrap()
}

/// The terms add up to the formula everywhere.
fn check_terms(src: &str, vars: &[&str]) -> Vec<(usize, String)> {
    let f = multi(src, vars);
    let terms = f.terms().unwrap();
    let points = [-1.0, -0.6, -0.25, 0.0, 0.3, 0.7, 1.0];
    let mut values = vec![0.0; vars.len()];
    for i in 0..points.len().pow(vars.len() as u32) {
        let mut k = i;
        for v in values.iter_mut() {
            *v = points[k % points.len()];
            k /= points.len();
        }
        let sum: f64 = terms.iter().map(|t| t.curve.eval(values[t.var])).sum();
        assert!(close(sum, f.eval(&values)), "{src} at {values:?}");
    }
    terms.iter().map(|t| (t.var, t.curve.to_string())).collect()
}

#[test]
fn variables() {
    let err = MultiFormula::parse("x - z", &HashMap::new(), &["x", "y"]).unwrap_err();
    assert_eq!(
        err,
        Error::UnknownName {
            pos: 4,
            name: "z".to_string()
        }
    );
    let constants = HashMap::from([("k".to_string(), 2.0)]);
    let f = MultiFormula::parse("x - y * k", &constants, &["x", "y"]).unwrap();
    assert_eq!(f.to_string(), "(x-(y*2.0))");
    assert_eq!(f.used(), [true, true]);
    assert_eq!(multi("y", &["x", "y"]).used(), [false, true]);
    assert!(close(f.eval(&[0.5, 0.25]), 0.0));
}

#[test]
fn separable_terms() {
    assert_eq!(
        check_terms("x - y", &["x", "y"]),
        [(0, "x".into()), (1, "(-x)".into())]
    );
    assert_eq!(
        check_terms("0.5 + (x - y) / 2", &["x", "y"]),
        [(0, "((x/2.0)+0.5)".into()), (1, "((-x)/2.0)".into())]
    );
    check_terms("2 * (x + y)", &["x", "y"]);
    check_terms("-(pow(x, 3) - abs(y)) * 0.5", &["x", "y"]);
    check_terms("x + y + x", &["x", "y"]);
    // Parts of one variable stay one term.
    assert_eq!(check_terms("pow(x, 2) + x", &["x", "y"]).len(), 1);
    // Constants alone go to the first variable.
    assert_eq!(check_terms("0", &["x"]), [(0, "0.0".into())]);
    assert_eq!(check_terms("y + 1", &["x", "y"]), [(1, "(x+1.0)".into())]);
}

#[test]
fn inseparable_formulas() {
    for (src, part) in [
        ("x * y", "(x*y)"),
        ("pow(x - y, 2)", "pow((x-y),2.0)"),
        ("1 / (x + y)", "(1.0/(x+y))"),
        ("x + max(x, y)", "max(x,y)"),
        ("abs(x - y)", "abs((x-y))"),
        ("x / y", "(x/y)"),
        ("clamp(x, y, 1)", "clamp(x,y,1.0)"),
    ] {
        assert_eq!(
            multi(src, &["x", "y"]).terms(),
            Err(Error::NotSeparable(part.to_string())),
            "{src}"
        );
    }
}
