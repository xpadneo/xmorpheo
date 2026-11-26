<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# DOX Framework

## Purpose

This subtree owns example configurations and curve profiles that users copy
into their own configuration.


## Ownership

- `vkb-gladiator-evo.toml` - expo curves for a pair of VKB Gladiator EVO
  sticks.
- `xbox-wireless-controller.toml` - mild expo curves for the sticks of an
  Xbox Wireless Controller with xpadneo.
- `xbox-combined-triggers.toml` - the same, with both triggers combined
  into one rotation axis.
- `t300rs-swap-pedals.toml` - swaps brake and clutch of a Thrustmaster
  T300 RS, only with the tmff2 driver of hid-tmff2 bound.


## Local Contracts

- Example configurations use CC0-1.0, so they can be copied without
  conditions.
- Use only axes and usages that the device's report descriptor really has.
  Keep the descriptor under `docs/descriptors/` and point to it.
- Follow the configuration format in `docs/CONFIG.md`. Each example is a
  complete drop-in for `/etc/xmorpheo/conf.d`.
- Every example must parse with `xmorpheo::config::Config::parse`. Add a
  test for each new example to `xmorpheo/src/config.rs`.


## Work Guidance


## Verification


## Child DOX Index

No child guides are currently needed.
