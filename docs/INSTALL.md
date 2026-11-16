<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Installing

## Requirements

- Linux 6.11 or newer with `CONFIG_HID_BPF`, and the BPF filesystem mounted
  at `/sys/fs/bpf`. systemd mounts it at boot.
- For building: Rust, clang, bpftool, and the libelf and zlib development
  files. On Debian: `apt install clang llvm bpftool libelf-dev zlib1g-dev`.
  The build reads the BTF of the running kernel from
  `/sys/kernel/btf/vmlinux`.


## Install

Build as your user, then install as root:

```sh
make
sudo make install
```

`make` builds the binary and generates the other files to install, like
the udev rule, from their templates in `dist/` into `target/dist`.
`make generate` does only the latter. It refuses to run as root, because that would leave files
owned by root in the source tree and in the home of root. `sudo make
install` only copies, and asks to run `make` first if the build is
missing. Package
builds that run as root set `ALLOW_ROOT_BUILD=1`.

This installs:

| File | Purpose |
|---|---|
| `/usr/local/bin/xmorpheo` | the tool |
| `/etc/udev/rules.d/60-xmorpheo.rules` | attaches configured devices when they appear |
| `/usr/local/share/xmorpheo/examples/` | example configurations |
| `/usr/local/lib/xmorpheo/conf.d/` | empty, for configuration that comes with an installation |
| `/etc/xmorpheo/conf.d/` | empty, for your configuration |

No configuration is active after installing. `PREFIX` selects another
location. The build puts the installation paths into the generated files,
like the udev rule, so give the same `PREFIX` to `make`, `make install`,
and `make uninstall`:

```sh
make PREFIX=/opt/xmorpheo
sudo make PREFIX=/opt/xmorpheo install
```

`make install` stops if the build used another `PREFIX`, instead of
generating files as root. The udev rule goes to
`/etc/udev/rules.d`, because udev reads no rules below `/usr/local` or
`/opt`. Packages install nothing below `/etc`, see
[PACKAGING.md](PACKAGING.md).

The tool can also be built in a container, for example a distrobox, and
installed on the host: `make` in the container, `sudo make install` on the
host. The binary needs `libelf.so.1`, `libz.so.1`, and `libzstd.so.1` on the
host.


## Configure

The configuration is made of `*.toml` files in `/etc/xmorpheo/conf.d`
and the other drop-in directories, see [CONFIG.md](CONFIG.md). To use an
example as it is, link it, so it follows updates:

```sh
sudo ln -s /usr/local/share/xmorpheo/examples/vkb-gladiator-evo.toml /etc/xmorpheo/conf.d/50-vkb.toml
```

To change it, copy it instead, or start from a template of your devices:

```sh
sudo cp /usr/local/share/xmorpheo/examples/vkb-gladiator-evo.toml /etc/xmorpheo/conf.d/50-vkb.toml
xmorpheo template 231d:0200 | sudo tee /etc/xmorpheo/conf.d/50-vkb.toml
```

Check which files are read, and the result, without loading anything:

```sh
xmorpheo config
xmorpheo attach --all --dry-run
```

Then let udev attach the configured devices, which also happens on every
replug and reboot:

```sh
sudo udevadm control --reload
sudo udevadm trigger --action=bind --subsystem-match=hid
```

`--action=bind`, not `add`: the rule attaches when a driver binds to the
device, because only then the report descriptor has the fixups of the
driver. After changing the configuration, run
`sudo xmorpheo attach --all --replace`. It keeps the old programs if the
new configuration has an error.

Calibrate a device in its own tool first, for example VKBDevCfg for VKB
sticks, and check with `evtest` that a released stick rests in the middle
of its range. A curve shapes the response, but it cannot fix an off-center
rest position. See [CLI.md](CLI.md#troubleshooting).


## Uninstall

```sh
sudo make uninstall
sudo udevadm control --reload
```

This detaches all programs and removes every installed file. Drop-in
directories that still have configuration files stay, remove them by hand
if you no longer need them. The evdev fuzz and flat stay 0 until the device is plugged in again.
