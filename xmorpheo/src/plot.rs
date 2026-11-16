// SPDX-License-Identifier: GPL-2.0-or-later

//! CSV and gnuplot output of `xmorpheo curve`.

use std::fmt::Write;

use xmorpheo::curve::{self, AxisKind, Field, Knot, ONE};
use xmorpheo::formula::Formula;

/// Most points of the curves in a gnuplot script. Larger fields are thinned
/// out, the error keeps its minimum and maximum per bucket.
const MAX_POINTS: usize = 2048;

/// A fitted curve with everything needed to show it.
pub struct Curve<'a> {
    pub formula: &'a Formula,
    pub field: &'a Field,
    pub knots: &'a [Knot],
    pub tolerance: f64,
}

/// One raw input value: its normalized input, the exact output, the output
/// of the integer math, and the error in steps.
struct Point {
    raw: i32,
    x: f64,
    exact: f64,
    lut: i32,
    error: f64,
}

impl Curve<'_> {
    fn lowest(&self) -> f64 {
        self.field.kind.lowest() as f64 / ONE as f64
    }

    fn point(&self, raw: i32) -> Point {
        let x = self.field.exact_x(raw);
        let exact = self
            .field
            .exact_raw(self.formula.eval(x).clamp(self.lowest(), 1.0));
        let lut = curve::eval_raw(self.field, self.knots, raw);
        Point {
            raw,
            x,
            exact,
            lut,
            error: lut as f64 - exact,
        }
    }

    fn points(&self) -> impl Iterator<Item = Point> + '_ {
        (self.field.min..=self.field.max).map(|raw| self.point(raw))
    }

    /// A knot as raw input and output, not rounded.
    fn knot(&self, k: &Knot) -> (f64, f64) {
        let raw = |v: i32| self.field.exact_raw(v as f64 / ONE as f64);
        (raw(k.x), raw(k.y))
    }

    fn kind(&self) -> &'static str {
        match self.field.kind {
            AxisKind::Centered => "centered",
            AxisKind::Unipolar => "unipolar",
        }
    }

    /// Every raw value as CSV, and with `knots`, the knots after an empty
    /// line.
    pub fn csv(&self, knots: bool) -> String {
        let mut out = String::from("raw_in,x,exact,lut,error_lsb\n");
        for p in self.points() {
            let _ = writeln!(
                out,
                "{},{:.6},{:.3},{},{:.3}",
                p.raw, p.x, p.exact, p.lut, p.error
            );
        }
        if knots {
            out.push_str("\nknot_in,knot_out,slope_q24\n");
            for k in self.knots {
                let (x, y) = self.knot(k);
                let _ = writeln!(out, "{x:.3},{y:.3},{}", k.slope);
            }
        }
        out
    }

    /// A gnuplot script with the data inline. With an image, it renders to
    /// that file with the terminal, otherwise to the default terminal.
    pub fn gnuplot(&self, image: Option<(&str, &str)>) -> String {
        let (min, max) = (self.field.min, self.field.max);
        let n = (max as i64 - min as i64 + 1) as usize;
        let bucket = n.div_ceil(MAX_POINTS);
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# xmorpheo curve: {}, {}, {min}..={max}, tolerance {}, {} knots",
            self.formula,
            self.kind(),
            self.tolerance,
            self.knots.len()
        );

        // The curves at the first value of each bucket, the error as the
        // smallest and largest in each bucket.
        out.push_str("$data << EOD\n# raw_in exact lut error_min error_max\n");
        let points: Vec<Point> = self.points().collect();
        for chunk in points.chunks(bucket) {
            let p = &chunk[0];
            let lo = chunk.iter().map(|p| p.error).fold(f64::INFINITY, f64::min);
            let hi = chunk
                .iter()
                .map(|p| p.error)
                .fold(f64::NEG_INFINITY, f64::max);
            let _ = writeln!(
                out,
                "{} {:.3} {} {:.3} {:.3}",
                p.raw, p.exact, p.lut, lo, hi
            );
        }
        out.push_str("EOD\n$knots << EOD\n# knot_in knot_out\n");
        for k in self.knots {
            let (x, y) = self.knot(k);
            let _ = writeln!(out, "{x:.3} {y:.3}");
        }
        out.push_str("EOD\n");

        if let Some((terminal, file)) = image {
            let _ = writeln!(out, "set terminal {terminal} size 1000,800");
            let _ = writeln!(out, "set output {file:?}");
        }
        let bits = (n as f64).log2().ceil() as u32;
        // Drawn first, so the curves stay on top. The error as a thin line, thinned out as the band between the
        // smallest and largest error of each group, with both as lines.
        let line = "axes x1y2 with lines lw 0.5 lc rgb '#60ff8c00'";
        let error = if bucket == 1 {
            format!("$data using 1:4 title 'error' {line}")
        } else {
            format!(
                "$data using 1:4:5 axes x1y2 notitle with filledcurves \\\n\
                 \x20        fs transparent solid 0.12 noborder lc rgb 'orange', \\\n\
                 \x20    $data using 1:4 title 'error' {line}, \\\n\
                 \x20    $data using 1:5 notitle {line}"
            )
        };
        // Tics at eighths of the span, like 0, 256, 512, ... for 11 bits,
        // so the center of the field has one. The range ends one past the
        // field, so the last tic shows too.
        let step = n as f64 / 8.0;
        let end = min as i64 + n as i64;
        // The middle of the field, the rest position of a centered axis,
        // as a stronger line, and the normalized input on top.
        let mid = (min as f64 + max as f64) / 2.0;
        let (lo, x2step) = match self.field.kind {
            AxisKind::Centered => (-1.0, 0.25),
            AxisKind::Unipolar => (0.0, 0.125),
        };
        // The normalized input at the end of the range, one past the field.
        let hi = lo + n as f64 * (1.0 - lo) / (n - 1) as f64;
        // Every output is within the tolerance plus half a step of the
        // exact curve: a dashed line at that bound, in twice the room.
        let bound = self.tolerance + 0.5;
        let e = 2.0 * bound;
        let _ = writeln!(
            out,
            "set title {:?} noenhanced offset 0,1",
            format!(
                "{}\n{}, {bits} bit, tolerance {}, {} knots",
                self.formula,
                self.kind(),
                self.tolerance,
                self.knots.len()
            )
        );
        let _ = writeln!(
            out,
            "set xlabel 'raw input'\n\
             set ylabel 'raw output'\n\
             set y2label 'error in steps'\n\
             set xrange [{min}:{end}]\n\
             set yrange [{min}:{end}]\n\
             set xtics {min},{step}\n\
             set ytics {min},{step} nomirror\n\
             set x2range [{lo}:{hi}]\n\
             set x2tics {x2step}\n\
             set x2label 'normalized input'\n\
             set tmargin 7\n\
             set arrow from {mid},graph 0 to {mid},graph 1 back nohead lw 1.5 lc rgb 'gray50'\n\
             set arrow from graph 0,first {mid} to graph 1,first {mid} back nohead lw 1.5 lc rgb 'gray50'\n\
             set y2range [-{e}:{e}]\n\
             set arrow from graph 0,second {bound} to graph 1,second {bound} back nohead dt 2 lc rgb 'dark-orange'\n\
             set arrow from graph 0,second -{bound} to graph 1,second -{bound} back nohead dt 2 lc rgb 'dark-orange'\n\
             set y2tics\n\
             set grid\n\
             set key left top\n\
             plot {error}, \\\n\
             \x20    x title 'linear' lc rgb 'gray' dt 2, \\\n\
             \x20    $data using 1:2 title 'exact' with lines lw 4 lc rgb '#8fb8ff', \\\n\
             \x20    $data using 1:3 title 'lut' with steps lc rgb 'red', \\\n\
             \x20    $knots using 1:2 title 'knots' with points pt 7 ps 0.6 lc rgb 'black'"
        );
        out
    }
}

#[cfg(test)]
mod tests;
