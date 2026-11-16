// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::collections::HashMap;

fn fitted(src: &str, kind: AxisKind, max: i32) -> (Formula, Field, Vec<Knot>) {
    let formula = Formula::parse(src, &HashMap::new()).unwrap();
    let field = Field::new(kind, 0, max).unwrap();
    let knots = curve::fit(&formula, &field, 0.5).unwrap();
    (formula, field, knots)
}

#[test]
fn csv_has_every_value_and_the_knots() {
    let (formula, field, knots) = fitted("0.4 * x + 0.6 * pow(x, 3)", AxisKind::Centered, 1023);
    let c = Curve {
        formula: &formula,
        field: &field,
        knots: &knots,
        tolerance: 0.5,
    };
    let csv = c.csv(true);
    let (values, knot_rows) = csv.split_once("\n\n").unwrap();
    let mut lines = values.lines();
    assert_eq!(lines.next(), Some("raw_in,x,exact,lut,error_lsb"));
    let rows: Vec<Vec<f64>> = lines
        .map(|l| l.split(',').map(|v| v.parse().unwrap()).collect())
        .collect();
    assert_eq!(rows.len(), 1024);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.len(), 5);
        assert_eq!(row[0], i as f64);
        // The integer math stays within one step of the exact curve.
        assert!(row[4].abs() <= 1.0, "{row:?}");
        assert!((row[3] - row[2] - row[4]).abs() < 0.01);
    }
    let mut knot_lines = knot_rows.lines();
    assert_eq!(knot_lines.next(), Some("knot_in,knot_out,slope_q24"));
    assert_eq!(knot_lines.count(), knots.len());
    assert!(!c.csv(false).contains("knot_in"));
}

#[test]
fn gnuplot_script_has_its_blocks_and_stays_small() {
    let (formula, field, knots) = fitted("0.4 * x + 0.6 * pow(x, 3)", AxisKind::Centered, 65535);
    let c = Curve {
        formula: &formula,
        field: &field,
        knots: &knots,
        tolerance: 0.5,
    };
    let script = c.gnuplot(None);
    for part in [
        "$data << EOD",
        "$knots << EOD",
        "axes x1y2",
        "16 bit",
        "centered",
        "x title 'linear'",
    ] {
        assert!(script.contains(part), "{part}");
    }
    assert!(!script.contains("set terminal"));
    let data = script.split("$data << EOD\n").nth(1).unwrap();
    let rows = data[..data.find("EOD").unwrap()].lines().count() - 1;
    assert!(rows <= MAX_POINTS, "{rows} rows");
    let knot_block = script.split("$knots << EOD\n").nth(1).unwrap();
    assert_eq!(
        knot_block[..knot_block.find("EOD").unwrap()]
            .lines()
            .count()
            - 1,
        knots.len()
    );

    let png = c.gnuplot(Some(("pngcairo", "curve.png")));
    assert!(png.contains("set terminal pngcairo") && png.contains("set output \"curve.png\""));
}

/// Runs the script through gnuplot with a text terminal, if gnuplot is
/// installed.
#[test]
fn gnuplot_runs_the_script() {
    // Every value, and thinned out.
    for max in [1023, 65535] {
        run_gnuplot(max);
    }
}

fn run_gnuplot(max: i32) {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let (formula, field, knots) = fitted("0.4 * x + 0.6 * pow(x, 3)", AxisKind::Centered, max);
    let c = Curve {
        formula: &formula,
        field: &field,
        knots: &knots,
        tolerance: 0.5,
    };
    let script = format!("set terminal dumb\n{}", c.gnuplot(None));
    let Ok(mut child) = Command::new("gnuplot")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    else {
        eprintln!("gnuplot is not installed, skipped");
        return;
    };
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success() && stderr.trim().is_empty(), "{stderr}");
}
