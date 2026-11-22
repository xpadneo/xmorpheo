<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Design

This document describes the architecture of xmorpheo. It covers what is built
now and keeps room for planned extensions, see [IDEAS.md](IDEAS.md).


## Goals

- Apply response curves and deadzones to selected axes of HID joysticks and
  gamepads.
- Work for every consumer of the device: evdev, SDL, HID drivers such as
  xpadneo, and `hidraw` readers such as games in Proton or Wine.
- Keep the device identity (bus, vendor ID, product ID, report descriptor)
  unchanged, so games keep their bindings.
- Keep the kernel part small, device-agnostic, and free of floating point math.


## Where xmorpheo Hooks In

```
device ──► HID core ──► HID-BPF (xmorpheo) ──┬──► hidraw ──► Proton, Wine, SDL hidapi
                                              └──► HID driver (hid-generic, xpadneo) ──► evdev
```

HID-BPF programs of type `hid_bpf_ops.hid_device_event` run on every incoming
report before the kernel passes it to `hidraw` and to the `raw_event` hook of
the HID driver. A program can modify the report in place. This needs Linux
6.11 or newer, where HID-BPF uses `struct_ops`.

The report descriptor is not changed, so the report layout stays the same.
Other HID-BPF programs that fix the descriptor, for example the ones loaded by
`udev-hid-bpf`, can attach to the same device. xmorpheo reads the descriptor
after such fixups.

### Values, Never the Descriptor

xmorpheo changes the values in reports, never the report descriptor, and
it does not plan to:

- Software expects the layout and ranges of known models. SDL with its
  HIDAPI drivers, Steam Input, and Wine and Proton with `hidraw` parse the
  reports of known controllers themselves, by model, not by descriptor.
  Games and SDL keep their mappings and bindings by vendor and product ID.
  A changed range or a new field would go unseen or be misread.
- A new descriptor reconnects the device, and to some software it is then
  another device.
- Drivers like xpadneo and hid-tmff2 fix the descriptor themselves, after
  the fixups of HID-BPF. A second fixup before theirs is fragile.
- Little is to gain: combined axes work within the range of their field,
  and lose at most one bit, like two 10 bit triggers on one 10 bit axis.

So the range of an axis is the range of its field, and buttons or axes can
only be remapped onto fields the device has.

### Report Layout Mismatches

The program sees the raw reports of the device, but the descriptor in sysfs
is the one after the `report_fixup` of the HID driver. If a driver changes
the descriptor, the fields xmorpheo finds may not match the raw reports.
As a guard, the loader stores the expected length of each input report, per
report ID, and the program leaves a report untouched when it is shorter.
Longer reports are rewritten, because some devices pad every report to the
size of their USB endpoint, like the Thrustmaster T300 RS with 64 byte
reports where its descriptor declares 15 bytes, and the HID core ignores the extra
bytes as well.

Drivers that keep the report length but move fields would still mismatch.
xpadneo, for example, splits the paddles out of the button field in its
`raw_event` hook. That only touches buttons, so axes stay where the
descriptor says. Any such driver must be checked before supporting it.


## Components

### BPF Program (`bpf/`)

The BPF program knows nothing about specific devices or formulas. It reads
five array maps, which the loader fills after loading, and the number of
fields and outputs in read-only data. `bpf/xmorpheo.h` defines their
layout and the integer math:

- **Fields**: up to 16 fields per device that the program reads and
  writes. Each holds the report ID, the expected length of that input
  report, the bit offset and bit size of the field (at most 16 bits),
  whether the field is signed, and its logical range.
- **Outputs**: up to 16 fields per device to write. Each holds the index of
  its field, whether it is centered, the multiplier from Q24 fixed point
  back to its raw range, and a slice of the term table.
- **Terms**: up to 32 per device, at most 4 per output. Each holds the
  index of its source field, how the source is normalized, the multiplier
  from its raw range to Q24, and the index of its curve. An output is the
  sum of its terms.
- **Curves**: up to 64 entries, holding the first knot and the number of
  knots, a slice of the knot pool.
- **Knots**: a pool of up to 8192 support points `{x, y, slope}` in Q24
  fixed point, sorted by `x` within each curve. The slope to the next knot
  is computed in user space, so the program needs no division.

For each input report, the program works in three bounded loops, like a
transaction: it reads all fields of the report, computes each output as
the sum of its terms from the values the device sent, then writes the
outputs. So no output depends on the order of the fields, and a field can
be the source of one output and the target of another. Fields of other
report IDs and reports shorter than expected are skipped, and an output keeps
its value if its field or any source is outside its logical range. The
computation only does integer math, with `span = max - min` and arithmetic shifts:

