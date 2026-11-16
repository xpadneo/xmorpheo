<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# DOX Framework

## Purpose

This subtree owns the project documentation and its images. The project
overview is `README.md` in the root, which links here.


## Ownership

- `DESIGN.md` - the architecture and the configuration format.
- `IDEAS.md` - ideas for later that are out of scope for now.


## Local Contracts

- Keep `INSTALL.md`, `PACKAGING.md`, `CONFIG.md`, `FORMULAS.md`,
  `RECIPES.md`, and `CLI.md` in sync with the `Makefile`, `xmorpheo/src/config.rs`,
  `xmorpheo/src/formula.rs`, and `xmorpheo/src/main.rs`.
- New documentation uses GPL-3.0-or-later.
- Keep `DESIGN.md` in sync with the code: describe what exists and what is
  being built, and move ideas from `IDEAS.md` into it only when work starts.
- Delete ideas from `IDEAS.md` once they are done.
- Hardware documentation must distinguish measured facts from inference.
- Show the logo through a `<picture>` element with a `prefers-color-scheme:
  dark` source, so both variants stay in use.


## Work Guidance

- Keep user-facing instructions operational.


## Verification

- Inspect documentation diffs for stale paths and broken cross references.


## Child DOX Index

No child guides are currently needed.
