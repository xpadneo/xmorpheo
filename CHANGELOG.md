<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Changelog

Notable changes of each release. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning](https://semver.org/). Before 1.0, the
configuration format may still change.


## [Unreleased]

### Added

- The website <https://xpadneo.github.io/xmorpheo/> with the documentation
  of the latest release.


## [0.2.0] - 2026-11-27

### Added

- `driver` limits a device entry to one HID driver, for drivers that
  replace the report descriptor and so rename the axes. An entry with
  `driver` wins over one without. `xmorpheo template` shows the driver.
- `xmorpheo list` shows the HID driver bound to each device.

### Fixed

- The Thrustmaster T300 RS example also applied under hid-generic, where
  the wheel names its pedals differently, so it swapped brake and gas
  instead of brake and clutch. It now only applies with hid-tmff2 bound.
- When a driver bound to a device that the configuration names, but no
  entry matched the device with that driver, the udev rule kept the
  program that was still attached. It now detaches it.


## [0.1.2] - 2026-11-22

### Fixed

- Reports longer than the report descriptor says were left untouched, so
  nothing was remapped on devices that pad their reports, like the
  Thrustmaster T300 RS, which sends 64 bytes for a report of 15 bytes.
  Longer reports are now rewritten, like the HID core reads them; shorter
  ones still stay untouched.

### Changed

- The pedal swap of the Thrustmaster T300 RS example is now tested on the
  device, in Euro Truck Simulator 2 under Proton.


## [0.1.1] - 2026-11-16

### Fixed

- `xmorpheo attach --replace` detached the old program before it pinned
  the new one, so a failure in between left the device without a program.
  The new program is now pinned under a temporary name first, then the old
  one is detached, and the new one renamed into place. If anything fails,
  exactly one program stays attached.
- An empty source table (`source = {}`) is a configuration error instead of
  a panic while planning.
- Tolerances must be finite numbers of at least 0.

### Changed

- When a udev bind fails, for example with an error in an edited
  configuration, the previous program stays if the report descriptor is
  unchanged. It is kept in `/run/xmorpheo/state/<hid_id>.rdesc`.
- `xmorpheo udev remove` logs no errors for devices that the configuration
  does not name.
- The error of a failed udev bind shows only once in the journal.


## [0.1.0] - 2026-11-16

Initial release: response curves and axis remapping for HID joysticks,
gamepads, and wheels with HID-BPF, tested with VKB Gladiator EVO sticks and
an Xbox Series X|S controller under xpadneo.

[Unreleased]: https://github.com/xpadneo/xmorpheo/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/xpadneo/xmorpheo/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/xpadneo/xmorpheo/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/xpadneo/xmorpheo/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/xpadneo/xmorpheo/releases/tag/v0.1.0
