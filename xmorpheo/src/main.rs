// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

mod devices;
mod plot;
mod udev;

use xmorpheo::curve::{self, AxisKind, Field, ONE};
use xmorpheo::formula::Formula;
use xmorpheo::{descriptor, usage};

/// Response curves for HID joysticks and gamepads using HID-BPF.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lists the HID devices, whether the configuration has an entry for
    /// them, and whether a program is attached.
    List {
        /// A configuration file or directory instead of the drop-in
        /// directories.
        #[arg(long, short)]
        config: Option<PathBuf>,
    },
    /// Shows the configuration drop-ins, which are active, overridden, or
    /// masked, and the contents of the active ones.
    Config {
        /// A configuration file or directory instead of the drop-in
        /// directories.
        #[arg(long, short)]
        config: Option<PathBuf>,
    },
    /// Attaches the curves of the configuration to devices. Needs root,
    /// except with --dry-run.
    Attach {
        /// A configuration file or directory instead of the drop-in
        /// directories.
        #[arg(long, short)]
        config: Option<PathBuf>,
        /// Devices by directory name, hid_id, or VENDOR:PRODUCT.
        devices: Vec<String>,
        /// Attach to all connected devices that the configuration names.
        #[arg(long, conflicts_with = "devices")]
        all: bool,
        /// Show the field table and knot counts without loading anything.
        #[arg(long)]
        dry_run: bool,
        /// Replace programs that are already attached. The old program
        /// stays when the new one cannot be prepared.
        #[arg(long)]
        replace: bool,
        /// Print the field table of each device, and the libbpf debug
        /// log, including the verifier log.
        #[arg(long, short)]
        verbose: bool,
    },
    /// Run by the udev rule: attaches to a configured device when a driver
    /// binds to it, and removes the pin when it goes away.
    #[command(hide = true)]
    Udev {
        /// bind or remove.
        action: String,
        /// The device directory name, like 0003:231D:0200.000A.
        sysname: String,
    },
    /// Detaches the program from devices. Needs root.
    Detach {
        /// Devices by directory name, hid_id, or VENDOR:PRODUCT.
        devices: Vec<String>,
        /// Detach from all devices.
        #[arg(long, conflicts_with = "devices")]
        all: bool,
    },
    /// Prints a configuration template for connected devices, with their
    /// axes commented out.
    Template {
        /// Devices by directory name, hid_id, or VENDOR:PRODUCT. All
        /// devices with axes without one.
        devices: Vec<String>,
    },
    /// Lists the input fields of a HID report descriptor, for example
    /// /sys/bus/hid/devices/<device>/report_descriptor.
    Descriptor {
        path: PathBuf,
        /// Also list one bit fields, such as buttons.
        #[arg(long)]
        all: bool,
    },
    /// Fits the knots of a curve formula for a field, and prints them
    /// together with sample values.
    Curve {
        formula: String,
        /// Unsigned field with this many bits.
        #[arg(long, default_value_t = 12, conflicts_with = "range")]
        bits: u32,
        /// Logical range of the field, as MIN..MAX.
        #[arg(long, value_parser = parse_range)]
        range: Option<(i32, i32)>,
        #[arg(long, value_enum, default_value_t = Kind::Centered)]
        kind: Kind,
        /// Fitting tolerance in least significant bits.
        #[arg(long, default_value_t = curve::DEFAULT_TOLERANCE)]
        tolerance: f64,
        /// A named constant for the formula, as NAME=VALUE.
        #[arg(long = "constant", short = 'c', value_parser = parse_constant)]
        constants: Vec<(String, f64)>,
        /// Output format: the knots and sample values, every value as CSV,
        /// or a gnuplot script.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        /// With --format csv, also print the knots after the values.
        #[arg(long)]
        knots: bool,
        /// Write to this file instead of standard output.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        /// With --format gnuplot, let the script render an image with this
        /// terminal instead of showing the plot.
        #[arg(long, value_enum, requires = "output_image")]
        terminal: Option<Terminal>,
        /// The image the script renders with --terminal.
        #[arg(long, requires = "terminal")]
        output_image: Option<String>,
    },
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum Format {
    Text,
    Csv,
    Gnuplot,
}

#[derive(Clone, Copy, ValueEnum)]
enum Terminal {
    Png,
    Svg,
}

