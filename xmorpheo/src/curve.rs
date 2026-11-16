// SPDX-License-Identifier: GPL-2.0-or-later

//! Response curves as knot tables, and the fixed point math of the BPF
//! program.
//!
//! All floating point math happens here, in user space. A curve is a
//! sorted list of knots `{x, y, slope}` in Q24 fixed point over the
//! normalized range, `-1..=1` for centered axes and `0..=1` for unipolar
//! axes, so `1.0` is `2^24`. Q24 leaves about 256 steps of the fixed point
//! format per step of a 16 bit field, so rounding inside the math stays far
//! below the fitting tolerance, also for sums of several terms. The BPF
//! program only does integer math, in 64 bits:
//!
//! 1. Normalize: `x = ((raw - min) * in_mul + 2^15) >> 16`, minus `2^24`
//!    for centered axes. `in_mul` is `round(2^41 / span)` for centered and
//!    `round(2^40 / span)` for unipolar axes, with `span = max - min`.
//! 2. Find the segment: the last knot with `knot.x <= x`, by a binary
//!    search of at most [`SEARCH_STEPS`] steps.
//! 3. Interpolate: `y = knot.y + (((x - knot.x) * knot.slope + 2^23) >> 24)`,
//!    with an arithmetic shift. `slope` is `round((y1 - y0) * 2^24 / (x1 -
//!    x0))` to the next knot, and 0 for the last knot.
//! 4. Clamp `y` to the normalized range, once, after adding all terms.
//! 5. Denormalize: `raw = min + (((y + off) * out_mul + 2^31) >> 32)`, with
//!    `off = 2^24` and `out_mul = span * 2^7` for centered axes, and
//!    `off = 0` and `out_mul = span * 2^8` for unipolar axes.
//!
//! Raw values outside the logical range stay unchanged. All products fit
//! in 64 bits for fields of up to 16 bits.
//!
//! [`fit`] places knots at raw values and extends each segment as long as
//! the integer evaluation stays within the tolerance of the exact formula
//! at every raw value of the segment, or rounds to the same raw value as
//! the exact formula. So every output is within `tolerance + 0.5` steps of
//! the exact formula, and a tolerance of 0 gives the exactly rounded curve.
//! [`eval_raw`] is the reference of the BPF side.

use std::fmt;

use crate::formula::Formula;

/// Fraction bits of the fixed point format.
pub const FRAC: u32 = 24;

/// `1.0` in Q24 fixed point.
pub const ONE: i64 = 1 << FRAC;

/// Most knots per curve. This bounds the binary search of the BPF program.
pub const MAX_KNOTS: usize = 1024;

/// Steps of the binary search over [`MAX_KNOTS`] knots.
pub const SEARCH_STEPS: u32 = MAX_KNOTS.ilog2();

/// Largest supported span `max - min` of a field.
pub const MAX_SPAN: u64 = u16::MAX as u64;

/// Default fitting tolerance, in least significant bits of the field.
pub const DEFAULT_TOLERANCE: f64 = 0.5;

/// How an axis is normalized before the curve applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisKind {
    /// Rests in the middle of its range, like a stick. Normalized to
    /// `-1..=1`.
    Centered,
    /// Rests at the minimum of its range, like a trigger. Normalized to
    /// `0..=1`.
    Unipolar,
}

impl AxisKind {
    /// Lower end of the normalized range, in Q24.
    pub fn lowest(self) -> i64 {
        match self {
            AxisKind::Centered => -ONE,
            AxisKind::Unipolar => 0,
        }
    }
}

/// A support point of a curve, in Q24 fixed point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Knot {
    pub x: i32,
    pub y: i32,
    /// Slope to the next knot, `dy / dx` in Q24. 0 for the last knot.
    pub slope: i64,
}

/// The normalization parameters of a field, as the BPF program uses them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub kind: AxisKind,
    pub min: i32,
    pub max: i32,
    pub in_mul: u64,
    pub out_mul: u64,
}

/// Errors while fitting a curve.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    InvalidRange {
        min: i32,
        max: i32,
    },
    RangeTooLarge {
        span: u64,
    },
    InvalidTolerance(f64),
    NotFinite {
        x: f64,
    },
    /// The curve of `formula` for a field of `bits` bits needs `needed`
    /// knots at `tolerance`; `suggestion` is the smallest of a few common
    /// tolerances that fits, if one does.
    TooManyKnots {
        formula: String,
        bits: u32,
        tolerance: f64,
        needed: usize,
        suggestion: Option<f64>,
    },
}

