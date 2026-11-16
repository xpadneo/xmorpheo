<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Configuration

The configuration is TOML. Devices and axes that are not listed stay
untouched. [RECIPES.md](RECIPES.md) has short configurations for
common problems.


## Files

Like systemd, xmorpheo reads `*.toml` drop-ins from these directories,
from the highest priority to the lowest:

1. `/etc/xmorpheo/conf.d`, for your configuration,
2. `/run/xmorpheo/conf.d`, for configuration until the next reboot,
3. `<prefix>/lib/xmorpheo/conf.d`, next to the installed tool, for example
   `/usr/local/lib/xmorpheo/conf.d`,
4. `/usr/lib/xmorpheo/conf.d`, for packages.

Distributions that build with another `SYSCONFDIR`, like `/usr/etc`, use
that instead of `/etc`. `xmorpheo config` shows the directories of the
installed tool.

- A file replaces every file of the same name with a lower priority. So
  `/etc/xmorpheo/conf.d/50-vkb.toml` overrides a `50-vkb.toml` of a package.
- An empty file, or a symlink to `/dev/null`, masks every file of the same
  name with a lower priority.
- The remaining files are read in the order of their names, like
  `10-vkb.toml` before `50-xbox.toml`, and merged into one configuration.
- `[constants]` and `tolerance` only apply to the file they are in.
- If two files have an entry for the same device, by `vendor`, `product`,
  and `uniq`, the entry of the later file replaces the earlier one as a
  whole, with a warning.

`xmorpheo config` shows which files are active, overridden, or masked, and
their contents. `xmorpheo list` shows the file each configured device comes
from. `-c` reads a single file or a single directory instead.


## Example

Expo curves for a pair of VKB Gladiator EVO sticks, from
`examples/vkb-gladiator-evo.toml`:

```toml
[constants]
k_stick = 0.6           # X/Y: slope 0.4 at the center
k_twist = 0.75          # Rz: slope 0.25 at the center

[[device]]
name = "VKB Gladiator EVO L"
vendor = 0x231d
product = 0x0201

[[device.axis]]
usage = "X"
curve = "(1 - k_stick) * x + k_stick * pow(x, 3)"

[[device.axis]]
usage = "Y"
curve = "(1 - k_stick) * x + k_stick * pow(x, 3)"

[[device.axis]]
usage = "Rz"
curve = "(1 - k_twist) * x + k_twist * pow(x, 3)"
```

The second stick follows with its own `[[device]]` entry.


## Top Level

| Key | Default | Meaning |
|---|---|---|
| `tolerance` | `0.5` | Fitting tolerance in steps of the field, for all axes. |
| `[constants]` | none | Named numbers for all formulas. |
| `[[device]]` | none | One entry per device. |

The tolerance bounds the error of the knot table: every output is within
`tolerance + 0.5` steps of the exact formula. The default of half a step
is below what a sensor can resolve: the output already moves in whole
steps. `0` gives the exactly rounded curve and needs more knots,
which only makes sense for fields with few bits: a 16 bit field can need
more than the 1024 knots a curve may have. The error then names the knots
the curve needs, and a tolerance that fits.


## Devices

| Key | Default | Meaning |
|---|---|---|
| `vendor` | required | USB or Bluetooth vendor ID, like `0x231d`. |
| `product` | required | Product ID, like `0x0200`. |
| `uniq` | any | The unique ID of the device (`HID_UNIQ`, usually the serial number), to tell identical devices apart. |
| `name` | none | A name for messages. |
| `evdev` | none | `"keep"` leaves the evdev fuzz and flat as the driver set them. |
| `[device.fuzz]` | `0` | The evdev fuzz by evdev axis, see below. |
| `[device.flat]` | `0` | The evdev flat by evdev axis, see below. |
| `[[device.axis]]` | none | One entry per axis to rewrite. |

`xmorpheo list` shows vendor, product, and unique ID of the connected
devices. The first entry that matches a device wins. An entry without
`uniq` matches every device with its vendor and product ID.


## evdev Fuzz and Flat

