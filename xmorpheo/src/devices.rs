// SPDX-License-Identifier: GPL-2.0-or-later

//! Commands that work on connected HID devices.

use std::error::Error;
use std::path::Path;

use xmorpheo::confd;
use xmorpheo::config::{self, Config};
use xmorpheo::curve::AxisKind;
use xmorpheo::descriptor::{self, Usage};
use xmorpheo::plan::{self, Plan};
use xmorpheo::sysfs::{self, HidDevice};
use xmorpheo::{bpf, evdev, loader, usage};

/// Reads the configuration: from a file or one directory, or without a
/// path from all drop-in directories. Warnings go to stderr.
pub fn read_config(path: Option<&Path>) -> Result<Config, Box<dyn Error>> {
    let (config, _) = confd::load(path)?;
    for w in &config.warnings {
        eprintln!("xmorpheo: warning: {w}");
    }
    Ok(config)
}

/// `xmorpheo config`: the drop-in files and what became of them, the
/// warnings, and the active files, like `systemd-analyze cat-config`.
pub fn show_config(path: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let (config, entries) = confd::load(path)?;
    if path.is_none() {
        println!("# drop-in directories, from the highest priority:");
        for dir in confd::dirs() {
            println!("#   {}", dir.display());
        }
    }
    if entries.is_empty() {
        println!("# no configuration files");
    }
    for entry in &entries {
        match entry {
            confd::Entry::Active(p) => println!("# active:     {}", p.display()),
            confd::Entry::Overridden { path, by } => {
                println!("# overridden: {} by {}", path.display(), by.display())
            }
            confd::Entry::Masked { path, by } if path == by => {
                println!("# masking:    {}", path.display())
            }
            confd::Entry::Masked { path, by } => {
                println!("# masked:     {} by {}", path.display(), by.display())
            }
        }
    }
    for w in &config.warnings {
        println!("# warning: {w}");
    }
    for entry in &entries {
        if let confd::Entry::Active(p) = entry {
            println!("\n# {}", p.display());
            print!("{}", std::fs::read_to_string(p)?);
        }
    }
    Ok(())
}

/// The devices that the selectors name, or all devices without selectors.
pub fn select(selectors: &[String]) -> Result<Vec<HidDevice>, Box<dyn Error>> {
    let all = sysfs::list(Path::new(sysfs::HID_DEVICES))
        .map_err(|e| format!("{}: {e}", sysfs::HID_DEVICES))?;
    if selectors.is_empty() {
        return Ok(all);
    }
    let mut selected: Vec<HidDevice> = Vec::new();
    for sel in selectors {
        let found: Vec<_> = all.iter().filter(|d| d.matches(sel)).collect();
        if found.is_empty() {
            return Err(format!("no HID device matches {sel:?}").into());
        }
        for dev in found {
            if !selected.iter().any(|d| d.hid_id == dev.hid_id) {
                selected.push(dev.clone());
            }
        }
    }
    Ok(selected)
}

fn yes_no(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "yes",
        Some(false) => "no",
        None => "?",
    }
}

/// `xmorpheo list`
pub fn list(config: &Config) -> Result<(), Box<dyn Error>> {
    println!("hid_id  device               attached  name  [configuration]");
    for dev in select(&[])? {
        let origin = config
            .find(dev.vendor, dev.product, &dev.uniq)
            .map(|d| match &d.origin {
                Some(p) => format!(" [{}]", p.display()),
                None => " [configured]".to_string(),
            })
            .unwrap_or_default();
        let mut name = dev.name.clone();
        if !dev.uniq.is_empty() {
            name = format!("{name} ({})", dev.uniq);
        }
        println!(
            "{:>6}  {:<19}  {:<8}  {name}{origin}",
            dev.hid_id,
            dev.sysname,
            yes_no(loader::is_attached(dev.hid_id)),
        );
    }
    Ok(())
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

const HAT_SWITCH: Usage = Usage::new(0x01, 0x39);

/// The configuration template for one device, with all axes commented out.
pub fn template(dev: &HidDevice, desc: &[u8]) -> Result<String, Box<dyn Error>> {
    let desc = descriptor::parse_descriptor(desc)?;
    let mut out = String::new();
    out.push_str("[[device]]\n");
    out.push_str(&format!("name = {}\n", quote(&dev.name)));
    out.push_str(&format!("vendor = 0x{:04x}\n", dev.vendor));
    out.push_str(&format!("product = 0x{:04x}\n", dev.product));
    if dev.uniq.is_empty() {
        out.push_str("# uniq = \"...\"        # only needed for identical devices\n");
    } else {
        out.push_str(&format!("# uniq = {}\n", quote(&dev.uniq)));
    }
    out.push_str("# [device.flat]       # evdev flat by axis, 0 by default\n");
    out.push_str("# ABS_X = 0\n");

    let mut seen: Vec<Usage> = Vec::new();
    for f in &desc.fields {
        if f.bit_size < 2 || f.is_relative() || f.usage.id == 0 || f.usage == HAT_SWITCH {
            continue;
        }
        if seen.contains(&f.usage) {
            continue;
        }
        seen.push(f.usage);
        let name = usage::name(f.usage).map_or(f.usage.to_string(), str::to_string);
        let kind = kind_name(usage::default_kind(f.usage));
        out.push_str(&format!(
            "\n# {name}: {} bits at bit {} of report {}, {}..={}, {kind} by default\n",
            f.bit_size, f.bit_offset, f.report_id, f.logical_min, f.logical_max
        ));
        out.push_str("# [[device.axis]]\n");
        out.push_str(&format!("# usage = {}\n", quote(&name)));
        out.push_str("# curve = \"x\"\n");
    }
    Ok(out)
}

/// `xmorpheo template`
pub fn templates(selectors: &[String]) -> Result<(), Box<dyn Error>> {
    println!("# SPDX-License-Identifier: CC0-1.0");
    println!("#");
    println!("# xmorpheo configuration, written by xmorpheo template. Uncomment");
    println!("# the axes to rewrite and set their curves, see docs/DESIGN.md.");
    for dev in select(selectors)? {
        let Ok(desc) = dev.report_descriptor() else {
            continue;
        };
        let text = template(&dev, &desc)?;
        // Without explicit selectors, skip devices without axes.
        if selectors.is_empty() && !text.contains("[[device.axis]]") {
            continue;
        }
        println!("\n# {} ({})", dev.sysname, dev.name);
        print!("{text}");
    }
    Ok(())
}

fn kind_name(kind: AxisKind) -> &'static str {
    match kind {
        AxisKind::Centered => "centered",
        AxisKind::Unipolar => "unipolar",
    }
}

