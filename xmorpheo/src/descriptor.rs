// SPDX-License-Identifier: GPL-2.0-or-later

//! Parser for HID report descriptors.
//!
//! The parser extracts the variable input fields of a report descriptor, so
//! the loader can tell the BPF program where each axis sits in a report.

use std::collections::HashMap;
use std::fmt;

/// A HID usage, made of the usage page and the usage ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Usage {
    pub page: u16,
    pub id: u16,
}

impl Usage {
    pub const fn new(page: u16, id: u16) -> Self {
        Self { page, id }
    }
}

impl fmt::Display for Usage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}:{:04x}", self.page, self.id)
    }
}

/// One variable input field of a report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub usage: Usage,
    /// Report ID, or 0 if the descriptor uses no report IDs.
    pub report_id: u8,
    /// Bit offset from the start of the report, including the report ID
    /// byte if the descriptor uses report IDs. This is the layout the BPF
    /// program sees.
    pub bit_offset: u32,
    pub bit_size: u32,
    pub logical_min: i32,
    pub logical_max: i32,
    /// Raw flags of the input main item.
    pub flags: u32,
}

impl Field {
    /// Whether values of this field use two's complement.
    pub fn is_signed(&self) -> bool {
        self.logical_min < 0
    }

    /// Whether this field reports relative values, such as a mouse.
    pub fn is_relative(&self) -> bool {
        self.flags & FLAG_RELATIVE != 0
    }
}

/// Errors while parsing a report descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The descriptor ends inside an item.
    Truncated { offset: usize },
    /// A Pop item without a matching Push item.
    PopWithoutPush { offset: usize },
    /// The report size of a variable input field is zero or larger than
    /// 32 bits.
    InvalidReportSize { offset: usize, size: u32 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated { offset } => {
                write!(f, "descriptor truncated in item at offset {offset}")
            }
            Error::PopWithoutPush { offset } => {
                write!(f, "pop without push at offset {offset}")
            }
            Error::InvalidReportSize { offset, size } => {
                write!(f, "invalid report size {size} at offset {offset}")
            }
        }
    }
}

impl std::error::Error for Error {}

const FLAG_CONSTANT: u32 = 1 << 0;
const FLAG_VARIABLE: u32 = 1 << 1;
const FLAG_RELATIVE: u32 = 1 << 2;

const TYPE_MAIN: u8 = 0;
const TYPE_GLOBAL: u8 = 1;
const TYPE_LOCAL: u8 = 2;

const MAIN_INPUT: u8 = 0x8;
const MAIN_OUTPUT: u8 = 0x9;
const MAIN_FEATURE: u8 = 0xb;

const GLOBAL_USAGE_PAGE: u8 = 0x0;
const GLOBAL_LOGICAL_MIN: u8 = 0x1;
const GLOBAL_LOGICAL_MAX: u8 = 0x2;
const GLOBAL_REPORT_SIZE: u8 = 0x7;
const GLOBAL_REPORT_ID: u8 = 0x8;
const GLOBAL_REPORT_COUNT: u8 = 0x9;
const GLOBAL_PUSH: u8 = 0xa;
const GLOBAL_POP: u8 = 0xb;

const LOCAL_USAGE: u8 = 0x0;
const LOCAL_USAGE_MIN: u8 = 0x1;
const LOCAL_USAGE_MAX: u8 = 0x2;

/// Upper bound for usages expanded from a usage range, to keep broken
/// descriptors from allocating huge lists.
const MAX_USAGE_RANGE: u32 = 1024;

#[derive(Clone, Default)]
struct Globals {
    usage_page: u16,
    logical_min: i32,
    logical_max_raw: u32,
    logical_max_signed: i32,
    report_size: u32,
    report_id: u8,
    report_count: u32,
}

impl Globals {
    /// Linux interprets the logical maximum as unsigned if the logical
    /// minimum is not negative, because many descriptors encode values
    /// like 255 as `25 ff`.
    fn logical_max(&self) -> i32 {
        if self.logical_min < 0 {
            self.logical_max_signed
        } else {
            self.logical_max_raw as i32
        }
    }
}

#[derive(Default)]
struct Locals {
    usages: Vec<Usage>,
    usage_min: Option<u32>,
}

impl Locals {
    /// Usages without a page take the current usage page. Linux resolves
    /// the page when the main item is parsed, and so do we.
    fn add_usage(&mut self, data: u32, size: usize, page: u16) {
        self.usages.push(Self::resolve(data, size, page));
    }

    fn resolve(data: u32, size: usize, page: u16) -> Usage {
        if size == 4 {
            Usage::new((data >> 16) as u16, data as u16)
        } else {
            Usage::new(page, data as u16)
        }
    }
}

/// A parsed report descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    /// Variable input fields in descriptor order.
    pub fields: Vec<Field>,
    /// Length of each input report in bytes, including the report ID byte
    /// if the descriptor uses report IDs, by report ID in ascending order.
    pub input_reports: Vec<(u8, u32)>,
}

impl Descriptor {
    /// The length of the input report with this ID, in bytes.
    pub fn input_report_len(&self, report_id: u8) -> Option<u32> {
        self.input_reports
            .iter()
            .find(|(id, _)| *id == report_id)
            .map(|(_, len)| *len)
    }
}