/// Tolerances to suggest when a curve needs too many knots, smallest first.
const SUGGESTED_TOLERANCES: [f64; 8] = [0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0];

impl Error {
    /// The error for a tolerance `factor` times larger, for a term that
    /// got a share of the tolerance of its axis.
    pub fn scale_tolerance(self, factor: f64) -> Self {
        match self {
            Error::TooManyKnots {
                formula,
                bits,
                tolerance,
                needed,
                suggestion,
            } => Error::TooManyKnots {
                formula,
                bits,
                tolerance: tolerance * factor,
                needed,
                suggestion: suggestion.map(|t| t * factor),
            },
            e => e,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidRange { min, max } => write!(f, "invalid logical range {min}..={max}"),
            Error::RangeTooLarge { span } => write!(
                f,
                "logical range spans {span} values, at most {MAX_SPAN} are supported"
            ),
            Error::InvalidTolerance(t) => write!(f, "tolerance {t} is negative"),
            Error::NotFinite { x } => write!(f, "the formula is not finite at x = {x}"),
            Error::TooManyKnots {
                formula,
                bits,
                tolerance,
                needed,
                suggestion,
            } => {
                write!(
                    f,
                    "the curve {formula} needs {needed} knots for {bits} bits at tolerance \
                     {tolerance}, at most {MAX_KNOTS} fit; "
                )?;
                match suggestion {
                    Some(t) if *t < DEFAULT_TOLERANCE => {
                        write!(f, "try tolerance = {t}, or the default {DEFAULT_TOLERANCE}")
                    }
                    Some(t) => write!(f, "try tolerance = {t}"),
                    None => write!(f, "try a larger tolerance"),
                }
            }
        }
    }
}

impl std::error::Error for Error {}

impl Field {
    /// Computes the normalization parameters for the logical range
    /// `min..=max`.
    pub fn new(kind: AxisKind, min: i32, max: i32) -> Result<Self, Error> {
        if min >= max {
            return Err(Error::InvalidRange { min, max });
        }
        let span = (max as i64 - min as i64) as u64;
        if span > MAX_SPAN {
            return Err(Error::RangeTooLarge { span });
        }
        let (num, out_mul) = match kind {
            AxisKind::Centered => (1u64 << 41, span << 7),
            AxisKind::Unipolar => (1u64 << 40, span << 8),
        };
        let in_mul = (num + span / 2) / span;
        Ok(Self {
            kind,
            min,
            max,
            in_mul,
            out_mul,
        })
    }

    fn span(&self) -> f64 {
        (self.max as i64 - self.min as i64) as f64
    }

    /// Normalizes a raw value of the logical range to Q24.
    pub fn normalize(&self, raw: i32) -> i64 {
        normalize(self.in_mul, self.min, self.kind, raw)
    }

    /// Converts a Q24 value of the normalized range back to a raw value.
    pub fn denormalize(&self, y: i64) -> i32 {
        denormalize(self.out_mul, self.min, self.kind, y)
    }

    /// The exact normalized value of a raw value.
    pub fn exact_x(&self, raw: i32) -> f64 {
        let t = (raw as i64 - self.min as i64) as f64 / self.span();
        match self.kind {
            AxisKind::Centered => 2.0 * t - 1.0,
            AxisKind::Unipolar => t,
        }
    }

    /// The exact raw value, not rounded, of a normalized value.
    pub fn exact_raw(&self, y: f64) -> f64 {
        let t = match self.kind {
            AxisKind::Centered => (y + 1.0) / 2.0,
            AxisKind::Unipolar => y,
        };
        self.min as f64 + t * self.span()
    }
}

/// Normalizes a raw value to Q24 with the multiplier of a field, like the
/// BPF program.
pub fn normalize(in_mul: u64, min: i32, kind: AxisKind, raw: i32) -> i64 {
    let off = (raw as i64 - min as i64) as u64;
    ((off * in_mul + (1 << 15)) >> 16) as i64 + kind.lowest()
}

