// SPDX-License-Identifier: GPL-2.0-or-later

//! What to load for a device: the tables of fields, outputs, and terms,
//! and the curves and knots, computed from its configuration and its
//! report descriptor.
//!
//! Each configured axis becomes one output per descriptor field with its
//! usage. An output is the sum of its terms, and each term is a curve over
//! a source field in the same report. Planning does no I/O, so
//! `xmorpheo attach --dry-run` and the tests can show the result without
//! loading anything. [`Plan::rewrite`] is the Rust reference of the BPF
//! program.

use std::fmt;

use crate::bpf::{self, types};
use crate::config;
use crate::curve::{self, AxisKind};
use crate::descriptor::{Descriptor, Field, Usage};
use crate::formula::Formula;
use crate::usage;

/// A term of a planned output, for display.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedTerm {
    /// The variable of the formula.
    pub var: String,
    pub source: Usage,
    pub kind: AxisKind,
    /// The canonical text of the term, in `x`.
    pub curve: String,
    pub curve_index: usize,
    pub knots: usize,
}

/// An output of the plan, for display.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedAxis {
    pub usage: Usage,
    pub report_id: u8,
    pub report_len: u32,
    pub bit_offset: u32,
    pub bit_size: u32,
    pub min: i32,
    pub max: i32,
    pub kind: AxisKind,
    /// The formula as configured, in canonical text.
    pub formula: String,
    pub terms: Vec<PlannedTerm>,
}

/// Everything the loader writes into the BPF program for one device.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub fields: Vec<types::xm_field>,
    pub outputs: Vec<types::xm_output>,
    pub terms: Vec<types::xm_term>,
    pub curves: Vec<types::xm_curve>,
    pub knots: Vec<types::xm_knot>,
    pub axes: Vec<PlannedAxis>,
}

/// Errors while planning.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    AxisNotFound(Usage),
    SourceNotInReport { usage: Usage, source: Usage },
    FieldTooLarge { usage: Usage, bits: u32 },
    NoInputReport { usage: Usage, report_id: u8 },
    TooManyFields,
    TooManyOutputs,
    TooManyTerms { usage: Usage },
    TooManyCurves,
    TooManyKnots { usage: Usage, needed: usize },
    Curve { usage: Usage, error: curve::Error },
}

fn name(u: Usage) -> String {
    usage::name(u).map_or(u.to_string(), str::to_string)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::AxisNotFound(u) => {
                write!(f, "axis {} is not in the report descriptor", name(*u))
            }
            Error::SourceNotInReport { usage, source } => write!(
                f,
                "axis {} reads {}, which is not in the same report",
                name(*usage),
                name(*source)
            ),
            Error::FieldTooLarge { usage, bits } => write!(
                f,
                "axis {} has {bits} bits, at most {} are supported",
                name(*usage),
                bpf::MAX_BITS
            ),
            Error::NoInputReport { usage, report_id } => write!(
                f,
                "axis {}: input report {report_id} has no length",
                name(*usage)
            ),
            Error::TooManyFields => write!(f, "more than {} fields per device", bpf::MAX_FIELDS),
            Error::TooManyOutputs => {
                write!(f, "more than {} axis fields per device", bpf::MAX_OUTPUTS)
            }
            Error::TooManyTerms { usage } => write!(
                f,
                "axis {} reads more than {} axes",
                name(*usage),
                bpf::OUTPUT_TERMS
            ),
            Error::TooManyCurves => write!(f, "more than {} curves", bpf::MAX_CURVES),
            Error::TooManyKnots { usage, needed } => write!(
                f,
                "axis {}: the curves of the device need {needed} knots, at most {} fit; \
                 try a larger tolerance, like the default {}",
                name(*usage),
                bpf::MAX_KNOTS,
                curve::DEFAULT_TOLERANCE
            ),
            Error::Curve { usage, error } => write!(f, "axis {}: {error}", name(*usage)),
        }
    }
}

impl std::error::Error for Error {}

/// A curve as the dedup sees it: the term in `x`, whether it is fitted as
/// a whole curve, and the kinds and ranges of source and target.
type CurveKey = (String, bool, AxisKind, i32, i32, AxisKind, i32, i32, u64);