fn print_plan(dev: &HidDevice, entry: &config::Device, plan: &Plan) {
    println!(
        "{} (hid_id {}): {}",
        dev.sysname,
        dev.hid_id,
        entry.name.as_deref().unwrap_or(&dev.name)
    );
    println!("  usage        report  length  offset  bits  range          kind       formula");
    for a in &plan.axes {
        let name = usage::name(a.usage).map_or(a.usage.to_string(), str::to_string);
        println!(
            "  {:<11}  {:>6}  {:>6}  {:>6}  {:>4}  {:<13}  {:<9}  {}",
            name,
            a.report_id,
            a.report_len,
            a.bit_offset,
            a.bit_size,
            format!("{}..={}", a.min, a.max),
            kind_name(a.kind),
            a.formula
        );
        for t in &a.terms {
            let source = usage::name(t.source).map_or(t.source.to_string(), str::to_string);
            println!(
                "    {} = {source} ({}): curve {}, {} knots: {}",
                t.var,
                kind_name(t.kind),
                t.curve_index,
                t.knots,
                t.curve
            );
        }
    }
    println!(
        "  fields {}/{}, outputs {}/{}, terms {}/{}, curves {}/{}, knots {}/{}",
        plan.fields.len(),
        bpf::MAX_FIELDS,
        plan.outputs.len(),
        bpf::MAX_OUTPUTS,
        plan.terms.len(),
        bpf::MAX_TERMS,
        plan.curves.len(),
        bpf::MAX_CURVES,
        plan.knots.len(),
        bpf::MAX_KNOTS
    );
}

/// `xmorpheo attach`
pub fn attach(
    config: &Config,
    selectors: &[String],
    all: bool,
    dry_run: bool,
    replace: bool,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    if selectors.is_empty() && !all {
        return Err("name the devices to attach to, or use --all".into());
    }
    let devices: Vec<_> = select(selectors)?
        .into_iter()
        .filter(|d| !all || config.find(d.vendor, d.product, &d.uniq).is_some())
        .collect();
    if devices.is_empty() {
        return Err("the configuration matches no connected device".into());
    }
    if !dry_run {
        loader::preflight()?;
    }

    let mut failed = 0;
    for dev in &devices {
        if let Err(e) = attach_one(config, dev, dry_run, replace, verbose) {
            eprintln!("xmorpheo: {}: {e}", dev.sysname);
            failed += 1;
        }
    }
    if failed > 0 {
        return Err(format!("{failed} of {} devices failed", devices.len()).into());
    }
    Ok(())
}

