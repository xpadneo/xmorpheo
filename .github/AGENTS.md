<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# DOX Framework

## Purpose

This subtree owns repository automation and GitHub-facing project metadata.


## Ownership

- Pull request templates and, later, issue templates.
- GitHub Actions workflows.


## Local Contracts

- Keep AI-assistance declarations aligned with `CONTRIBUTING.md`.
- Preserve the human review, human testing, and human sign-off requirement for
  AI-assisted pull requests.
- The HTML comment anchors in `pull_request_template.md` (what agents may
  fill, what must stay verbatim, and that agents must not open the pull
  request themselves) are binding instructions, not suggestions.
- Treat workflow edits as repository behavior changes because they can affect
  checks, notifications, and pull request state.


## Work Guidance

- Keep template language short and actionable.


## Verification

- Inspect diffs for template wording changes.


## Child DOX Index

No child guides are currently needed.