1. Normalize to Q24, where `1.0` is `2^24`: `x = ((raw - min) * in_mul +
   2^15) >> 16`, minus `2^24` for centered axes. `in_mul` is
   `round(2^41 / span)` for centered and `round(2^40 / span)` for unipolar
   axes.
2. Find the segment: the last knot with `knot.x <= x`, by a binary search
   of at most 10 steps over at most 1024 knots.
3. Interpolate: `y = knot.y + (((x - knot.x) * knot.slope + 2^23) >> 24)`.
   `slope` is `round((y1 - y0) * 2^24 / (x1 - x0))` to the next knot, 0 for
   the last knot, and needs 64 bits.
4. Add up the terms of the output, then clamp the sum to `-2^24..=2^24` or
   `0..=2^24`.
5. Denormalize: `raw = min + (((y + off) * out_mul + 2^31) >> 32)`, with
   `off = 2^24` and `out_mul = span * 2^7` for centered axes, and
   `off = 0` and `out_mul = span * 2^8` for unipolar axes.

Q24 leaves about 256 steps of the fixed point format per step of a 16 bit
field, so rounding inside the math stays far below the fitting tolerance.
Intermediate values are 64 bits wide, and only the final value is clamped
and rounded to the raw range. All products fit in 64 bits for fields of up
to 16 bits. The identity curve maps every raw value to itself.

Knots are normalized, so all axes that use the same formula share one curve,
independent of their resolution.

The loader creates one instance of the program per HID device, because the
`struct_ops` map binds to one `hid_id`. Instances can reuse the maps of the
first instance, so each formula exists once on the system. For now, each
instance has its own curve and knot maps.

### User-Space Tool (`xmorpheo/`)

A Rust crate with a library and the `xmorpheo` command line tool:

- **Descriptor parser**: parses the HID report descriptor from
  `/sys/bus/hid/devices/<bus>:<vid>:<pid>.<id>/report_descriptor`, and lists
  the input fields with usage, report ID, bit offset, bit size, and logical
  range, and the length of each input report.
- **Formulas**: parses curve formulas in the normalized input `x`, see
  below.
- **Curves**: evaluates a formula at every raw value of a field and fits
  knots greedily: each linear segment extends as long as the integer
  evaluation of the BPF program, before rounding to a raw value, stays
  within the tolerance of the exact formula at every raw value, or rounds
  to the same raw value. The tolerance defaults to half the least
  significant bit of the finest field that uses the curve, and can be set
  globally or per axis. Every output is within the tolerance plus half a
  step of the exact formula, so within one step by default. A tolerance of
  0 gives the exactly rounded curve, with a knot wherever needed. The error
  bound governs the accuracy, not the knot spacing. The staircase of
  integer outputs is inherent to every slope other than 1: a slope of 0.4
  repeats outputs, a slope of 2 skips them. Kinks, such as
  deadzone edges, get knots automatically. A curve like `x^1.7` needs
  about 30 knots for 10 bit fields and 60 for 12 bit fields, the count
  doubles with every two more bits, up to about 240 for 16 bit fields. A
  Rust reference of the integer math tests the fit.
- **Configuration**: reads the configuration, resolves usages and
  formulas, and later matches devices and selects the axes to rewrite, see
  below.
- **Loader**: opens the BPF object with libbpf-rs, through a skeleton that
  `build.rs` generates, sets the `hid_id` and the field table, fills the
  maps, attaches the program, and pins the link to
  `/sys/fs/bpf/xmorpheo/<hid_id>`, so the program stays attached after the
  tool exits. It then sets the evdev fuzz and flat of all axes of the
  device to 0, or to the configured values. Before loading, it checks for the
  needed capabilities, the BPF filesystem, and HID-BPF `struct_ops` in the
  kernel, and explains what is missing.
- **Inspection**: `xmorpheo descriptor <path>` lists the input fields of a
  report descriptor, and `xmorpheo curve <formula>` prints the knots of a
  curve for a field together with sample values, without touching any
  device.
- **Device commands**: `xmorpheo list` shows the HID devices with their
  `hid_id`, whether the configuration has an entry for them, and whether a
  program is attached. `xmorpheo template` prints a configuration template
  for connected devices with their axes commented out. `xmorpheo attach`
  attaches the configured curves to the named devices or, with `--all`, to
  all configured devices, and `--dry-run` shows the field table and knot
  counts without loading anything. `xmorpheo detach` removes them again.

libbpf-rs is used because aya does not support HID-BPF `struct_ops` yet.


## Configuration