/// Converts a Q24 value back to a raw value, clamped to the range, with the
/// multiplier of a field, like the BPF program.
pub fn denormalize(out_mul: u64, min: i32, kind: AxisKind, y: i64) -> i32 {
    let u = (y.clamp(kind.lowest(), ONE) - kind.lowest()) as u64;
    (min as i64 + ((u * out_mul + (1 << 31)) >> 32) as i64) as i32
}

/// Evaluates a curve at a normalized Q24 input, like the BPF program.
pub fn eval(knots: &[Knot], x: i64) -> i64 {
    let (mut lo, mut hi) = (0usize, knots.len() - 1);
    for _ in 0..SEARCH_STEPS {
        if lo >= hi {
            break;
        }
        let mid = (lo + hi).div_ceil(2);
        if knots[mid].x as i64 <= x {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    segment(&knots[lo], x)
}

fn segment(k: &Knot, x: i64) -> i64 {
    k.y as i64 + (((x - k.x as i64) * k.slope + (1 << (FRAC - 1))) >> FRAC)
}

/// Evaluates a curve for a raw value of a field, like the BPF program.
pub fn eval_raw(field: &Field, knots: &[Knot], raw: i32) -> i32 {
    if raw < field.min || raw > field.max {
        return raw;
    }
    field.denormalize(eval(knots, field.normalize(raw)))
}

/// Evaluates one term for a raw value of its source field, in Q24 of the
/// target, like the BPF program. `None` if the value is outside the
/// logical range of the source.
pub fn eval_term(source: &Field, knots: &[Knot], raw: i32) -> Option<i64> {
    (source.min..=source.max)
        .contains(&raw)
        .then(|| eval(knots, source.normalize(raw)))
}

/// Evaluates a sum of terms for a target field, like the BPF program. The
/// target keeps its raw value if it or any source is outside its logical
/// range.
pub fn eval_sum(target: &Field, target_raw: i32, terms: &[(&Field, &[Knot], i32)]) -> i32 {
    if !(target.min..=target.max).contains(&target_raw) {
        return target_raw;
    }
    let mut sum = 0;
    for (source, knots, raw) in terms {
        match eval_term(source, knots, *raw) {
            Some(y) => sum += y,
            None => return target_raw,
        }
    }
    target.denormalize(sum)
}

fn slope(x0: i64, y0: i64, x1: i64, y1: i64) -> i64 {
    let (num, den) = ((y1 - y0) << FRAC, x1 - x0);
    // Rounds half away from zero; den is positive.
    if num >= 0 {
        (num + den / 2) / den
    } else {
        (num - den / 2) / den
    }
}

/// Largest absolute output of a term, in normalized units. Terms are not
/// clamped to the range of the target, only their sum is.
pub const TERM_LIMIT: f64 = 4.0;

/// Fits knots to a formula for a field. Each raw value of the field
/// evaluates within `tolerance` least significant bits of the exact
/// formula before rounding, or rounds to the same raw value as the exact
/// formula.
pub fn fit(formula: &Formula, field: &Field, tolerance: f64) -> Result<Vec<Knot>, Error> {
    fit_or_suggest(formula, field, field, tolerance, true)
}

/// Fits knots to one term of a sum: the input is a raw value of `source`,
/// the output is in the normalized units of `target`, and it is not
/// clamped, because only the sum of all terms is. Each raw value of the
/// source evaluates within `tolerance` least significant bits of the
/// target.
pub fn fit_term(
    formula: &Formula,
    source: &Field,
    target: &Field,
    tolerance: f64,
) -> Result<Vec<Knot>, Error> {
    fit_or_suggest(formula, source, target, tolerance, false)
}

/// Fits, and if the curve needs too many knots, finds a tolerance to
/// suggest.
fn fit_or_suggest(
    formula: &Formula,
    source: &Field,
    target: &Field,
    tolerance: f64,
    clamp: bool,
) -> Result<Vec<Knot>, Error> {
    let needed = match fit_impl(formula, source, target, tolerance, clamp)? {
        Ok(knots) => return Ok(knots),
        Err(needed) => needed,
    };
    let suggestion = SUGGESTED_TOLERANCES
        .into_iter()
        .filter(|&t| t > tolerance)
        .find(|&t| matches!(fit_impl(formula, source, target, t, clamp), Ok(Ok(_))));
    let span = source.max as i64 - source.min as i64 + 1;
    Err(Error::TooManyKnots {
        formula: formula.to_string(),
        bits: (span as f64).log2().ceil() as u32,
        tolerance,
        needed,
        suggestion,
    })
}

fn fit_impl(
    formula: &Formula,
    source: &Field,
    target: &Field,
    tolerance: f64,
    clamp: bool,
) -> Result<Result<Vec<Knot>, usize>, Error> {
    if tolerance.is_nan() || tolerance < 0.0 {
        return Err(Error::InvalidTolerance(tolerance));
    }
    let (lowest, highest) = if clamp {
        (target.kind.lowest() as f64 / ONE as f64, 1.0)
    } else {
        (-TERM_LIMIT, TERM_LIMIT)
    };
    let n = (source.max as i64 - source.min as i64 + 1) as usize;
    let raw = |i: usize| (source.min as i64 + i as i64) as i32;
    // The output as a raw value of the target, not rounded, and clamped
    // like the BPF program does where the output is clamped.
    let raw_of_q16 = |y: i64| {
        let y = if clamp {
            y.clamp(target.kind.lowest(), ONE)
        } else {
            y
        };
        target.exact_raw(y as f64 / ONE as f64)
    };

    // Exact outputs, normalized and as raw values, and the input in Q24.
    let mut exact = Vec::with_capacity(n);
    let mut wanted = Vec::with_capacity(n);
    let mut xs = Vec::with_capacity(n);
    for i in 0..n {
        let x = source.exact_x(raw(i));
        let y = formula.eval(x);
        if !y.is_finite() {
            return Err(Error::NotFinite { x });
        }
        let y = y.clamp(lowest, highest);
        let t = target.exact_raw(y);
        // For tolerances below half a step, nudge the Q24 value so the
        // knot itself rounds like the exact formula, where Q24 can
        // represent that. Larger tolerances do not need it, and on 16 bit
        // fields the nudge would bend the segments. Terms are rounded only
        // as a sum, so they are not nudged.
        let mut q = (y * ONE as f64).round() as i64;
        if clamp && tolerance < 0.5 {
            q = [0, 1, -1, 2, -2]
                .iter()
                .map(|d| q + d)
                .find(|&q| (target.denormalize(q) as f64 - t).abs() <= 0.5)
                .unwrap_or(q);
            q = q.clamp(target.kind.lowest(), ONE);
        }
        exact.push(q);
        wanted.push(t);
        xs.push(source.normalize(raw(i)));
    }

    // A segment from knot s to knot e is good if every raw value from s up
    // to, but not including, e is within the tolerance or, for a whole
    // curve, rounds like the exact formula. The value at e belongs to the
    // next segment.
    let good = |s: usize, e: usize| {
        let k = Knot {
            x: xs[s] as i32,
            y: exact[s] as i32,
            slope: slope(xs[s], exact[s], xs[e], exact[e]),
        };
        (s..e).all(|i| {
            let y = segment(&k, xs[i]);
            (raw_of_q16(y) - wanted[i]).abs() <= tolerance
                || (clamp && (target.denormalize(y) as f64 - wanted[i]).abs() <= 0.5)
        })
    };

    let mut ends = vec![0usize];
    let mut s = 0;
    while s < n - 1 {
        // Gallop to a bad segment end, then search back for the last good
        // one. A segment of one step is always good.
        let max_len = n - 1 - s;
        let mut good_len = 1;
        let mut bad_len = None;
        let mut len = 2;
        while good_len < max_len {
            let try_len = len.min(max_len);
            if good(s, s + try_len) {
                good_len = try_len;
                len *= 2;
            } else {
                bad_len = Some(try_len);
                break;
            }
        }
        if let Some(mut bad_len) = bad_len {
            while bad_len - good_len > 1 {
                let mid = (good_len + bad_len) / 2;
                if good(s, s + mid) {
                    good_len = mid;
                } else {
                    bad_len = mid;
                }
            }
        }
        s += good_len;
        ends.push(s);
    }
    if ends.len() > MAX_KNOTS {
        return Ok(Err(ends.len()));
    }

    let knots = ends
        .iter()
        .enumerate()
        .map(|(j, &i)| Knot {
            x: xs[i] as i32,
            y: exact[i] as i32,
            slope: match ends.get(j + 1) {
                Some(&e) => slope(xs[i], exact[i], xs[e], exact[e]),
                None => 0,
            },
        })
        .collect();
    Ok(Ok(knots))
}
