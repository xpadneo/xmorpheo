// SPDX-License-Identifier: GPL-2.0-or-later

//! The configuration file.
//!
//! The configuration is TOML. It names devices by vendor and product ID,
//! plus an optional HID unique ID, and lists the axes to rewrite with
//! their curve formulas:
//!
//! ```toml
//! tolerance = 0.5         # optional, in least significant bits
//!
//! [constants]
//! k = 0.6
//!
//! [[device]]
//! name = "VKB Gladiator EVO R"
//! vendor = 0x231d
//! product = 0x0200
//!
//! [device.flat]           # optional, evdev flat by evdev axis, default 0
//! ABS_X = 128
//!
//! [[device.axis]]
//! usage = "X"
//! curve = "(1 - k) * x + k * pow(x, 3)"
//! kind = "centered"       # optional, defaults by usage
//! tolerance = 0           # optional, overrides the global tolerance
//! ```
//!
//! Formulas are parsed and their constants resolved while reading the
//! configuration, so every error shows up at once.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde::Deserialize;

use crate::curve::{self, AxisKind};
use crate::descriptor::Usage;
use crate::formula::{self, MultiFormula, Term};
use crate::usage;

/// A parsed configuration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    pub devices: Vec<Device>,
    /// Problems that do not stop the configuration from working, like
    /// variables that no formula uses.
    pub warnings: Vec<String>,
}

/// A device and the axes to rewrite on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub name: Option<String>,
    pub vendor: u16,
    pub product: u16,
    /// The HID unique ID (`HID_UNIQ`), to tell identical devices apart.
    pub uniq: Option<String>,
    /// The evdev fuzz of axes by evdev name, like `ABS_X`. Axes not listed
    /// get 0.
    pub fuzz: BTreeMap<String, u32>,
    /// The evdev flat of axes by evdev name. Axes not listed get 0.
    pub flat: BTreeMap<String, u32>,
    /// Leave the evdev fuzz and flat as the driver set them.
    pub keep_evdev: bool,
    pub axes: Vec<Axis>,
    /// The file the entry comes from, if it was read from one.
    pub origin: Option<std::path::PathBuf>,
}

/// An axis to rewrite.
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub usage: Usage,
    pub kind: AxisKind,
    /// The formula, in the variables of `sources`.
    pub curve: MultiFormula,
    /// The variables of the formula and the axes they read. Without a
    /// `source` table, `x` reads the axis itself, with its kind.
    pub sources: Vec<Source>,
    /// The formula as a sum of terms in one variable each.
    pub terms: Vec<Term>,
    /// Fitting tolerance in least significant bits.
    pub tolerance: f64,
}

/// A variable of a formula and the axis it reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub name: String,
    pub usage: Usage,
    /// How the value is normalized: by the usage, or as configured.
    pub kind: AxisKind,
}

/// Errors while reading a configuration.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    Toml(toml::de::Error),
    InvalidTolerance(f64),
    InvalidVariable {
        device: usize,
        usage: Usage,
        name: String,
    },
    InvalidEvdev {
        device: usize,
        value: String,
    },
    KeptEvdevWithValues {
        device: usize,
    },
    UnknownUsage {
        device: usize,
        usage: String,
    },
    DuplicateAxis {
        device: usize,
        usage: Usage,
    },
    Formula {
        device: usize,
        usage: Usage,
        error: formula::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Toml(e) => write!(f, "{e}"),
            Error::InvalidTolerance(t) => write!(f, "tolerance {t} is negative"),
            Error::InvalidVariable {
                device,
                usage,
                name: var,
            } => write!(
                f,
                "device {device}, axis {}: {var:?} is not a valid variable name, use a \
                 name made of letters, digits, and underscores that is no function or constant",
                name(*usage)
            ),
            Error::InvalidEvdev { device, value } => {
                write!(
                    f,
                    "device {device}: evdev = {value:?}, only \"keep\" is known"
                )
            }
            Error::KeptEvdevWithValues { device } => write!(
                f,
                "device {device}: evdev = \"keep\" leaves no room for [device.fuzz] or [device.flat]"
            ),
            Error::UnknownUsage { device, usage } => {
                write!(f, "device {device}: unknown usage {usage:?}")
            }
            Error::DuplicateAxis { device, usage } => {
                write!(f, "device {device}: axis {} is listed twice", name(*usage))
            }
            Error::Formula {
                device,
                usage,
                error,
            } => write!(f, "device {device}, axis {}: {error}", name(*usage)),
        }
    }
}

impl std::error::Error for Error {}

