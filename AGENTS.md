<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# DOX Framework

- DOX is the agent-guide hierarchy installed here.
- Agents must follow DOX instructions across edits.


## Core Contract

- Agent guides are binding work contracts for their subtrees.
- Work products, source materials, instructions, records, assets, and durable
  docs must stay understandable from the nearest applicable guide plus every
  parent guide above it.
- Follow `CONTRIBUTING.md` for licensing, commit rules, AI assistance, and
  sign-off requirements.
- AI-assisted work must remain human-owned: a human reviews, tests, signs off,
  and takes responsibility for submitted commits.
- Write AI-assisted text as the human author's own statement. Do not narrate
  the human in the third person ("the owner confirmed", "the user tested");
  state verified results directly and attribute only third parties, such as
  other reporters or linked issues.
- AI agents must not add `Co-authored-by` trailers for themselves or other AI
  tools.
- AI-assisted pull requests must declare AI assistance in the pull request
  template.
- Agents must not open pull requests for external contributions on a human's
  behalf; they prepare the branch and text, and the human opens the pull
  request and ticks the template checkboxes themselves.
- Agents are collaborators, not copy-paste text generators: verify and engage
  with the human before producing unprompted pull request or commit text that
  a human is expected to paste as-is.


## Read Before Editing

1. Read the root guide.
2. Identify every file or folder you expect to touch.
3. Walk from the repository root to each target path.
4. Read every guide found along each route.
5. If a parent guide lists a child guide whose scope contains the path, read
   that child and continue from there.
6. Use the nearest guide as the local contract and parent docs for repo-wide
   rules.
7. If docs conflict, the closer doc controls local work details, but no child
   doc may weaken DOX.

Do not rely on memory. Re-read the applicable DOX chain in the current session
before editing.

Also read `CONTRIBUTING.md` before contribution, branch, commit, or
AI-attribution decisions.


## Update After Editing

Every meaningful change requires a DOX pass before the task is done.

Update the closest owning guide when a change affects:

- purpose, scope, ownership, or responsibilities
- durable structure, contracts, workflows, or operating rules
- required inputs, outputs, permissions, constraints, side effects, or artifacts
- user preferences about behavior, communication, process, organization, or
  quality
- guide creation, deletion, move, rename, or index contents

Update parent docs when parent-level structure, ownership, workflow, or child
index changes. Update child docs when parent changes alter local rules. Remove
stale or contradictory text immediately. Small edits that do not change behavior
or contracts may leave docs unchanged, but the DOX pass still must happen.


## Hierarchy

- The root guide is the DOX rail for project-wide instructions, global
  preferences, durable workflow rules, and the top-level Child DOX Index.
- Child guides own domain-specific instructions and their own Child DOX Index.
- Each parent explains what its direct children cover and what stays owned by
  the parent.
- The closer a doc is to the work, the more specific and practical it must be.


## Child Doc Shape

- Create a child guide when a folder becomes a durable boundary with its own
  purpose, rules, responsibilities, workflow, materials, or quality standards.
- Work Guidance must reflect the current standards of the project or user
  instructions. If there are no specific standards yet, leave it empty.
- Verification must reflect an existing check. If no verification framework
  exists yet, leave it empty and update it when one exists.

Default section order:

- Purpose
- Ownership
- Local Contracts
- Work Guidance
- Verification
- Child DOX Index


## Style

- Keep docs concise, current, and operational.
- Document stable contracts, not diary entries.
- Put broad rules in parent docs and concrete details in child docs.
- Prefer direct bullets with explicit names.
- Do not duplicate rules across many files unless each scope needs a local
  version.
- Delete stale notes instead of explaining history.
- Trim obvious statements, repeated rules, misplaced detail, and warnings for
  risks that no longer exist.


## Project Work Guidance

- Keep commits small, atomic, separable, and bisectable. Each commit must
  build and pass `cargo test` on its own.
- Do not mix refactors with behavior changes or new features.
- Keep tests out of the code they test, like Go: unit tests of a Rust module
  `src/foo.rs` go into `src/foo/tests.rs`, included with
  `#[cfg(test)] mod tests;`. Tests that only need the public API go into
  the `tests/` directory of the crate.
- Start simple. Build the smallest working piece first and keep the design
  open for the ideas in `docs/IDEAS.md` instead of building them early.
- Ask the human when planning decisions are unclear, instead of guessing.
- Write spontaneous ideas that are out of scope into `docs/IDEAS.md`.
- Keep the BPF program device-agnostic and free of floating point math. Put
  device knowledge and math into the Rust user-space tool.
- Do not run privileged commands, such as loading BPF programs or writing to
  `/sys` or `/etc`, unless the human explicitly asks for that action.
- Note user-visible changes in `CHANGELOG.md` under "Unreleased", and
  move them under the version when releasing.
- Keep the "AI Transparency" section of `README.md` current when the use
  of AI agents or the tested hardware changes.
- Do not push, merge, close issues, resolve review threads, or post GitHub
  comments unless explicitly asked.


## Verification

- Run `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test` for
  Rust changes.
- Run `markdownlint-cli2` on changed Markdown files. It reads
  `.markdownlint.jsonc`, and `.github/.markdownlint.jsonc` extends it.
- Report any verification that could not be run, especially behavior that
  needs real hardware or a kernel with HID-BPF.


## Closeout

1. Re-check changed paths against the DOX chain.
2. Update nearest owning docs and any affected parents or children.
3. Refresh every affected Child DOX Index.
4. Remove stale or contradictory text.
5. Run existing verification when relevant.
6. Report any docs intentionally left unchanged and why.


## User Preferences

When the user requests a durable behavior change, record it here or in the
relevant child guide.


## Child DOX Index

- `.github/` - pull request template and future CI workflows.
- `bpf/` - the HID-BPF program under GPL-2.0-only.
- `dist/` - the udev rule that `make install` installs.
- `docs/` - user-facing documentation, the design document, the list of
  ideas, and images.
- `examples/` - example configurations and curve profiles under CC0-1.0.
- `site/` - the build of the project website.
- `xmorpheo/` - the Rust crate with the library and the `xmorpheo` command
  line tool.

The root keeps ownership of `README.md`, `CHANGELOG.md`, top-level
project metadata, licensing, contribution policy, the Cargo workspace
manifest, the `Makefile` for installing, and editor configuration.
