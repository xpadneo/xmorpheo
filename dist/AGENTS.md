<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# DOX Framework

## Purpose

This subtree owns the files that `make install` puts on a system besides
the binary and the examples.


## Ownership

- `60-xmorpheo.rules.in` - the udev rule.


## Local Contracts

- Every `<name>.in` is a template: `make generate`, part of `make`, writes
  it to `target/dist/<name>` with `@BINDIR@`, `@DATADIR@`, `@LIBDIR@`, and
  `@SYSCONFDIR@` replaced. Templates are generated as the user, never by
  `make install` as root.
- `target/dist/paths` records the substituted paths. It is only rewritten
  when they change, which regenerates the templates, and `make install`
  refuses a build with other paths.
- A new kind of generated file needs its destination in the `install` and
  `uninstall` targets of the root `Makefile`; `.rules` files go to
  `UDEVRULESDIR`.
- The rule runs `xmorpheo udev` on `bind` and `remove` of HID devices, not
  on `add`, because the descriptor in sysfs only has the fixups of the
  driver after it bound.
- The rule goes to `/etc/udev/rules.d` for every `PREFIX`, because udev
  reads no rules below `/usr/local` or `/opt`.
- `make install` installs no active configuration, only empty drop-in
  directories, and `make uninstall` keeps them while they have files.
  `INSTALL_SYSCONFDIR=no` installs nothing below `SYSCONFDIR`, for
  packages.


## Work Guidance


## Verification

- `make install-check` installs into `target/install-check`, uninstalls,
  and fails if any file is left behind.
- `make install-check-package` does the same with the layout of a
  distribution package, and fails if anything is installed below `/etc`.


## Child DOX Index

No child guides are currently needed.