struct Planner<'a> {
    desc: &'a Descriptor,
    plan: Plan,
    keys: Vec<CurveKey>,
}

impl Planner<'_> {
    /// The index of a descriptor field in the field table, adding it.
    fn slot(&mut self, f: &Field, usage: Usage) -> Result<u16, Error> {
        if f.bit_size > bpf::MAX_BITS {
            return Err(Error::FieldTooLarge {
                usage,
                bits: f.bit_size,
            });
        }
        let len = self
            .desc
            .input_report_len(f.report_id)
            .filter(|len| *len <= u16::MAX as u32)
            .ok_or(Error::NoInputReport {
                usage,
                report_id: f.report_id,
            })?;
        if let Some(i) = self.plan.fields.iter().position(|e| {
            (e.report_id, e.bit_offset, e.bit_size as u32)
                == (f.report_id, f.bit_offset, f.bit_size)
        }) {
            return Ok(i as u16);
        }
        if self.plan.fields.len() >= bpf::MAX_FIELDS {
            return Err(Error::TooManyFields);
        }
        self.plan.fields.push(bpf::field(f, len as u16));
        Ok((self.plan.fields.len() - 1) as u16)
    }

    /// The index of a curve, fitting it unless an equal one exists.
    fn curve(
        &mut self,
        usage: Usage,
        term: &Formula,
        whole: bool,
        source: &curve::Field,
        target: &curve::Field,
        tolerance: f64,
    ) -> Result<usize, Error> {
        let key = (
            term.to_string(),
            whole,
            source.kind,
            source.min,
            source.max,
            target.kind,
            target.min,
            target.max,
            tolerance.to_bits(),
        );
        if let Some(i) = self.keys.iter().position(|k| *k == key) {
            return Ok(i);
        }
        let knots = if whole {
            curve::fit(term, source, tolerance)
        } else {
            curve::fit_term(term, source, target, tolerance)
        }
        .map_err(|error| Error::Curve { usage, error })?;
        if self.plan.curves.len() >= bpf::MAX_CURVES {
            return Err(Error::TooManyCurves);
        }
        if self.plan.knots.len() + knots.len() > bpf::MAX_KNOTS {
            return Err(Error::TooManyKnots {
                usage,
                needed: self.plan.knots.len() + knots.len(),
            });
        }
        self.plan.curves.push(types::xm_curve {
            first: self.plan.knots.len() as u32,
            count: knots.len() as u32,
        });
        self.plan.knots.extend(knots.iter().map(bpf::knot));
        self.keys.push(key);
        Ok(self.keys.len() - 1)
    }

    fn axis(&mut self, axis: &config::Axis, target: &Field) -> Result<(), Error> {
        let usage = axis.usage;
        let curve_error = |error| Error::Curve { usage, error };
        let target_norm = curve::Field::new(axis.kind, target.logical_min, target.logical_max)
            .map_err(curve_error)?;
        let target_slot = self.slot(target, usage)?;
        if axis.terms.len() > bpf::OUTPUT_TERMS {
            return Err(Error::TooManyTerms { usage });
        }
        // The tolerance applies to the sum, so each term gets its share.
        let tolerance = axis.tolerance / axis.terms.len() as f64;

        let first_term = self.plan.terms.len();
        let mut planned = Vec::new();
        for t in &axis.terms {
            let source = &axis.sources[t.var];
            let field = if source.usage == usage {
                target
            } else {
                self.desc
                    .fields
                    .iter()
                    .find(|f| {
                        f.usage == source.usage
                            && f.report_id == target.report_id
                            && !f.is_relative()
                    })
                    .ok_or(Error::SourceNotInReport {
                        usage,
                        source: source.usage,
                    })?
            };
            let source_norm = curve::Field::new(source.kind, field.logical_min, field.logical_max)
                .map_err(curve_error)?;
            // A curve of the axis itself is fitted as a whole, clamped and
            // rounded like a curve without terms.
            let whole =
                axis.terms.len() == 1 && std::ptr::eq(field, target) && source.kind == axis.kind;
            let index = self
                .curve(
                    usage,
                    &t.curve,
                    whole,
                    &source_norm,
                    &target_norm,
                    tolerance,
                )
                .map_err(|e| match e {
                    // Name the tolerance of the axis, not the share of the term.
                    Error::Curve { usage, error } => Error::Curve {
                        usage,
                        error: error.scale_tolerance(axis.terms.len() as f64),
                    },
                    e => e,
                })?;
            let source_slot = self.slot(field, usage)?;
            if self.plan.terms.len() >= bpf::MAX_TERMS {
                return Err(Error::TooManyTerms { usage });
            }
            self.plan
                .terms
                .push(bpf::term(source_slot, &source_norm, index as u16));
            planned.push(PlannedTerm {
                var: source.name.clone(),
                source: source.usage,
                kind: source.kind,
                curve: t.curve.to_string(),
                curve_index: index,
                knots: self.plan.curves[index].count as usize,
            });
        }

        if self.plan.outputs.len() >= bpf::MAX_OUTPUTS {
            return Err(Error::TooManyOutputs);
        }
        self.plan.outputs.push(bpf::output(
            target_slot,
            &target_norm,
            first_term as u16,
            axis.terms.len() as u8,
        ));
        let report_len = self.plan.fields[target_slot as usize].report_len as u32;
        self.plan.axes.push(PlannedAxis {
            usage,
            report_id: target.report_id,
            report_len,
            bit_offset: target.bit_offset,
            bit_size: target.bit_size,
            min: target.logical_min,
            max: target.logical_max,
            kind: axis.kind,
            formula: axis.curve.to_string(),
            terms: planned,
        });
        Ok(())
    }
}

