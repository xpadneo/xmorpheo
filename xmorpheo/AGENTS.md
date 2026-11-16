<!-- SPDX-License-Identifier: GPL-2.0-or-later -->

# DOX Framework

## Purpose

This subtree owns the Rust crate with the xmorpheo library and the `xmorpheo`
command line tool.


## Ownership

- `build.rs` - builds the BPF program into a libbpf-rs skeleton, and the
  host build of its math for tests.
- `src/bpf.rs` - the skeleton, and the conversion into its map layout.
- `src/confd.rs` - finds, overrides, masks, and merges the configuration
  drop-ins in the `conf.d` directories.
- `src/config.rs` - reader for the TOML configuration.
- `src/descriptor.rs` - HID report descriptor parser.
- `src/curve.rs` - knot fitting for curves, and a reference of the BPF-side
  fixed point math.
- `src/formula.rs` - parser and evaluator for curve formulas.
- `src/plan.rs` - maps a configured device and its descriptor to the
  tables, curves, and knots of the BPF program, and the Rust reference of
  its rewrite.
- `src/usage.rs` - names and default axis kinds of HID axis usages.
- `src/<module>/tests.rs` - unit tests of `src/<module>.rs`.
- `tests/` - tests that only use the public API of the library.
- `tests/data/` - raw report descriptors of real devices, documented in
  `docs/descriptors/`.


## Local Contracts

- New files use GPL-2.0-or-later.
- Field bit offsets include the report ID byte when the descriptor uses report
  IDs, because that is the report layout the BPF program sees.
- Follow the Linux HID core where report descriptors are ambiguous, so fields
  match what the kernel and `hidraw` readers see.
- Curve outputs never leave the logical range of the field.
- Keep the BPF-side integer math in `src/curve.rs` (`eval`, `eval_raw`) in
  sync with the BPF program and with `docs/DESIGN.md`.
- Formulas stay compatible with the stateless part of the ETS2 `controls.sii`
  expression syntax. Document inferred semantics as inferred.
- Keep parsing and computation free of I/O, so they stay unit-testable without
  hardware.


## Work Guidance

- Building needs clang, bpftool, and the libelf and zlib headers. `build.rs`
  generates `vmlinux.h` from `/sys/kernel/btf/vmlinux`, unless
  `XMORPHEO_VMLINUX_H` names a directory that has one.
- Use real descriptor dumps as test fixtures where possible, and name their
  source.


## Verification

- `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test`.


## Child DOX Index

No child guides are currently needed.