/// How `xmorpheo curve` prints the curve.
struct CurveOutput {
    format: Format,
    knots: bool,
    output: Option<PathBuf>,
    image: Option<(&'static str, String)>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Kind {
    Centered,
    Unipolar,
}

fn parse_range(s: &str) -> Result<(i32, i32), String> {
    let (min, max) = s
        .split_once("..=")
        .or_else(|| s.split_once(".."))
        .ok_or("expected MIN..MAX")?;
    let min = min.parse().map_err(|e| format!("minimum: {e}"))?;
    let max = max.parse().map_err(|e| format!("maximum: {e}"))?;
    Ok((min, max))
}

fn parse_constant(s: &str) -> Result<(String, f64), String> {
    let (name, value) = s.split_once('=').ok_or("expected NAME=VALUE")?;
    let value = value.parse().map_err(|e| format!("value: {e}"))?;
    Ok((name.to_string(), value))
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xmorpheo: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Command::List { config } => devices::list(&devices::read_config(config.as_deref())?),
        Command::Config { config } => devices::show_config(config.as_deref()),
        Command::Template { devices } => devices::templates(&devices),
        Command::Attach {
            config,
            devices,
            all,
            dry_run,
            replace,
            verbose,
        } => {
            if verbose {
                libbpf_rs::set_print(Some((libbpf_rs::PrintLevel::Debug, |_, msg| {
                    eprint!("{msg}")
                })));
            }
            let config = devices::read_config(config.as_deref())?;
            devices::attach(&config, &devices, all, dry_run, replace, verbose)
        }
        Command::Detach { devices, all } => devices::detach(&devices, all),
        Command::Udev { action, sysname } => udev::run(&action, &sysname),
        Command::Descriptor { path, all } => {
            let desc = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            print_descriptor(&desc, all)
        }
        Command::Curve {
            formula,
            bits,
            range,
            kind,
            tolerance,
            constants,
            format,
            knots,
            output,
            terminal,
            output_image,
        } => {
            if knots && format != Format::Csv {
                return Err("--knots needs --format csv".into());
            }
            if terminal.is_some() && format != Format::Gnuplot {
                return Err("--terminal needs --format gnuplot".into());
            }
            let image = terminal.zip(output_image).map(|(t, file)| {
                let t = match t {
                    Terminal::Png => "pngcairo",
                    Terminal::Svg => "svg",
                };
                (t, file)
            });
            let out = CurveOutput {
                format,
                knots,
                output,
                image,
            };
            let kind = match kind {
                Kind::Centered => AxisKind::Centered,
                Kind::Unipolar => AxisKind::Unipolar,
            };
            let (min, max) = match range {
                Some(range) => range,
                None if (1..=16).contains(&bits) => (0, (1 << bits) - 1),
                None => return Err(format!("{bits} bits are not in 1..=16").into()),
            };
            let constants = constants.into_iter().collect();
            print_curve(&formula, &constants, kind, min, max, tolerance, &out)
        }
    }
}

fn print_descriptor(desc: &[u8], all: bool) -> Result<(), Box<dyn Error>> {
    let fields = descriptor::parse(desc)?;
    println!("report  usage        bits  offset  logical range");
    let mut hidden = 0;
    for f in &fields {
        if f.bit_size == 1 && !all {
            hidden += 1;
            continue;
        }
        let name = usage::name(f.usage).map_or(f.usage.to_string(), str::to_string);
        let mut notes = Vec::new();
        if f.is_relative() {
            notes.push("relative");
        }
        if f.logical_min < 0 {
            notes.push("signed");
        }
        let line = format!(
            "{:>6}  {:<11}  {:>4}  {:>6}  {}..={} {}",
            f.report_id,
            name,
            f.bit_size,
            f.bit_offset,
            f.logical_min,
            f.logical_max,
            notes.join(", ")
        );
        println!("{}", line.trim_end());
    }
    if hidden > 0 {
        println!("{hidden} one bit fields not shown, use --all to list them");
    }
    Ok(())
}

fn print_curve(
    src: &str,
    constants: &HashMap<String, f64>,
    kind: AxisKind,
    min: i32,
    max: i32,
    tolerance: f64,
    out: &CurveOutput,
) -> Result<(), Box<dyn Error>> {
    let formula = Formula::parse(src, constants)?;
    let field = Field::new(kind, min, max)?;
    let knots = curve::fit(&formula, &field, tolerance)?;

    let plot = plot::Curve {
        formula: &formula,
        field: &field,
        knots: &knots,
        tolerance,
    };
    let text = match out.format {
        Format::Text => None,
        Format::Csv => Some(plot.csv(out.knots)),
        Format::Gnuplot => Some(plot.gnuplot(out.image.as_ref().map(|(t, f)| (*t, f.as_str())))),
    };
    match (text, &out.output) {
        (Some(text), Some(path)) => {
            return std::fs::write(path, text)
                .map_err(|e| format!("{}: {e}", path.display()).into());
        }
        (Some(text), None) => {
            print!("{text}");
            return Ok(());
        }
        (None, Some(_)) => return Err("--output needs --format csv or gnuplot".into()),
        (None, None) => {}
    }

    println!("formula: {formula}");
    println!(
        "field:   {min}..={max}, {kind:?}, in_mul {}, out_mul {}",
        field.in_mul, field.out_mul
    );
    println!("knots:   {}", knots.len());
    println!();
    println!("     x (Q24)      y (Q24)   slope (Q24)        x        y");
    for k in &knots {
        println!(
            "{:>12} {:>12} {:>13} {:>8.4} {:>8.4}",
            k.x,
            k.y,
            k.slope,
            k.x as f64 / ONE as f64,
            k.y as f64 / ONE as f64
        );
    }

    println!();
    println!("       x      raw   output    exact");
    let lowest = kind.lowest() as f64 / ONE as f64;
    let steps = 8;
    for i in 0..=steps {
        let x = lowest + (1.0 - lowest) * i as f64 / steps as f64;
        let raw = field.exact_raw(x).round() as i32;
        let exact = field.exact_raw(formula.eval(field.exact_x(raw)).clamp(lowest, 1.0));
        let out = curve::eval_raw(&field, &knots, raw);
        println!("{x:>8.3} {raw:>8} {out:>8} {exact:>8.2}");
    }
    Ok(())
}

#[cfg(test)]
#[path = "main/tests.rs"]
mod tests;