/// Plans the rewrite of a device. Every configured axis and every source
/// must exist in the descriptor. An axis whose usage appears in several
/// fields, for example in several reports, rewrites all of them, each with
/// the sources of its own report.
pub fn plan(device: &config::Device, desc: &Descriptor) -> Result<Plan, Error> {
    let mut p = Planner {
        desc,
        plan: Plan::default(),
        keys: Vec::new(),
    };
    for axis in &device.axes {
        let targets: Vec<_> = desc
            .fields
            .iter()
            .filter(|f| f.usage == axis.usage && !f.is_relative())
            .collect();
        if targets.is_empty() {
            return Err(Error::AxisNotFound(axis.usage));
        }
        for f in targets {
            p.axis(axis, f)?;
        }
    }
    Ok(p.plan)
}

fn kind(centered: u8) -> AxisKind {
    if centered != 0 {
        AxisKind::Centered
    } else {
        AxisKind::Unipolar
    }
}

impl Plan {
    /// Rewrites a report like the BPF program: all fields are read first,
    /// then all outputs computed and written. Reports of other lengths or
    /// report IDs stay untouched, and so does an output whose field or any
    /// source is outside its logical range.
    pub fn rewrite(&self, report: &mut [u8]) {
        let Some(&report_id) = report.first() else {
            return;
        };
        let values: Vec<Option<i32>> = self
            .fields
            .iter()
            .map(|f| {
                let matches = f.report_len as usize == report.len()
                    && (f.report_id == 0 || f.report_id == report_id);
                matches
                    .then(|| bpf::get(f, report))
                    .flatten()
                    .filter(|v| (f.min..=f.max).contains(v))
            })
            .collect();

        let mut writes = Vec::new();
        'outputs: for o in &self.outputs {
            let target = o.field as usize;
            let Some(old) = values[target] else {
                continue;
            };
            let mut sum = 0;
            for t in &self.terms[o.first_term as usize..][..o.nr_terms as usize] {
                let f = &self.fields[t.field as usize];
                let Some(raw) = values[t.field as usize] else {
                    continue 'outputs;
                };
                let c = &self.curves[t.curve as usize];
                let knots: Vec<curve::Knot> = self.knots[c.first as usize..][..c.count as usize]
                    .iter()
                    .map(|k| curve::Knot {
                        x: k.x,
                        y: k.y,
                        slope: k.slope,
                    })
                    .collect();
                sum += curve::eval(
                    &knots,
                    curve::normalize(t.in_mul, f.min, kind(t.centered), raw),
                );
            }
            let f = &self.fields[target];
            let new = curve::denormalize(o.out_mul, f.min, kind(o.centered), sum);
            if new != old {
                writes.push((target, new));
            }
        }
        for (target, value) in writes {
            bpf::put(&self.fields[target], report, value);
        }
    }
}
