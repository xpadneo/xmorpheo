// SPDX-License-Identifier: GPL-2.0-or-later

//! Response curves for HID joysticks and gamepads using HID-BPF.

pub mod bpf;
pub mod confd;
pub mod config;
pub mod curve;
pub mod descriptor;
pub mod evdev;
pub mod formula;
pub mod loader;
pub mod plan;
pub mod sysfs;
pub mod usage;
