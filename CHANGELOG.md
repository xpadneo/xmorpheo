<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Changelog

Notable changes of each release. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning](https://semver.org/). Before 1.0, the
configuration format may still change.


## [Unreleased]


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

[Unreleased]: https://github.com/xpadneo/xmorpheo/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/xpadneo/xmorpheo/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/xpadneo/xmorpheo/releases/tag/v0.1.0
