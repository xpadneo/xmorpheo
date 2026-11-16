<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# DOX Framework

## Purpose

This subtree owns the HID-BPF program that rewrites axis fields in input
reports.


## Ownership

- `xmorpheo.bpf.c` - the `hid_bpf_ops` struct_ops program and its maps.
- `xmorpheo.h` - the map layout, the field access, the integer math, and
  the three phases of the rewrite, shared with the host build.
- `xmorpheo_host.c` - host build of the rewrite and the layout, only for
  tests in `xmorpheo/src/bpf.rs`.


## Local Contracts

- C files use GPL-2.0-only, because the kernel only offers HID-BPF to GPL
  programs.
- Keep the program device-agnostic and free of floating point math.
- Keep the integer math in `xmorpheo.h` identical to the Rust reference in
  `xmorpheo/src/curve.rs` and `xmorpheo/src/plan.rs`, and to
  `docs/DESIGN.md`.
- Keep the limits in `xmorpheo.h` in sync with `xmorpheo/src/bpf.rs` and
  `xmorpheo/src/curve.rs`.
- Every loop is bounded by a constant.
- Read all fields of a report before writing any, so outputs never depend
  on the order of the fields or on other outputs.
- Keep the tables in maps that the loader fills after loading. Tables in
  read-only data once let the compiler fold their zero initializer and drop
  the rewrite; a test in `xmorpheo/src/bpf.rs` checks the program size.
- Keep the rewrite logic in `xmorpheo.h`, so the host build tests the same
  code against `Plan::rewrite` in `xmorpheo/src/plan.rs`.
- Leave reports untouched that do not match the expected report ID or
  length, and values outside the logical range.


## Work Guidance


## Verification

- `cargo test` builds the program with clang through `xmorpheo/build.rs`,
  and compares the host build of the math and the layout with the Rust side.
- Loading the program needs root and a kernel with HID-BPF. Report that as
  not verified unless the human ran it.


## Child DOX Index

No child guides are currently needed.
