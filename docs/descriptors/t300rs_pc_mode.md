<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Thrustmaster T300 RS, PC mode, with hid-tmff2 and hid-generic

## Device

- Vendor ID: `0x044f`
- Product ID: `0xb66e`, confirmed for a newer wheel with a PS3-PS4/PC
  switch in PC mode (PS3 normal mode in hid-tmff2). Older wheels have a
  PS3 switch position, which hid-tmff2 recommends for them; their product
  ID in that mode is not confirmed here, check it with `xmorpheo list`.
  hid-tmff2 also knows `0xb66f` (PS3 advanced mode) and `0xb66d` (PS4
  normal mode).
- Drivers: hid-tmff2 (driver `tmff2` of the module `hid-tmff-new`), or
  hid-generic. The upstream hid-thrustmaster only binds to `0xb65d` and
  switches the wheel into its mode, then hid-generic binds to it.


## Report Descriptor with hid-tmff2

hid-tmff2 replaces the report descriptor of the wheel in its
`report_fixup` with a fixed one, `t300rs_rdesc_nrm_fixed` from
`src/tmt300rs/hid-tmt300rs.c` of
[hid-tmff2](https://github.com/Kimplul/hid-tmff2), commit `2874a8e`. The
fixup does not look at the descriptor of the wheel. This is the fixed
descriptor of 135 bytes, read from sysfs with hid-tmff2 bound. The raw
file is `xmorpheo/tests/data/t300rs_pc_mode.rdesc`.

```text
05 01 09 04 a1 01 09 01 a1 00 85 07 09 30 15 00
27 ff ff 00 00 35 00 47 ff ff 00 00 75 10 95 01
81 02 09 35 26 ff 03 46 ff 03 81 02 09 32 81 02
09 31 81 02 81 03 05 09 19 01 29 0d 25 01 45 01
75 01 95 0d 81 02 75 0b 95 01 81 03 05 01 09 39
25 07 46 3b 01 55 00 65 14 75 04 81 42 65 00 81
03 85 60 06 00 ff 09 60 75 08 95 3f 26 ff 7f 15
00 46 ff 7f 36 00 80 91 02 85 02 09 02 81 02 09
14 85 14 81 02 c0 c0
```


## Report Descriptor of the Wheel

Measured on 2026-11-26 in sysfs with hid-generic bound, so without any
fixup: the wheel in PC mode reports `044f:b66e` and its own descriptor
of 130 bytes. It differs from the fixed one of hid-tmff2 in the usages
of the pedals, and in its vendor reports: report 10 is the output
report, and reports 2 and 20 have a logical maximum of 255. The raw file
is `xmorpheo/tests/data/t300rs_pc_mode_hid_generic.rdesc`.

```text
05 01 09 04 a1 01 09 01 a1 00 85 07 09 30 15 00
27 ff ff 00 00 35 00 47 ff ff 00 00 75 10 95 01
81 02 09 31 26 ff 03 46 ff 03 81 02 09 35 81 02
09 36 81 02 81 03 05 09 19 01 29 0d 25 01 45 01
75 01 95 0d 81 02 75 0b 95 01 81 03 05 01 09 39
25 07 46 3b 01 55 00 65 14 75 04 81 42 65 00 81
03 85 0a 06 00 ff 09 0a 75 08 95 3f 26 ff 00 46
ff 00 91 02 85 02 09 02 81 02 09 14 85 14 81 02
c0 c0
```


## Input Fields

Parsed with `xmorpheo::descriptor::parse`. Bit offsets include the report
ID byte. Input report 7 has the axes:

| Report | Usage with hid-tmff2 | Usage with hid-generic | Bit offset | Bits | Logical range | Comment in hid-tmff2 |
|---|---|---|---|---|---|---|
| 7 | Generic Desktop X (0x30) | X (0x30) | 8 | 16 | 0..65535 | wheel |
| 7 | Generic Desktop Rz (0x35) | Y (0x31) | 24 | 16 | 0..1023 | brake |
| 7 | Generic Desktop Z (0x32) | Rz (0x35) | 40 | 16 | 0..1023 | gas |
| 7 | Generic Desktop Y (0x31) | Slider (0x36) | 56 | 16 | 0..1023 | clutch |
| 7 | Buttons 1..13 | Buttons 1..13 | 88 | 1 each | 0..1 | |
| 7 | Generic Desktop Hat switch (0x39) | Hat switch (0x39) | 112 | 4 | 0..7 | |

Reports 2 and 20 are vendor reports for force feedback. The pedals lie
at the same places with both drivers, so the raw reports are the same.

The wheel pads input report 7 to the 64 bytes of its interrupt endpoint,
measured on 2026-11-22 in the debugfs events of the HID device: the 15
bytes of the descriptor, then zeros. The HID core ignores the extra bytes.


## Pedal Mapping

The fixed descriptors of hid-tmff2 name the same pedal fields differently
per mode: Rz, Z, and Y for `0xb66e`, Y, Rz, and Slider for `0xb66f`.
Under hid-generic, the wheel itself names them Y, Rz, and Slider in
`0xb66e` too. Games and their templates expect brake and clutch
differently, see
[hid-tmff2#233](https://github.com/Kimplul/hid-tmff2/issues/233).

Measured in Euro Truck Simulator 2 under Proton: with hid-generic, the
pedals map as the game expects, with hid-tmff2, brake and clutch are
swapped. Inferred from the sources of Proton's Wine and SDL: Proton
reads the wheel through SDL, not `hidraw`, and SDL numbers the evdev axes
in the order of their codes. Under hid-generic, that is wheel, brake,
gas, and clutch, under hid-tmff2 wheel, clutch, gas, and brake. So a
configuration that swaps pedals must name the driver its usages are
written for, see `examples/t300rs-swap-pedals.toml`.

Measured with evtest on 2026-11-22, on a wheel in PC mode: the brake
pedal moves Rz and the clutch pedal Y, as the comments in hid-tmff2 say.
The pedals rest at the maximum of 1023 and go down to 0 when pressed.