The configuration is made of TOML drop-ins in `conf.d` directories, like
systemd: `/etc`, `/run`, `<prefix>/lib`, and `/usr/lib`, each below
`xmorpheo/conf.d`, from the highest priority to the lowest. A file replaces
or masks the files of the same name with a lower priority, the rest is read
in the order of the file names, and a later entry for the same device
replaces an earlier one, see [CONFIG.md](CONFIG.md).
Devices match by vendor ID and product ID, plus an optional HID unique ID
(`HID_UNIQ`, usually the serial number) to tell identical devices apart.
Axes are selected by HID usage. Axes not listed stay untouched.

- `usage` is a name, `X`, `Y`, `Z`, `Rx`, `Ry`, `Rz`, `Slider`, `Dial`,
  `Wheel`, `Rudder`, `Throttle`, `Accelerator`, `Brake`, `Clutch`, or
  `Steering`, or a numeric `page:id` in hexadecimal, such as `0001:0030`.
- `kind` is `centered` or `unipolar`. It defaults by usage: `Slider`,
  `Dial`, `Throttle`, `Accelerator`, `Brake`, and `Clutch` are unipolar,
  everything else is centered.
- `tolerance` sets the fitting tolerance in least significant bits, at the
  top level for all axes and per axis. It defaults to 0.5.
- `[constants]` holds named constants for all formulas.
- `[device.fuzz]` and `[device.flat]` set the evdev fuzz and flat by evdev
  axis name, like `ABS_X`, in raw units. Every other axis of a managed
  device gets 0.
- `evdev = "keep"` leaves the evdev fuzz and flat of a device as the driver
  set them.

Unknown keys, unknown usages, axes listed twice, and invalid formulas are
errors.

```toml
[[device]]
name = "VKB Gladiator EVO R"
vendor = 0x231d
product = 0x0200        # 0x0201 is the Gladiator EVO L
uniq = "..."            # optional, only needed for identical devices

[constants]
dz = 0.02

[[device.axis]]
usage = "X"             # Generic Desktop X
curve = "pow(normalize(abs(x), dz), 1.7) * sign(x)"

[[device.axis]]
usage = "Y"
curve = "pow(normalize(abs(x), dz), 1.7) * sign(x)"
```

The library parses this format. It may still change until the loader
applies it.


## Formulas

Formulas follow the input mixing expressions of Euro Truck Simulator 2
(`controls.sii`), restricted to their stateless arithmetic part. The input
`x` is normalized to `-1..1` for centered axes (sticks) and to `0..1` for
unipolar axes (triggers, throttles). The output uses the same range and is
clamped to it.

- Numbers, `x`, named constants, `+ - * /`, unary minus, and parentheses.
- `abs(v)`, `sign(v)` (0 for 0), `pow(v, e)`, `min(a, b)`, `max(a, b)`,
  `clamp(v, lo, hi)`, and `sel(c, a, b)` (`a` if `c` is not 0, else `b`).
- `normalize(v, dz)` maps `dz..1` to `0..1` and everything below to 0.
- `deadzone(v, dz)` does the same for both directions of a centered axis.

The meaning of `normalize` and `deadzone` is inferred from how ETS2 uses
them, not from documentation.

Constants resolve when parsing. Formulas with the same canonical text share
one curve.


## Device Lifecycle

When a driver binds to a HID device, the udev rule runs `xmorpheo udev
bind`, which attaches the program if the configuration names the device.
`bind` instead of `add`, because only then the descriptor in sysfs has the
fixups of the driver and the evdev nodes exist. After a driver change, the
program is replaced, because the descriptor may differ: the new program is
planned and loaded first, then attached, and the old one detached. If that
fails, for example with an error in the configuration, the old program
stays if the report descriptor is the same as the one it was planned for,
which attach keeps in `/run/xmorpheo/state/<hid_id>.rdesc` next to the
pin. If the descriptor differs or is not known, the old program is
detached, because its fields may no longer match. Either way, the error
goes to the journal. Attaching and
detaching hold a lock on the pin directory, so the rule and a manual
`xmorpheo attach` never attach to a device twice. When the
device disappears, the kernel detaches the program, and `xmorpheo udev
remove` removes the stale pin. The tool can also attach to all present
devices on demand, and detach them again.

The rule runs for every HID device. Devices whose vendor and product the
configuration does not name return before any sysfs or BPF work.
`/sys/fs/bpf` must be mounted, systemd does that at boot. Programs run by
udev run as root and must finish quickly, udev kills them after the event
timeout; attaching takes milliseconds.


## Non-Goals for Now

- Buttons. The field map is generic enough to add them later.
- A graphical user interface.
