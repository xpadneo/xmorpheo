<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VKB Gladiator EVO L

## Device

- Vendor ID: `0x231d`
- Product ID: `0x0201`
- bcdDevice: `0x2122`
- HID_NAME: `VKB-Sim © Alex Oz 2021  VKBsim Gladiator EVO  L  `
- HID_UNIQ: empty
- Report descriptor: 313 bytes, byte-identical between the EVO L and the
  EVO R.


## Report Descriptor

Read from `/sys/bus/hid/devices/0003:231D:0201.*/report_descriptor`.

```text
05 01 09 04 a1 01 05 01 85 01 05 01 09 30 75 10
95 01 15 00 26 ff 0f 46 ff 0f 81 02 05 01 09 31
75 10 95 01 15 00 26 ff 0f 46 ff 0f 81 02 05 01
09 35 75 10 95 01 15 00 26 ff 07 46 ff 07 81 02
05 01 09 32 75 10 95 01 15 00 26 ff 07 46 ff 07
81 02 05 01 09 33 75 10 95 01 15 00 26 ff 03 46
ff 03 81 02 05 01 09 34 75 10 95 01 15 00 26 ff
03 46 ff 03 81 02 05 00 09 00 75 10 95 01 15 00
26 ff 07 46 ff 07 81 02 05 00 09 00 75 10 95 01
15 00 26 ff 07 46 ff 07 81 02 05 09 19 01 2a 20
00 15 00 25 01 75 01 96 80 00 81 02 05 01 09 39
15 00 26 07 00 35 00 46 68 01 65 14 55 01 75 04
95 01 81 42 09 00 65 00 55 00 75 04 95 03 81 01
05 01 09 00 75 10 95 01 81 01 05 01 09 00 75 10
95 01 81 01 05 01 09 00 75 10 95 01 81 01 05 01
09 00 75 08 95 17 81 01 85 0b 05 01 09 00 75 08
95 3f 81 01 85 0c 05 01 09 00 75 08 95 3f 81 01
85 08 05 01 09 00 75 08 95 3f 81 01 15 00 26 ff
00 46 ff 00 85 58 75 08 95 3f 09 00 91 02 85 59
75 08 95 80 09 00 b1 02 c0
```


## Input Fields

Measured: parsed with `xmorpheo::descriptor::parse`. Bit offsets include the
report ID byte. All axes are unsigned and absolute.

| Report | Usage | Bit offset | Bits | Logical range |
|---|---|---|---|---|
| 1 | Generic Desktop X (0x30) | 8 | 16 | 0..4095 |
| 1 | Generic Desktop Y (0x31) | 24 | 16 | 0..4095 |
| 1 | Generic Desktop Rz (0x35) | 40 | 16 | 0..2047 |
| 1 | Generic Desktop Z (0x32) | 56 | 16 | 0..2047 |
| 1 | Generic Desktop Rx (0x33) | 72 | 16 | 0..1023 |
| 1 | Generic Desktop Ry (0x34) | 88 | 16 | 0..1023 |
| 1 | Undefined (page 0x00, usage 0x00) | 104 | 16 | 0..2047 |
| 1 | Undefined (page 0x00, usage 0x00) | 120 | 16 | 0..2047 |
| 1 | Buttons, 128 fields, usages 1..32, the last usage repeats | 136 | 1 each | 0..1 |
| 1 | Generic Desktop Hat switch (0x39), null state | 264 | 4 | 0..7 |

The hat switch reports 0..7 for 0..315 degrees and a null state when
centered. Report 1 ends with constant padding. Reports 0x0b, 0x0c, and 0x08
are constant input reports without usages, report 0x58 is a 63 byte output
report, and report 0x59 is a 128 byte feature report.


## Axis Assignment

Measured with `evtest`, using the default VKB firmware profile:

- X/Y (`ABS_X`, `ABS_Y`, 0..4095) are the stick.
- Rz (`ABS_RZ`, 0..2047) is the twist.
- Z (`ABS_Z`, 0..2047) is the throttle lever at the base. It is unipolar
  and reports 2047 at one end stop.
- Rx/Ry (`ABS_RX`, `ABS_RY`, 0..1023) are the analog ministick. With the
  evdev fuzz set to 0, it returns exactly to 512 after letting go.
- The ministick is dual use: a click toggles it between analog (Rx/Ry)
  and digital (the hat switch, `ABS_HAT0X`, `ABS_HAT0Y`). This is a macro
  of the default VKB firmware profile.
- With the evdev fuzz set to 0, the stick and the twist return exactly to
  the center after letting go (X/Y 2048, Rz 1024, measured on the EVO L).
  With the default fuzz of hid-input, the reported rest position stayed up
  to about ±4 counts off center, because the input core holds back small
  changes.
- Moving one axis on purpose does not move any other axis.

Measured with `EVIOCGABS` on Linux 7.3: Y reports fuzz 15 and flat 255, Z
and Rz report fuzz 7 and flat 127, and Rx/Ry report fuzz 3 and flat 63. X
reports fuzz 0 and flat 255.

Inferred: hid-input sets fuzz to `range >> 8` and flat to `range >> 4`, and
a udev rule that only covered X had reset its fuzz. The input core smooths
changes below twice the fuzz, so the curve output near the center is
filtered too. Flat is only a hint for user space.

The two fields without a usage, at bits 104 and 120, are unassigned in the
default VKB profile: they stayed 0 on both sticks while all controls were
moved. Presumably they are free axis slots that VKBDevCfg can assign.
