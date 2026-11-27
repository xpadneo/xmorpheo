<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# DOX Framework

## Purpose

This subtree owns the build of the project website at
<https://xpadneo.github.io/xmorpheo/>: the documentation under `/docs/`,
rendered with mdBook.


## Ownership

- `build.py` - the build, run by `make site`, writes `target/site`.
- `book.toml` - the mdBook configuration.


## Local Contracts

- The Markdown files in `docs/` and `CHANGELOG.md` stay the single source
  of the documentation. `build.py` copies them into a generated book below
  `target/` at build time. Never keep copies of them in `site/`.
- The order of the chapters is `CHAPTERS` in `build.py`. Other
  `docs/*.md` files follow them, `docs/descriptors/*.md` get their own
  part. Guides for agents stay out of the book.
- The build fails on mdBook warnings, and deletes only below `target/`.
- Pin the exact mdBook version.


## Work Guidance

- Use only the Python standard library in `build.py`.


## Verification

- Run `make site` and check that it finishes without warnings.


## Child DOX Index

No child guides are currently needed.