fn name(u: Usage) -> String {
    usage::name(u).map_or(u.to_string(), str::to_string)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    tolerance: Option<f64>,
    #[serde(default)]
    constants: HashMap<String, f64>,
    #[serde(default)]
    device: Vec<RawDevice>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDevice {
    name: Option<String>,
    vendor: u16,
    product: u16,
    uniq: Option<String>,
    evdev: Option<String>,
    #[serde(default)]
    fuzz: BTreeMap<String, u32>,
    #[serde(default)]
    flat: BTreeMap<String, u32>,
    #[serde(default)]
    axis: Vec<RawAxis>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAxis {
    usage: String,
    curve: String,
    kind: Option<RawKind>,
    tolerance: Option<f64>,
    source: Option<BTreeMap<String, RawSource>>,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum RawSource {
    Usage(String),
    Table(RawSourceTable),
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceTable {
    usage: String,
    kind: Option<RawKind>,
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn kind_of(raw: Option<RawKind>) -> Option<AxisKind> {
    raw.map(|k| match k {
        RawKind::Centered => AxisKind::Centered,
        RawKind::Unipolar => AxisKind::Unipolar,
    })
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RawKind {
    Centered,
    Unipolar,
}

fn tolerance(t: f64) -> Result<f64, Error> {
    if t.is_nan() || t < 0.0 {
        return Err(Error::InvalidTolerance(t));
    }
    Ok(t)
}

impl Config {
    /// Parses a configuration from TOML text.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let raw: RawConfig = toml::from_str(text).map_err(Error::Toml)?;
        let global = tolerance(raw.tolerance.unwrap_or(curve::DEFAULT_TOLERANCE))?;

        let mut devices = Vec::new();
        let mut warnings = Vec::new();
        for (index, dev) in raw.device.into_iter().enumerate() {
            let device = index + 1;
            let mut axes: Vec<Axis> = Vec::new();
            for axis in dev.axis {
                let usage = usage::parse(&axis.usage).map_err(|_| Error::UnknownUsage {
                    device,
                    usage: axis.usage.clone(),
                })?;
                if axes.iter().any(|a| a.usage == usage) {
                    return Err(Error::DuplicateAxis { device, usage });
                }
                let kind = kind_of(axis.kind).unwrap_or_else(|| usage::default_kind(usage));
                let sources = match axis.source.clone() {
                    None => vec![Source {
                        name: "x".to_string(),
                        usage,
                        kind,
                    }],
                    Some(table) => {
                        let mut sources = Vec::new();
                        for (name, src) in table {
                            if !is_identifier(&name)
                                || formula::is_function(&name)
                                || raw.constants.contains_key(&name)
                            {
                                return Err(Error::InvalidVariable {
                                    device,
                                    usage,
                                    name,
                                });
                            }
                            let (src_usage, src_kind) = match src {
                                RawSource::Usage(u) => (u, None),
                                RawSource::Table(t) => (t.usage, kind_of(t.kind)),
                            };
                            let src_usage =
                                usage::parse(&src_usage).map_err(|_| Error::UnknownUsage {
                                    device,
                                    usage: src_usage.clone(),
                                })?;
                            sources.push(Source {
                                name,
                                usage: src_usage,
                                kind: src_kind.unwrap_or_else(|| usage::default_kind(src_usage)),
                            });
                        }
                        sources
                    }
                };
                // Constant curves like "0" do not need the implicit x.
                let explicit = axis.source.is_some();
                let names: Vec<&str> = sources.iter().map(|s| s.name.as_str()).collect();
                let formula_error = |error| Error::Formula {
                    device,
                    usage,
                    error,
                };
                let curve = MultiFormula::parse(&axis.curve, &raw.constants, &names)
                    .map_err(formula_error)?;
                let terms = curve.terms().map_err(formula_error)?;
                for (src, used) in sources.iter().zip(curve.used()) {
                    if explicit && !used {
                        warnings.push(format!(
                            "device {device}, axis {}: variable {} is not used",
                            name(usage),
                            src.name
                        ));
                    }
                }
                let tolerance = match axis.tolerance {
                    Some(t) => tolerance(t)?,
                    None => global,
                };
                axes.push(Axis {
                    usage,
                    kind,
                    curve,
                    sources,
                    terms,
                    tolerance,
                });
            }
            let keep_evdev = match dev.evdev.as_deref() {
                None => false,
                Some("keep") => true,
                Some(value) => {
                    return Err(Error::InvalidEvdev {
                        device,
                        value: value.to_string(),
                    });
                }
            };
            if keep_evdev && !(dev.fuzz.is_empty() && dev.flat.is_empty()) {
                return Err(Error::KeptEvdevWithValues { device });
            }
            devices.push(Device {
                name: dev.name,
                vendor: dev.vendor,
                product: dev.product,
                uniq: dev.uniq,
                keep_evdev,
                fuzz: dev.fuzz,
                flat: dev.flat,
                axes,
                origin: None,
            });
        }
        Ok(Self { devices, warnings })
    }

    /// The first device entry that matches a device. An entry without a
    /// unique ID matches every device with its vendor and product ID.
    pub fn find(&self, vendor: u16, product: u16, uniq: &str) -> Option<&Device> {
        self.devices.iter().find(|d| {
            d.vendor == vendor
                && d.product == product
                && d.uniq.as_deref().is_none_or(|u| u == uniq)
        })
    }
}