fn attach_one(
    config: &Config,
    dev: &HidDevice,
    dry_run: bool,
    replace: bool,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let entry = config
        .find(dev.vendor, dev.product, &dev.uniq)
        .ok_or("the configuration has no entry for this device")?;
    let desc = descriptor::parse_descriptor(&dev.report_descriptor()?)?;
    let plan = plan::plan(entry, &desc)?;
    if dry_run || verbose {
        print_plan(dev, entry, &plan);
    }
    let nodes = dev.event_nodes()?;

    if dry_run {
        let mut present = Vec::new();
        if entry.keep_evdev {
            println!("  would leave evdev fuzz and flat as the driver set them");
        }
        for node in nodes.iter().filter(|_| !entry.keep_evdev) {
            let axes = evdev::axes(node).unwrap_or_default();
            if axes.is_empty() {
                continue;
            }
            let names: Vec<_> = axes.iter().map(|(c, _)| evdev::abs_name(*c)).collect();
            println!(
                "  would set evdev fuzz and flat on {}: {}",
                node.display(),
                names.join(" ")
            );
            present.extend(axes.iter().map(|(c, _)| *c));
        }
        for (what, table) in [("fuzz", &entry.fuzz), ("flat", &entry.flat)] {
            for (name, value) in table {
                println!("  {what} {name} = {value}");
            }
        }
        for w in evdev_warnings(entry, &present) {
            println!("  {w}");
        }
        println!("  dry run, nothing loaded");
        return Ok(());
    }

    let (pin, replaced) = loader::attach(dev.hid_id, &plan, replace)?;
    let what = if replaced { "replaced" } else { "attached" };
    let evdev = set_evdev(dev, entry);
    if verbose {
        println!("  {what}, link pinned at {}", pin.display());
        println!("  evdev: {evdev}");
    } else {
        println!(
            "{} (hid_id {}): {what}, {} fields, {} knots, evdev: {evdev}",
            entry.name.as_deref().unwrap_or(&dev.name),
            dev.hid_id,
            plan.fields.len(),
            plan.knots.len()
        );
    }
    Ok(())
}

/// Sets the evdev fuzz and flat of all axes of a device to the configured
/// values, 0 by default, and describes the result. Failures and unknown
/// axis names are part of the description, because the program is
/// attached either way.
pub fn set_evdev(dev: &HidDevice, entry: &config::Device) -> String {
    if entry.keep_evdev {
        return "fuzz and flat left as the driver set them".to_string();
    }
    let lookup = |table: &std::collections::BTreeMap<String, u32>, code: u16| {
        table
            .iter()
            .find(|(name, _)| evdev::abs_code(name) == Some(code))
            .map_or(0, |(_, v)| *v as i32)
    };
    let mut notes = Vec::new();
    let mut seen: Vec<u16> = Vec::new();
    match dev.event_nodes() {
        Ok(nodes) if !nodes.is_empty() => {
            for node in nodes {
                match evdev::set_fuzz_and_flat(&node, |code| {
                    (lookup(&entry.fuzz, code), lookup(&entry.flat, code))
                }) {
                    Ok((codes, 0)) if codes.is_empty() => {}
                    Ok((codes, 0)) => {
                        notes.push(format!(
                            "{}: fuzz and flat of {} axes already as configured",
                            node.display(),
                            codes.len()
                        ));
                        seen.extend(codes);
                    }
                    Ok((codes, n)) => {
                        notes.push(format!(
                            "{}: set fuzz and flat of {n} of {} axes",
                            node.display(),
                            codes.len()
                        ));
                        seen.extend(codes);
                    }
                    Err(e) => notes.push(format!("{}: {e}", node.display())),
                }
            }
        }
        Ok(_) => notes.push("no evdev nodes".to_string()),
        Err(e) => notes.push(format!("no evdev nodes: {e}")),
    }
    notes.extend(evdev_warnings(entry, &seen));
    notes.join(", ")
}

/// Warnings for configured evdev axes that are unknown or that the device
/// does not have.
fn evdev_warnings(entry: &config::Device, present: &[u16]) -> Vec<String> {
    let mut warnings = Vec::new();
    for name in entry.fuzz.keys().chain(entry.flat.keys()) {
        match evdev::abs_code(name) {
            None => warnings.push(format!("warning: unknown evdev axis {name}")),
            Some(code) if !present.contains(&code) => {
                warnings.push(format!("warning: the device has no evdev axis {name}"))
            }
            Some(_) => {}
        }
    }
    warnings.dedup();
    warnings
}

/// `xmorpheo detach`
pub fn detach(selectors: &[String], all: bool) -> Result<(), Box<dyn Error>> {
    let ids: Vec<u32> = if all {
        match std::fs::read_dir(loader::PIN_DIR) {
            Ok(entries) => {
                // A temporary pin "<hid_id>.new" of an interrupted replace
                // belongs to its device too.
                let mut ids: Vec<u32> = entries
                    .filter_map(|e| {
                        let name = e.ok()?.file_name().into_string().ok()?;
                        name.trim_end_matches(".new").parse().ok()
                    })
                    .collect();
                ids.sort_unstable();
                ids.dedup();
                ids
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("{}: {e}", loader::PIN_DIR).into()),
        }
    } else if selectors.is_empty() {
        return Err("name the devices to detach from, or use --all".into());
    } else {
        select(selectors)?.iter().map(|d| d.hid_id).collect()
    };
    for id in ids {
        match loader::detach(id) {
            Ok(true) => println!("hid_id {id}: detached"),
            Ok(false) => println!("hid_id {id}: not attached"),
            Err(e) => return Err(format!("hid_id {id}: {e}").into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
