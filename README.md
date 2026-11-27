<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Input Curves for Linux Joysticks and Gamepads

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/img/logo-dark.png">
  <img alt="xmorpheo" src="docs/img/logo-light.png" width="600">
</picture>

xmorpheo reshapes the axes of HID input devices in the kernel, using HID-BPF.
It applies response curves and deadzones to the raw input reports before any
driver or `hidraw` reader sees them. This way, the changes apply to every
consumer of the device:

- native games using evdev or SDL,
- drivers such as [xpadneo](https://github.com/atar-axis/xpadneo),
- games running in Proton or Wine that read the device through `hidraw`.

The device keeps its vendor and product ID, so games that bind controls to a
specific device continue to recognize it.

> [!NOTE]
> xmorpheo is young and has not been tested widely by the community yet. It
> works on the devices we tested, but you may run into bugs, and the
> configuration format may still change before 1.0. Feedback and bug reports
> are very welcome.

Examples in [examples/](examples/) cover the main uses:

- Response curves for flight sticks:
  [vkb-gladiator-evo.toml](examples/vkb-gladiator-evo.toml).
- Response curves for gamepad sticks:
  [xbox-wireless-controller.toml](examples/xbox-wireless-controller.toml).
- Two triggers combined into one axis:
  [xbox-combined-triggers.toml](examples/xbox-combined-triggers.toml).
- Two pedals swapped:
  [t300rs-swap-pedals.toml](examples/t300rs-swap-pedals.toml).

[docs/RECIPES.md](docs/RECIPES.md) has short configurations for inverting,
swapping, combining, and pinning axes, deadzones, and curves.

Tested devices:

- VKB Gladiator EVO L and R, over USB, see
  [docs/descriptors/vkb_gladiator_evo_r.md](docs/descriptors/vkb_gladiator_evo_r.md).
- Xbox Wireless Controller (Series X|S) over Bluetooth LE with xpadneo, see
  [docs/descriptors/xbox_series_xs_ble.md](docs/descriptors/xbox_series_xs_ble.md).


## Quickstart

Needs Linux 6.11 or newer with `CONFIG_HID_BPF` and BTF, and for building
Rust, clang, bpftool, and the libelf and zlib development files, see
[INSTALL.md](docs/INSTALL.md), or [PACKAGING.md](docs/PACKAGING.md) for
distribution packages.

1. Build as your user, and install as root:

   ```sh
   make
   sudo make install
   ```

2. Find your device and its `VENDOR:PRODUCT`:

   ```sh
   xmorpheo list
   ```

3. Write a configuration in `/etc/xmorpheo/conf.d`. Link or copy an
   example from [examples/](examples/), take a recipe from
   [RECIPES.md](docs/RECIPES.md), or start from a template with all axes
   of your device commented out:

   ```sh
   sudo ln -s /usr/local/share/xmorpheo/examples/vkb-gladiator-evo.toml /etc/xmorpheo/conf.d/50-vkb.toml
   xmorpheo template 231d:0200 | sudo tee /etc/xmorpheo/conf.d/50-mystick.toml
   ```

   In a template, uncomment the axes to change and give each a curve, see
   [FORMULAS.md](docs/FORMULAS.md).

4. Look at a curve, optionally as a plot if gnuplot is installed:

   ```sh
   xmorpheo curve "(1 - k) * x + k * pow(x, 3)" -c k=0.6
   xmorpheo curve "(1 - k) * x + k * pow(x, 3)" -c k=0.6 --format gnuplot | gnuplot -p
   ```

5. Check the configuration without loading anything:

   ```sh
   xmorpheo config
   xmorpheo attach --all --dry-run
   ```

6. Let udev attach it, now and on every replug and reboot:

   ```sh
   sudo udevadm control --reload
   sudo udevadm trigger --action=bind --subsystem-match=hid
   ```

   Or attach by hand with `sudo xmorpheo attach --all`. Check the result
   with `evtest` or in a game, and the messages of the udev rule with
   `journalctl -t xmorpheo`.

7. To undo it, `sudo xmorpheo detach --all`, and remove the file from
   `/etc/xmorpheo/conf.d`.

After changing the configuration, run:

```sh
sudo xmorpheo attach --all --replace
```

With an error in the new configuration, the old curves stay. On force
feedback devices like wheels, read
[Safety and Liability](#safety-and-liability) first.


## How It Works

- A small, device-agnostic BPF program attaches to each configured HID device
  and rewrites the selected axis fields of each input report by
  interpolating between the knots of a curve.
- The `xmorpheo` command line tool reads the HID report descriptor, finds the
  axes, fits the knots of each curve from the configuration, and loads the
  program.

See [DESIGN.md](docs/DESIGN.md) for details.


## Documentation

The documentation of the latest release is also on the website,
<https://xpadneo.github.io/xmorpheo/>.

- [INSTALL.md](docs/INSTALL.md): building, installing, and uninstalling.
- [CONFIG.md](docs/CONFIG.md): the configuration file.
- [FORMULAS.md](docs/FORMULAS.md): the curve formulas, with expo curves.
- [RECIPES.md](docs/RECIPES.md): short configurations for common problems.
- [CLI.md](docs/CLI.md): the commands, and troubleshooting.
- [PACKAGING.md](docs/PACKAGING.md): notes for distribution packages.
- [DESIGN.md](docs/DESIGN.md): the architecture.
- [CHANGELOG.md](CHANGELOG.md): the changes of each release.


## Requirements

- Linux 6.11 or newer with `CONFIG_HID_BPF` enabled.
- For building: Rust, clang, bpftool, and the libelf and zlib development
  files, see [INSTALL.md](docs/INSTALL.md).


## Safety and Liability

> [!CAUTION]
> xmorpheo changes what games see from your device. On force feedback
> devices such as wheels and joysticks, a wrong configuration, like an
> inverted or swapped axis, can make the game drive the device with full
> force against its stops, see
> [RECIPES.md](docs/RECIPES.md#invert-an-axis). Depending on the device,
> this can damage the hardware or hurt hands, fingers, or thumbs. Test new configurations
> with the force feedback of the game turned down or off, and keep your
> hands clear of the spokes of a wheel and the gaps at the base of a
> stick.
>
> You use xmorpheo at your own risk. It comes without any warranty, as its
> licenses state: sections 11 and 12 of the GPL-2.0 for the BPF program and
> the tool, sections 15 and 16 of the GPL-3.0 for the documentation, and
> section 4 of CC0-1.0 for the examples, see [LICENSE.md](LICENSE.md). The
> authors are not liable for damage to devices or for injuries caused by
> its use or by configurations.


## AI Transparency

I develop xmorpheo with substantial help from an AI coding agent, Claude
Code by Anthropic. The agent wrote most of the code, the tests, and drafts
of the documentation, and did much of the research, like reading the
HID-BPF interfaces of the kernel and the descriptor fixups of drivers such
as xpadneo and hid-tmff2.

I set the goals and make the design and architecture decisions, like
keeping the report descriptor unchanged. I review every change, correct
the agent where it is wrong, and take responsibility for the result. The
tests on real hardware are my own: VKB Gladiator EVO sticks, and an Xbox
Series X|S controller with xpadneo, and a Thrustmaster T300 RS with
hid-tmff2 in Euro Truck Simulator 2. Examples and recipes for other devices
say so when they are not tested on the device yet. Privileged steps, like
loading the BPF program into the kernel, are only run by me.

What keeps the work in check:

- Automated tests, among them a build of the math of the BPF program on
  the host that is compared to the Rust reference on every raw value, and
  a check of the compiled BPF program against the expected size.
- Small commits that each build and pass the tests on their own.
- The `AGENTS.md` guides in the repository, the working rules for AI
  agents.

Every commit carries my `Signed-off-by`. The commits have no
`Co-authored-by` lines for the agent, because I take responsibility for
them, as
[CONTRIBUTING.md](CONTRIBUTING.md#contributions-using-artificial-intelligence)
requires. This section discloses the help instead.

Contributions made with the help of AI are welcome under the same rules:
you take responsibility for your contribution, you understand and have
tested what you submit, and you declare the help of AI in the pull request.


## Other Projects

- [udev-hid-bpf](https://gitlab.freedesktop.org/libevdev/udev-hid-bpf) loads
  device-specific HID-BPF programs that fix broken devices.
- [xpadneo](https://github.com/atar-axis/xpadneo) is an advanced Linux driver
  for Xbox wireless controllers.
