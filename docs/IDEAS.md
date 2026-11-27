<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Ideas

Ideas for later. They are not planned for a release yet. Move an idea into
[DESIGN.md](DESIGN.md) when work on it starts, and delete it here when done.


## Remapping

- Swap buttons.
- Swap the buttons for a left-handed mode of gamepads, like the bumpers and
  the stick buttons. The axes already work, see
  [RECIPES.md](RECIPES.md#left-handed-gamepad-axes).
- Layers: a button that toggles between two or more mappings of the buttons
  and axes, like a shift key. The state goes into a BPF map per device, with
  the previous state of the toggle button for detecting its edges. That
  needs no descriptor fixup as long as a layer only remaps buttons and axes
  the device has, and the toggle button itself can be swallowed. Extra
  buttons in a second layer would need a changed descriptor, which is not
  planned, see [DESIGN.md](DESIGN.md#values-never-the-descriptor). Layers
  end the statelessness for buttons and layers, the curves stay stateless.
  See also switching curve sets under [Curves](#curves).
- Simple macros, like auto-repeat (turbo) for a held button. The program
  only runs when the device sends a report, and many devices only send on
  changes, so turbo needs a timer, `bpf_timer` or `bpf_wq`, that injects
  synthetic reports with the kfunc `hid_bpf_input_report`. Open: which
  kernel offers that to HID-BPF struct_ops programs, how drivers react to
  injected reports, like the deduplication in the `raw_event` of xpadneo,
  and what hidraw readers see.


## Curves

- Curve presets as named formulas, such as "precise" or "aggressive".
- Switch between curve sets at runtime, for example a precision curve on a
  button press for aiming.
- Expo curve presets of the form `(1-k)*x + k*pow(x, 3)`, which keep a
  non-zero slope at the center.
- Always set the evdev fuzz of all axes of managed devices to 0 when
  attaching (`EVIOCSABS`), without an option. The hid-input default
  (fuzz = range >> 8) dates from potentiometer sticks and removes about
  three bits of resolution around the center of modern sensors. Curves make
  it worse: the input core filters output changes that a flatter curve has
  shrunk.
- Also set flat to 0 by default, configurable per device and axis. Flat is
  only a deadzone hint for user space. Games, SDL, and Proton apply their
  own deadzones, and devices that do not center exactly, such as some older
  hall sensor sticks, should get a deadzone in the curve instead.
- Both make separate udev rules with `evdev-joystick` unnecessary. hidraw
  readers are not affected.


## Force Feedback

- Invert force feedback together with an inverted steering axis. HID-BPF
  also hooks the way from the game to the device, `hid_hw_output_report`
  and `hid_hw_request` in `hid_bpf_ops`, so a program could mirror the
  direction of force feedback reports. Whether that works is open, and it
  depends on the protocol of each device, like the one of Thrustmaster
  wheels that hid-tmff2 speaks.


## User Interface

- An SVG output of `xmorpheo curve` written by the tool itself, so a plot of
  a curve needs no gnuplot.

- A graphical user interface, for example built with GPUI, that shows a
  realistic preview of the device based on its vendor and product ID, and
  previews the curve on live input.


## Website

- The rustdoc API documentation of the crate for contributors, under
  `/api/` of the project website.
