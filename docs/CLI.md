<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Command Line

Devices are selected by their sysfs name like `0003:231D:0200.000A`, by
their `hid_id` like `10`, or by `VENDOR:PRODUCT` like `231d:0200`.
`xmorpheo list` shows all three. Commands that load or remove programs
need root.

| Command | Root | Purpose |
|---|---|---|
| `xmorpheo list [-c PATH]` | no | HID devices, whether a program is attached, and the file that configures them |
| `xmorpheo config [-c PATH]` | no | the configuration files, which are active, overridden, or masked, and their contents |
| `xmorpheo template [DEVICE...]` | no | a configuration template with all axes commented out |
| `xmorpheo descriptor PATH [--all]` | no | the input fields of a report descriptor |
| `xmorpheo curve FORMULA [options]` | no | the knots and sample values of a curve, as text, CSV, or a gnuplot script |
| `xmorpheo attach [-c PATH] DEVICE... \| --all [--dry-run] [--replace] [-v]` | yes | attach the configured curves |
| `xmorpheo detach DEVICE... \| --all` | yes | detach them again |


`-c` reads a single configuration file or directory instead of the drop-in
directories, see [CONFIG.md](CONFIG.md#files).


## list

Without root, the attached column shows `?`, because `/sys/fs/bpf` is only
readable by root.


## config

Shows the drop-in directories, which files are active, overridden by a file
of the same name with a higher priority, or masked, the warnings, and then
the contents of the active files in the order they are read, like
`systemd-analyze cat-config`.


## template

Prints a template for the named devices, or for all connected devices with
axes. Each axis has a comment with its bit size, position, logical range,
and default kind. Uncomment the axes to rewrite and set their curves.

```sh
xmorpheo template 231d:0200 | sudo tee /etc/xmorpheo/conf.d/50-vkb.toml
```


## descriptor

```sh
xmorpheo descriptor /sys/bus/hid/devices/0003:231D:0200.000A/report_descriptor
```

lists report ID, usage, bit size, bit offset, and logical range of each
input field. One bit fields like buttons only show with `--all`.


## curve

```sh
xmorpheo curve "(1 - k) * x + k * pow(x, 3)" -c k=0.6 --bits 12
```

| Option | Default | Meaning |
|---|---|---|
| `--bits N` | `12` | an unsigned field with `N` bits |
| `--range MIN..MAX` | | the logical range instead of `--bits` |
| `--kind centered\|unipolar` | `centered` | how the axis rests |
| `--tolerance T` | `0.5` | the fitting tolerance in steps |
| `-c NAME=VALUE` | | a constant, may be repeated |
| `--format text\|csv\|gnuplot` | `text` | what to print, see below |
| `--knots` | | with `csv`, also the knots |
| `-o FILE` | | write to `FILE` instead of standard output |
| `--terminal png\|svg` | | with `gnuplot`, render an image instead of showing the plot |
| `--output-image FILE` | | the image `--terminal` renders |

`text` prints the canonical formula, the knots, and the output at nine
sample inputs next to the exact value.

`csv` prints every raw input of the field: `raw_in`, the normalized input
`x`, the `exact` output of the formula as a raw value, the `lut` output of
the integer math that the BPF program computes, and `error_lsb`, their
difference in steps. With `--knots`, an empty line and the knots follow,
as `knot_in,knot_out,slope_q24`.

`gnuplot` prints a script with the data inline. It plots the exact curve,
the output of the integer math, the knots, the identity as a reference, and
the error in steps on the right axis. The title names the formula, the
kind, the bit width, the tolerance, and the number of knots. Fields with
more than 2048 values are thinned out to 2048 points; the error then shows
the smallest and largest error of each group of values, so no error goes
missing.

```sh
xmorpheo curve "(1 - k) * x + k * pow(x, 3)" -c k=0.6 --format gnuplot | gnuplot -p
xmorpheo curve "pow(x, 1.5)" --kind unipolar --bits 16 --format gnuplot \
	--terminal png --output-image brake.png | gnuplot
```

`curve` takes formulas in `x` only, because it fits one curve. Formulas
with more variables, like `gas - brake`, go into a configuration, and
`attach --dry-run` shows their terms, see [FORMULAS.md](FORMULAS.md#several-variables).


## attach

Reads the configuration, finds the axes in the report descriptor of each
device, fits the curves, loads one instance of the BPF program per device,
and pins it to `/sys/fs/bpf/xmorpheo/<hid_id>`. Then it sets the evdev fuzz
and flat of all axes of the device to 0, or to the values of
`[device.fuzz]` and `[device.flat]`. The
program stays attached after the tool exits, until `detach`, unplugging,
or reboot. After installing, the udev rule does this on its own whenever a
configured device appears.

- `--all` attaches to every connected device that the configuration
  names.
- `--dry-run` prints the field table, the curves, and the knot counts
  without loading anything, and needs no root.
- `--replace` replaces a program that is already attached, for example
  after changing the configuration. Without it, such a device is an error.
- `-v`, `--verbose` prints the field table of each device, and the libbpf
  log, including the verifier log.

Without `--verbose`, `attach` prints one line per device:

```
VKB Gladiator EVO L (hid_id 9): attached, 3 fields, 104 knots, evdev: /dev/input/event11: set fuzz and flat of 8 of 8 axes
```

The evdev part says on how many axes `attach` changed the fuzz and flat,
or that they were already as configured, like after an earlier attach.
They stay until the device is plugged in again. The udev rule writes the
same line to the journal, see
`journalctl -t xmorpheo`, and more only on errors.

`--replace` reads the configuration, plans the device, loads and attaches
the new program, and pins it at `/sys/fs/bpf/xmorpheo/<hid_id>.new`
before it touches the old one. If anything fails up to there, the old
program stays and the error says why. Then it detaches the old program
and renames the new pin into place. The kernel cannot swap the program of
a HID-BPF link in place, so for that moment both run, and a single report
can pass both curves. If the rename fails, the new program runs under the
temporary name, and the next `attach` or `detach` of the device moves or
removes it.


## detach

Removes the pinned link, which detaches the program. `--all` detaches every
program that xmorpheo pinned.


## Troubleshooting

**Check the rest position.** Run `evtest` on the event node of the device
and let go of the stick. It should rest in the middle of its range, like
2048 for 0..4095. If it rests elsewhere, calibrate the device in its own
tool first, for example VKBDevCfg for VKB sticks. A curve shapes the
response around the rest position, it cannot move it.

**Small movements are swallowed.** The kernel sets the evdev fuzz of
joystick axes to `range >> 8` and smooths changes below about twice that,
which costs resolution around the center. `xmorpheo attach` sets it to 0.
Check with `evdev-joystick --showcal /dev/input/eventN`. Without xmorpheo,
a udev rule with `evdev-joystick --evdev $devnode --fuzz 0 --deadzone 0`
does the same. Programs that read `hidraw` are not affected.

**The deadzone is too large or too small.** evdev flat is only a hint for
programs that read evdev. Games usually apply their own deadzone. Put a
deliberate deadzone into the curve with `deadzone(x, dz)`.

**Errors when attaching.** The tool checks for root, the BPF filesystem,
and HID-BPF in the kernel and says what is missing. If the verifier
rejects the program, run `attach --verbose` and look at the end of the
log.

**udev.** The rule logs to syslog as `xmorpheo`:

```sh
journalctl -t xmorpheo
```

To see the rule run, check that it is installed, reload, and trigger the
HID devices:

```sh
cat /etc/udev/rules.d/60-xmorpheo.rules
sudo udevadm control --reload
sudo udevadm trigger --action=bind --subsystem-match=hid
```

`sudo udevadm test /sys/bus/hid/devices/<device>` shows which rules match,
without running them.