/// Parses a report descriptor and returns its variable input fields in
/// descriptor order.
///
/// Constant fields (padding) and array fields only advance the bit offset.
pub fn parse(desc: &[u8]) -> Result<Vec<Field>, Error> {
    Ok(parse_descriptor(desc)?.fields)
}

/// Parses a report descriptor into its variable input fields and the
/// lengths of its input reports.
pub fn parse_descriptor(desc: &[u8]) -> Result<Descriptor, Error> {
    let mut fields = Vec::new();
    let mut globals = Globals::default();
    let mut stack: Vec<Globals> = Vec::new();
    let mut locals = Locals::default();
    // Bit offsets per (main item tag, report ID).
    let mut offsets: HashMap<(u8, u8), u32> = HashMap::new();
    let mut uses_report_ids = false;

    let mut pos = 0;
    while pos < desc.len() {
        let start = pos;
        let prefix = desc[pos];
        pos += 1;

        // Long items carry no information we need.
        if prefix == 0xfe {
            let size = *desc.get(pos).ok_or(Error::Truncated { offset: start })? as usize;
            pos += 2 + size;
            if pos > desc.len() {
                return Err(Error::Truncated { offset: start });
            }
            continue;
        }

        let size = match prefix & 0x3 {
            3 => 4,
            n => n as usize,
        };
        let kind = (prefix >> 2) & 0x3;
        let tag = prefix >> 4;
        let bytes = desc
            .get(pos..pos + size)
            .ok_or(Error::Truncated { offset: start })?;
        pos += size;

        let mut udata: u32 = 0;
        for (i, b) in bytes.iter().enumerate() {
            udata |= (*b as u32) << (8 * i);
        }
        let sdata: i32 = match size {
            1 => udata as u8 as i8 as i32,
            2 => udata as u16 as i16 as i32,
            4 => udata as i32,
            _ => 0,
        };

        match kind {
            TYPE_MAIN => {
                if matches!(tag, MAIN_INPUT | MAIN_OUTPUT | MAIN_FEATURE) {
                    let size = globals.report_size;
                    let count = globals.report_count;
                    let base = if uses_report_ids { 8 } else { 0 };
                    let offset = offsets.entry((tag, globals.report_id)).or_insert(base);
                    let variable = udata & (FLAG_CONSTANT | FLAG_VARIABLE) == FLAG_VARIABLE;

                    if tag == MAIN_INPUT && variable && count > 0 {
                        if size == 0 || size > 32 {
                            return Err(Error::InvalidReportSize {
                                offset: start,
                                size,
                            });
                        }
                        let usages = expand_usages(&locals, globals.usage_page);
                        for i in 0..count {
                            // Linux repeats the last usage for the remaining fields.
                            let Some(usage) = usages.get(i as usize).or(usages.last()) else {
                                break;
                            };
                            fields.push(Field {
                                usage: *usage,
                                report_id: globals.report_id,
                                bit_offset: *offset + i * size,
                                bit_size: size,
                                logical_min: globals.logical_min,
                                logical_max: globals.logical_max(),
                                flags: udata,
                            });
                        }
                    }
                    *offset += count * size;
                }
                locals = Locals::default();
            }
            TYPE_GLOBAL => match tag {
                GLOBAL_USAGE_PAGE => globals.usage_page = udata as u16,
                GLOBAL_LOGICAL_MIN => globals.logical_min = sdata,
                GLOBAL_LOGICAL_MAX => {
                    globals.logical_max_raw = udata;
                    globals.logical_max_signed = sdata;
                }
                GLOBAL_REPORT_SIZE => globals.report_size = udata,
                GLOBAL_REPORT_ID => {
                    globals.report_id = udata as u8;
                    uses_report_ids = true;
                }
                GLOBAL_REPORT_COUNT => globals.report_count = udata,
                GLOBAL_PUSH => stack.push(globals.clone()),
                GLOBAL_POP => {
                    globals = stack.pop().ok_or(Error::PopWithoutPush { offset: start })?;
                }
                _ => {}
            },
            TYPE_LOCAL => match tag {
                LOCAL_USAGE => locals.add_usage(udata, size, globals.usage_page),
                LOCAL_USAGE_MIN => locals.usage_min = Some(udata),
                LOCAL_USAGE_MAX => {
                    if let Some(min) = locals.usage_min.take() {
                        let min_usage = Locals::resolve(min, size.max(1), globals.usage_page);
                        let max_usage = Locals::resolve(udata, size, globals.usage_page);
                        let last = (max_usage.id as u32).min(min_usage.id as u32 + MAX_USAGE_RANGE);
                        for id in min_usage.id as u32..=last {
                            locals.usages.push(Usage::new(min_usage.page, id as u16));
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    let mut input_reports: Vec<(u8, u32)> = offsets
        .iter()
        .filter(|((tag, _), _)| *tag == MAIN_INPUT)
        .map(|((_, id), bits)| (*id, bits.div_ceil(8)))
        .collect();
    input_reports.sort_unstable();
    Ok(Descriptor {
        fields,
        input_reports,
    })
}

fn expand_usages(locals: &Locals, page: u16) -> Vec<Usage> {
    if locals.usages.is_empty() {
        // A field without any usage still occupies its bits. Report it with
        // usage 0, so callers can see it but will not match it by name.
        vec![Usage::new(page, 0)]
    } else {
        locals.usages.clone()
    }
}
