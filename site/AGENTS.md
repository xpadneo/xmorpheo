<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# DOX Framework

## Purpose

This subtree owns the project website at
<https://xpadneo.github.io/xmorpheo/>: a hand-made landing page, and the
documentation under `/docs/`, rendered with mdBook.


## Ownership

- `build.py` - the build, run by `make site`, writes `target/site`.
- `index.html.in` and `style.css` - the landing page, plain HTML and CSS
  without JavaScript.
- `book.toml` - the mdBook configuration.
- `docs.css` - extra styles for the book.


## Local Contracts

- The Markdown files in `docs/` and `CHANGELOG.md` stay the single source
  of the documentation. `build.py` copies them into a generated book below
  `target/` at build time. Never keep copies of them in `site/`.
- The order of the chapters is `CHAPTERS` in `build.py`. Other
  `docs/*.md` files follow them, `docs/descriptors/*.md` get their own
  part. Guides for agents stay out of the book.
- The documentation comes from a git revision, by default the latest `v*`
  tag, never from the working tree. Every page names it below its title,
  with a link to the release.
- Links within the book stay relative. Links to other files of the
  repository, like `../examples/*.toml` or `README.md`, become GitHub URLs
  at the documented revision. A link to a missing file fails the build.
- The build fails on mdBook warnings and errors, and on links to missing
  pages or anchors in the output. It deletes only below `target/`.
- The landing page and the build come from the working tree. `build.py`
  replaces `@VERSION_LINE@` and `@REF@` in `index.html.in`, and fails on
  GitHub links at `@REF@` to files missing in the documented revision.
- The landing page states only what `README.md`, `docs/RECIPES.md`, and
  the examples state, and changes with them. Each use case links to its
  recipe or example.
- Keep the landing page device- and game-agnostic: xmorpheo works with any
  HID device and any game, because it changes the reports in the kernel.
  Name devices and games only as tested examples.
- Keep the caution for force feedback devices and the link to the AI
  Transparency section of `README.md`.
- Show the logo through a `<picture>` element with a
  `prefers-color-scheme: dark` source, like `docs/AGENTS.md` requires.
- English only.
- Pin the exact mdBook version.


## Work Guidance

- Use only the Python standard library in `build.py`.
- Take the colors of the landing page from the logo, in a light and a dark
  scheme.


## Verification

- Run `make site` and `make site TAG=HEAD`, and check that both finish
  without warnings.
- Preview with `python3 -m http.server -d target/site`, in a light and a
  dark color scheme and at the width of a phone.


## Child DOX Index

No child guides are currently needed.
