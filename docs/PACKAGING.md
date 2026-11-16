<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Packaging

Notes for distribution package maintainers. [INSTALL.md](INSTALL.md)
covers building and installing by hand.


## Build and Install

```sh
make PREFIX=/usr UDEVRULESDIR=/usr/lib/udev/rules.d INSTALL_SYSCONFDIR=no
make PREFIX=/usr UDEVRULESDIR=/usr/lib/udev/rules.d INSTALL_SYSCONFDIR=no \
	DESTDIR="$pkgdir" install
```

Give the same variables to both steps. The build puts the paths into the
binary and into generated files like the udev rule, records them in
`target/dist/paths`, and `make install` refuses a build with other paths.
`make install` only copies, it never builds or generates anything.

`make install-check-package` installs with exactly these settings into a
directory below `target`, fails if anything ends up below `/etc`, and
uninstalls again.


## Variables

| Variable | Default | Meaning |
|---|---|---|
| `PREFIX` | `/usr/local` | base of the other paths; packages use `/usr` |
| `BINDIR` | `$(PREFIX)/bin` | the binary; the udev rule calls it by this path |
| `DATADIR` | `$(PREFIX)/share/xmorpheo` | the examples, in `examples/` |
| `LIBDIR` | `$(PREFIX)/lib/xmorpheo` | `conf.d` for configuration shipped with the package |
| `SYSCONFDIR` | `/etc` | where the tool looks for `xmorpheo/conf.d` of the administrator |
| `INSTALL_SYSCONFDIR` | `yes` | `no` installs nothing below `SYSCONFDIR`; packages use `no` |
| `UDEVRULESDIR` | `/etc/udev/rules.d` | the udev rule; packages use `/usr/lib/udev/rules.d` |
| `DESTDIR` | empty | staging directory for `make install` |
| `ALLOW_ROOT_BUILD` | empty | `1` allows `make` as root, for build environments that run as root |
| `CARGO` | `cargo` | the cargo binary |

`SYSCONFDIR` and `LIBDIR` reach the binary at build time, through the
environment variables `XMORPHEO_SYSCONFDIR` and `XMORPHEO_LIBDIR` of cargo.
`xmorpheo config` shows the directories a binary uses. A distribution that
keeps the administrator's files in `/usr/etc` builds with
`SYSCONFDIR=/usr/etc`.


## What to Ship

- `/usr/bin/xmorpheo`.
- `/usr/lib/udev/rules.d/60-xmorpheo.rules`. A rule of the same name in
  `/etc/udev/rules.d` overrides it, as usual for udev.
- `/usr/share/xmorpheo/examples/*.toml`, the example configurations. They
  are not active.
- `/usr/lib/xmorpheo/conf.d/`, empty, or with configuration files the
  package wants active. The administrator overrides a file there with one
  of the same name in `/etc/xmorpheo/conf.d`, or masks it with an empty file
  or a symlink to `/dev/null`, see [CONFIG.md](CONFIG.md#files).
- Nothing below `/etc`. xmorpheo works without `/etc/xmorpheo/conf.d`, a
  missing drop-in directory counts as empty.

The licenses are in `LICENSES/`, see [LICENSE.md](../LICENSE.md): the BPF
program is GPL-2.0-only, the tool GPL-2.0-or-later, the documentation
GPL-3.0-or-later, and the examples CC0-1.0.


## After Installing and Removing

Reload the udev rules, and let them attach to devices that are already
connected:

```sh
udevadm control --reload
udevadm trigger --action=bind --subsystem-match=hid
```

`--action=bind`, because the rule attaches when a driver binds to a device.
Before removing the package, detach the programs:

```sh
xmorpheo detach --all
```

The programs stay attached until then, or until the devices go away.


## Build Requirements

- Rust with cargo. The build uses `--locked`. For offline builds, vendor the
  dependencies with `cargo vendor` and point cargo to them, and keep
  `CARGO_HOME` inside the build directory if the build environment requires
  that.
- clang with the BPF target, and bpftool.
- The libelf and zlib development files. libbpf is built from the copy that
  libbpf-sys vendors.
- The BTF of a kernel: the build generates `vmlinux.h` from
  `/sys/kernel/btf/vmlinux` of the build host. Build hosts without it, or
  reproducible builds, set `XMORPHEO_VMLINUX_H` to a directory with a
  prepared `vmlinux.h`. The program only uses `hid_bpf_ops` and
  `hid_bpf_ctx`, which libbpf relocates with CO-RE, so the header does not
  have to match the target kernel exactly.


## Runtime Requirements

- Linux 6.11 or newer with `CONFIG_HID_BPF` and BTF, for HID-BPF with
  `struct_ops`.
- The BPF filesystem at `/sys/fs/bpf`. systemd mounts it.
- `libelf.so.1`, `libz.so.1`, and `libzstd.so.1`.
- Loading needs `CAP_BPF` and `CAP_SYS_ADMIN`. The udev rule runs as root.