When attaching, xmorpheo sets the evdev fuzz and flat of every absolute
axis of the device to 0. The kernel default fuzz smooths small movements
and costs modern sensors resolution around the center, see
[CLI.md](CLI.md#troubleshooting). The flat is only a deadzone hint for
programs that read evdev, the kernel does not apply it. Games, SDL, and
Proton apply their own deadzones.

Other values are set per evdev axis, by the names that `evtest` and
`evdev-joystick` show, in raw units of the evdev axis:

```toml
[device.flat]
ABS_X = 3072
ABS_Y = 3072

[device.fuzz]
ABS_Z = 4
```

These are evdev axes, not HID usages, because the driver decides which
evdev axis a HID field becomes, and with which range. Unknown names and
axes that the device does not have give a warning, not an error.

`evdev = "keep"` leaves fuzz and flat as the driver set them, for drivers
that choose them deliberately, like xpadneo. It cannot be combined with
`[device.fuzz]` or `[device.flat]`.


## Axes

| Key | Default | Meaning |
|---|---|---|
| `usage` | required | The HID usage of the axis, see below. |
| `curve` | required | The formula, see [FORMULAS.md](FORMULAS.md). |
| `kind` | by usage | `centered` or `unipolar`. |
| `source.<name>` | `x` = the axis | The axes the formula reads, see below. |
| `tolerance` | top level | Fitting tolerance for this axis. |

Usages by name: `X`, `Y`, `Z`, `Rx`, `Ry`, `Rz`, `Slider`, `Dial`,
`Wheel`, `Rudder`, `Throttle`, `Accelerator`, `Brake`, `Clutch`, and
`Steering`. Any other usage works as `page:id` in hexadecimal, as `xmorpheo
descriptor` prints it, for example `0001:0030` for X.

A `centered` axis rests in the middle, like a stick, and the formula sees
`x` in `-1..1`. A `unipolar` axis rests at one end, like a throttle or a
pedal, and the formula sees `x` in `0..1`. `Slider`, `Dial`, `Throttle`,
`Accelerator`, `Brake`, and `Clutch` are unipolar by default, everything
else is centered. Set `kind` where a device uses a usage differently, for
example `kind = "unipolar"` for a throttle on `Z`.


## Sources

By default, a formula reads its own axis as `x`. With `source.<name>`, it
reads other axes of the same report instead, as variables of the formula:

```toml
[[device.axis]]
usage = "Z"                                     # the axis to write
kind = "centered"
source.l = { usage = "Z", kind = "unipolar" }   # left trigger
source.r = { usage = "Rz", kind = "unipolar" }  # right trigger
curve = "r - l"

[[device.axis]]
usage = "Rz"
kind = "unipolar"
curve = "0"                                     # pinned at rest
```

- A source is a usage, like `source.y = "Y"`, which takes the kind of the
  usage, or a table `{ usage = "...", kind = "..." }`. The kind of a source
  never comes from the `[[device.axis]]` entry of the same usage: above, Z
  is a centered target, but a unipolar source.
- With a source table, the formula only sees its variables, and `x` only
  if it is listed. Variable names are letters, digits, and underscores, and
  must not be a function or a constant.
- Every variable is normalized by the kind of its source, and the result by
  the kind of the target, so formulas never see raw values.
- The formula must be a sum of parts that each read one variable, like
  `r - l` or `pow(x, 2) + y / 2`. Formulas like `x * y` are rejected.
- All axes read the values the device sent. Writing an axis does not change
  what other formulas read from it, so above, Rz is pinned at rest while Z
  still reads the right trigger. In the same way, two axes can be swapped
  with `source.x = "Y"` on X and `source.x = "X"` on Y.
- A source must be in the same report as its target, and an axis reads at
  most 4 sources.
- The target keeps the range of its field, also for a combined axis, see
  [DESIGN.md](DESIGN.md#values-never-the-descriptor).

> [!WARNING]
> Do not invert or swap an axis with force feedback, like the steering of a
> wheel or an axis of a force feedback joystick, other than as a test with
> the force feedback of the game turned down or off. The game computes its
> centering forces from the rewritten position, and the device drives
> itself to the end stop, see
> [FORMULAS.md](FORMULAS.md#examples).

Without a source table, the configurations of earlier versions keep their
meaning.

If an axis appears in several fields of the report descriptor, all of them
are rewritten. An axis that the descriptor does not have is an error.
Fields of up to 16 bits are supported, and up to 16 axis fields per
device.


## Errors

Unknown keys, unknown usages, axes listed twice, empty source tables,
invalid formulas or variable names, and tolerances that are negative or
not finite are reported when the configuration is read, with the number
of the device entry and the axis.
Variables that the formula does not use are a warning.
