<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Xbox Wireless Controller (Series X|S), Bluetooth LE, with xpadneo

## Device

- Vendor ID: `0x045e`
- Product ID: `0x0b13`, as in the sysfs name `0005:045E:0B13.*`
- HID_NAME: `Xbox Wireless Controller`
- HID_UNIQ: the Bluetooth address of the controller
- Firmware: BLE 5.23, connected through HOGP and uhid
- Driver: xpadneo
- Report descriptor: 283 bytes, crc16 `0x931d`


## Product ID

xpadneo changes the product ID of the HID device to `0x028e` and reports
that as "pretending XB1S Windows wireless mode". `HID_ID` in the uevent and
the evdev device then show `045e:028e`, but the sysfs name keeps
`0B13`. xmorpheo matches by the sysfs name, so the configuration uses
`product = 0x0b13`.


## Report Descriptor

Read from `/sys/bus/hid/devices/0005:045E:0B13.*/report_descriptor`, so
this is the descriptor after the `report_fixup` of xpadneo. The raw file is
`xmorpheo/tests/data/xbox_series_xs_ble_xpadneo.rdesc`.

```text
05 01 09 05 a1 01 85 01 09 01 a1 00 09 30 09 31
15 00 27 ff ff 00 00 95 02 75 10 81 02 c0 09 01
a1 00 09 33 09 34 15 00 27 ff ff 00 00 95 02 75
10 81 02 c0 05 01 09 32 15 00 26 ff 03 95 01 75
0a 81 02 15 00 25 00 75 06 95 01 81 03 05 01 09
35 15 00 26 ff 03 95 01 75 0a 81 02 15 00 25 00
75 06 95 01 81 03 05 01 09 39 15 01 25 08 35 00
46 3b 01 66 14 00 75 04 95 01 81 42 75 04 95 01
15 00 25 00 35 00 45 00 65 00 81 03 05 09 19 01
29 0c 15 00 25 01 75 01 95 0c 81 02 15 00 25 00
75 01 95 04 81 03 05 0c 0a b2 00 15 00 25 01 95
01 75 01 81 02 15 00 25 00 75 07 95 01 81 03 05
0f 09 21 85 03 a1 02 09 97 15 00 25 01 75 04 95
01 91 02 15 00 25 00 75 04 95 01 91 03 09 70 15
00 25 64 75 08 95 04 91 02 09 50 66 01 10 55 0e
15 00 26 ff 00 75 08 95 01 91 02 09 a7 15 00 26
ff 00 75 08 95 01 91 02 65 00 55 00 09 7c 15 00
26 ff 00 75 08 95 01 91 02 c0 c0
```


## Input Fields

Measured: parsed with `xmorpheo::descriptor::parse`. Bit offsets include the
report ID byte. Input report 1 has 17 bytes.

| Report | Usage | Bit offset | Bits | Logical range |
|---|---|---|---|---|
| 1 | Generic Desktop X (0x30) | 8 | 16 | 0..65535 |
| 1 | Generic Desktop Y (0x31) | 24 | 16 | 0..65535 |
| 1 | Generic Desktop Rx (0x33) | 40 | 16 | 0..65535 |
| 1 | Generic Desktop Ry (0x34) | 56 | 16 | 0..65535 |
| 1 | Generic Desktop Z (0x32) | 72 | 10 | 0..1023 |
| 1 | Generic Desktop Rz (0x35) | 88 | 10 | 0..1023 |
| 1 | Generic Desktop Hat switch (0x39), null state | 104 | 4 | 1..8 |
| 1 | Buttons 1..12 | 112 | 1 each | 0..1 |
| 1 | Consumer AC Share (0x0c:0xb2) | 128 | 1 | 0..1 |

Report 3 is the rumble output report.


## Changes by xpadneo

From the xpadneo source, `report_fixup` in `xpadneo/device.c`:

- Right stick: usage Z becomes Rx and Rz becomes Ry.
- Triggers: Simulation Brake becomes Generic Desktop Z, and Simulation
  Accelerator becomes Generic Desktop Rz.
- Buttons: 15 buttons and 1 padding bit become 12 buttons and 4 padding
  bits.

All of these only change usages and counts within the same bits, so the
bit offsets and the report length stay as in the raw reports, which the
BPF program sees. `raw_event` drops repeated reports, remaps buttons, and
reads the profile bytes of Elite Series 2 controllers, but does not touch
the axes. The mouse mode of xpadneo works on the values after the curve.

With another driver, the right stick is Z and Rz, and the triggers are
Brake and Accelerator, so a configuration for xpadneo does not fit.


## evdev

Measured with `EVIOCGABS` on `event28` (the gamepad node of xpadneo): the
sticks report -32768..32767 with fuzz 32 and flat 3072, the triggers
(`ABS_Z`, `ABS_RZ`) 0..1023 with fuzz 4 and flat 0. xpadneo sets these
itself. The device also has consumer control, keyboard, and mouse nodes
without absolute axes.
