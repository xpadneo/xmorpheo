<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Recipes

Short configurations for common problems. Each one is a complete drop-in
for `/etc/xmorpheo/conf.d`, see [CONFIG.md](CONFIG.md). Replace `vendor`,
`product`, and the usages with those of your device: `xmorpheo template`
prints them. The tests of xmorpheo parse every recipe here.

Formulas see normalized values: `-1..1` for centered axes like sticks and
wheels, `0..1` for unipolar axes like pedals, triggers, and throttles, see
[FORMULAS.md](FORMULAS.md).


## Invert an Axis

```toml
[[device]]
vendor = 0x231d
product = 0x0200

[[device.axis]]
usage = "Y"
curve = "-x"
```

For a unipolar axis, like a throttle, the inverse is `1 - x`, with
`kind = "unipolar"` if the usage is not unipolar by default.

> [!WARNING]
> Do not invert an axis with force feedback, like the steering of a wheel
> or an axis of a force feedback joystick, other than as a test with the
> force feedback of the game turned down or off. The game sees the inverted
> position and computes its centering forces in the wrong direction, so the
> device drives itself to the end stop, which can be unpleasant with a
> strong wheel and can pinch fingers on a joystick. Spring and damper
> effects that run in the firmware of the device use the real position and
> stay correct. For lasting use, a game must invert the forces itself, like
> Forza Horizon 5 can. See also [IDEAS.md](IDEAS.md#force-feedback).


## Swap Two Axes

Each axis reads the other one. All formulas read the values the device
sent, so the order does not matter. Here brake (Rz) and clutch (Y) of a
wheel:

```toml
[[device]]
vendor = 0x044f
product = 0xb66e

[[device.axis]]
usage = "Rz"
kind = "unipolar"
source.x = { usage = "Y", kind = "unipolar" }
curve = "x"

[[device.axis]]
usage = "Y"
kind = "unipolar"
source.x = { usage = "Rz", kind = "unipolar" }
curve = "x"
```

See [examples/t300rs-swap-pedals.toml](../examples/t300rs-swap-pedals.toml).


## Combine Two Pedals or Triggers on One Axis

Older games want gas and brake on one axis, resting in the middle. The
target becomes centered, both sources stay unipolar:

```toml
[[device]]
vendor = 0x044f
product = 0xb66e

[[device.axis]]
usage = "Z"                                  # gas, becomes the combined axis
kind = "centered"
source.gas = { usage = "Z", kind = "unipolar" }
source.brake = { usage = "Rz", kind = "unipolar" }
curve = "gas - brake"

[[device.axis]]
usage = "Rz"                                 # brake, pinned at rest
kind = "unipolar"
curve = "0"
```

Pinning the brake does not change what the combined axis reads. See
[examples/xbox-combined-triggers.toml](../examples/xbox-combined-triggers.toml)
for the triggers of a gamepad.


## Left-Handed Gamepad (Axes)

The sticks trade places, and so do the triggers. Here for an Xbox Wireless
Controller with xpadneo, which names the right stick Rx/Ry and the triggers
Z (left) and Rz (right):

```toml
[[device]]
vendor = 0x045e
product = 0x0b13
evdev = "keep"            # keep the fuzz and flat xpadneo chooses

[[device.axis]]
usage = "X"
source.x = "Rx"
curve = "x"

[[device.axis]]
usage = "Y"
source.x = "Ry"
curve = "x"

[[device.axis]]
usage = "Rx"
source.x = "X"
curve = "x"

[[device.axis]]
usage = "Ry"
source.x = "Y"
curve = "x"

[[device.axis]]
usage = "Z"
kind = "unipolar"
source.x = { usage = "Rz", kind = "unipolar" }
curve = "x"

[[device.axis]]
usage = "Rz"
kind = "unipolar"
source.x = { usage = "Z", kind = "unipolar" }
curve = "x"
```

A curve for the sticks goes in place of `x`, like in
[examples/xbox-wireless-controller.toml](../examples/xbox-wireless-controller.toml).
xmorpheo does not rewrite buttons yet, so the bumpers, the stick buttons,
and the other buttons stay where they are, see [IDEAS.md](IDEAS.md#remapping).


## Pin an Axis at Rest

A constant formula holds an axis still, for example a twist that should
not yaw:

```toml
[[device]]
vendor = 0x231d
product = 0x0200

[[device.axis]]
usage = "Rz"
curve = "0"
```

For a unipolar axis, `0` is the released end.


## Add a Deadzone

Around the center of a stick, and at the start of a pedal:

```toml
[[device]]
vendor = 0x045e
product = 0x0b13

[[device.axis]]
usage = "X"
curve = "deadzone(x, 0.05)"

[[device.axis]]
usage = "Z"
kind = "unipolar"
curve = "normalize(x, 0.05)"
```

Full deflection stays reachable. Many games apply their own deadzone, so
add one only where the device needs it.


## Reach Full Deflection Early

Sticks and pedals that do not quite reach their end stop, or that should
reach full output before it. The result of every formula is clamped to the
range of the axis, so scaling is enough: `x / 0.95` gives full output from
95 % of the travel on, in both directions of a centered axis. With a
deadzone at the center or at the start too, map the range in between:

```toml
[constants]
dz = 0.05                 # deadzone at the center or start
e = 0.05                  # travel left before the end stop

[[device]]
vendor = 0x044f
product = 0xb66e

[[device.axis]]
usage = "X"               # centered: 5 % deadzone, full from 95 %
curve = "clamp((abs(x) - dz) / (1 - dz - e), 0, 1) * sign(x)"

[[device.axis]]
usage = "Z"               # unipolar: nothing below 5 %, full from 90 %
kind = "unipolar"
curve = "(x - dz) / (0.9 - dz)"
```

For the unipolar pedal, the clamp to `0..1` of the result already keeps
the start at 0. For the centered stick, the inner `clamp` keeps the center
at 0, see [FORMULAS.md](FORMULAS.md#clamping). The constants keep the
formulas readable. To combine it with another curve, put the expression in
place of `x` of that curve, for example `pow(clamp((x - dz) / (0.9 - dz), 0,
1), 1.5)` for a progressive pedal, where the clamp keeps `pow` away from
negative values.


## Finer Control Around the Center

An expo curve keeps a reduced slope at the center and reaches full
deflection at the edges. A larger `k` gives finer control around the
center:

```toml
[constants]
k = 0.6

[[device]]
vendor = 0x231d
product = 0x0200

[[device.axis]]
usage = "X"
curve = "(1 - k) * x + k * pow(x, 3)"

[[device.axis]]
usage = "Y"
curve = "(1 - k) * x + k * pow(x, 3)"
```

See [FORMULAS.md](FORMULAS.md#expo-curves) for the effect of `k`.


## Progressive Brake

Finer control at the start of the travel of a pedal:

```toml
[[device]]
vendor = 0x044f
product = 0xb66e

[[device.axis]]
usage = "Rz"
kind = "unipolar"
curve = "pow(x, 1.5)"
```
