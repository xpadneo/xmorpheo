<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Formulas

A curve is a formula in the normalized input `x`. For centered axes, `x`
runs from `-1` to `1` with the rest position at `0`. For unipolar axes,
it runs from `0` to `1`. The result uses the same range and is clamped to
it, so `2 * x` saturates at half deflection.

The syntax follows the input mixing expressions of Euro Truck Simulator 2
(`controls.sii`), restricted to the part without state.


## Syntax

- Numbers like `0.6` or `3`, the input `x`, and named constants from the
  `[constants]` table of the configuration.
- With sources (see [CONFIG.md](CONFIG.md#sources)), the variables named
  there instead of `x`.
- `+`, `-`, `*`, `/` with the usual precedence, unary minus, and
  parentheses. There is no power operator, use `pow`.

| Function | Result |
|---|---|
| `abs(v)` | absolute value |
| `sign(v)` | `-1`, `0`, or `1`; `0` for `0` |
| `pow(v, e)` | `v` to the power of `e` |
| `min(a, b)`, `max(a, b)` | the smaller or larger value |
| `clamp(v, lo, hi)` | `v` limited to `lo..hi` |
| `sel(c, a, b)` | `a` if `c` is not `0`, else `b` |
| `normalize(v, dz)` | maps `dz..1` to `0..1`, and everything below `dz` to `0` |
| `deadzone(v, dz)` | like `normalize`, for both directions of a centered axis |

Functions can be nested freely.


## Clamping

The result of every formula is clamped to the range of its axis: `-1..1`
for centered axes, `0..1` for unipolar axes. So a formula needs no outer
`clamp`, and scaling reaches full output early: `x / 0.95` is full from 95 %
of the travel on, and there is no separate function for a deadzone at the
end stops, see [RECIPES.md](RECIPES.md#reach-full-deflection-early).

Inner parts are not clamped. In
`clamp((abs(x) - dz) / (1 - dz - e), 0, 1) * sign(x)`, the `clamp` keeps
the deadzone at 0, which the outer range would not do for a centered axis.
The terms of a formula in several variables are not clamped one by one
either, only their sum, see [Several Variables](#several-variables).

The meaning of `normalize` and `deadzone` follows how ETS2 uses them. It is
inferred, not documented by ETS2.

`pow` of a negative number with a fractional exponent is not defined. Use
`pow(abs(x), e) * sign(x)` for centered axes. The tool reports the input
where a formula is not finite.


## Examples

| Formula | Effect |
|---|---|
| `x` | unchanged |
| `-x` | inverted; not for axes with force feedback, see below |
| `deadzone(x, 0.05)` | deadzone of 5 % around the center, full deflection stays reachable |
| `pow(abs(x), 1.7) * sign(x)` | power curve, flat at the center |
| `pow(normalize(abs(x), 0.05), 1.7) * sign(x)` | the ETS2 steering curve, deadzone and power |
| `(1 - k) * x + k * pow(x, 3)` | expo curve, see below |
| `pow(x, 2)` | for unipolar axes, finer control at the start |
| `x / 0.95` | full output from 95 % of the travel on, because the result is clamped |
| `(x - 0.05) / 0.85` | unipolar, nothing below 5 %, full from 90 % |


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


## Expo Curves

`(1 - k) * x + k * pow(x, 3)` with `k` between `0` and `1` keeps a linear
part at the center, unlike a power curve, so small corrections still have
an effect:

| `k` | slope at the center | slope at full deflection | output at half deflection |
|---|---|---|---|
| `0` | 1 | 1 | 50 % |
| `0.5` | 0.5 | 2 | 31 % |
| `0.6` | 0.4 | 2.2 | 28 % |
| `0.75` | 0.25 | 2.5 | 22 % |
| `1` | 0 | 3 | 13 % |

The slope at the center is `1 - k`, at full deflection `1 + 2k`. A larger
`k` gives finer control around the center, at the cost of a faster
response near the end stops.


## Several Variables

A formula with sources is split into a sum of parts that each depend on one
variable. Each part becomes its own curve, and the program adds them up and
clamps the sum once. So `0.5 + (r - l) / 2` works, and so does
`pow(x, 2) - abs(y)`. Parts that combine variables, like `x * y`,
`pow(x - y, 2)`, or `max(x, y)`, cannot be split and are rejected. The
tolerance of the axis is divided among the parts, so their sum stays
within it.

Combined triggers on one centered axis, with the two triggers as unipolar
sources `l` and `r`:

| Formula | Effect |
|---|---|
| `r - l` | right trigger turns one way, left trigger the other, both or none rest in the middle |
| `pow(r, 2) - pow(l, 2)` | the same, with finer control at the start of the travel |


## Limits

The BPF program in the kernel evaluates curves as tables of knots in one
variable, with integer math only. That sets these limits.

**Only sums of parts in one variable each.** A product, quotient, or other
function of two variables would need a table in two dimensions, so these
are rejected when the configuration is read, with an explanation: `x * y`,
`x / y`, `pow(x - y, 2)`, `abs(x - y)`, and `min`, `max`, `clamp`, or `sel`
over more than one variable. Sums and differences of parts in one variable
each work, and so do constant factors, like `2 * (x - y)` or
`(r - l) / 2`.

**No state.** A formula sees the current values only: no time, no
smoothing or other filters, no hysteresis, and no toggles.

**The range of the field.** The result has the range of the field of the
axis, because xmorpheo never changes the report descriptor, see
[DESIGN.md](DESIGN.md#values-never-the-descriptor). A combined axis shares
that range among its sources.

**Sources from the same report.** All variables of an axis must be fields
of the same input report as the axis, or planning fails.

**Capacities**, checked when planning, with an error that names the limit:

| What | Limit |
|---|---|
| Field size | 16 bits |
| Fields read or written per device | 16 |
| Axes written per device | 16 |
| Variables per axis | 4 |
| Terms per device | 32 |
| Curves per device | 64 |
| Knots per curve | 1024 |
| Knots per device | 8192 |
| Value of a single term, before the sum is clamped | `-4..4` |

A tolerance of 0 on a 16 bit field can need more than 1024 knots; a small
tolerance like 0.1 needs about 500. The default of 0.5 steps is below what a
sensor can resolve, the output moves in whole steps anyway, so a tolerance of 0 only makes sense for
fields with few bits. A curve that needs too many knots is an error that
names the formula, the bits, the tolerance, the knots it needs, and the
smallest of a few common tolerances that fits.

**Names.** Variables and constants are letters, digits, and underscores,
not starting with a digit. A variable must not have the name of a function
or a constant.

**Undefined values.** `pow` of a negative number with a fractional
exponent, and a division by zero, are errors that name the input where
they happen.


## Checking a Formula

```sh
xmorpheo curve "(1 - k) * x + k * pow(x, 3)" -c k=0.6 --bits 12
```

prints the canonical formula, the knots the BPF program will use, and the
output for sample inputs next to the exact value. With `--format gnuplot`,
it prints a plot of the curve and its error, and with `--format csv`, every
value, see [CLI.md](CLI.md#curve). Formulas that only differ in spacing or
constant names have the same canonical text and share one curve.
