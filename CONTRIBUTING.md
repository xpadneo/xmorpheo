<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Contributing and licensing

xmorpheo uses file-level licensing via SPDX identifiers.


## License scope

- BPF programs loaded into the kernel (files under `bpf/**`) are licensed under **GPL-2.0-only**.
- The Rust user-space tools (files under `xmorpheo/**`) and the Cargo manifests are licensed under
  **GPL-2.0-or-later**.
- Example configurations and curve profiles meant to be copied (files under `examples/**`) are licensed under
  **CC0-1.0**.
- Everything else (for example documentation and helper scripts) is generally licensed under **GPL-3.0-or-later**,
  unless a file states otherwise.


## Your contribution

By submitting a contribution, you agree that your changes are licensed under the license specified by the SPDX header
in each file you modify.

If you add a new file, you must add an SPDX license identifier at the top of that file and choose the license
appropriate for its scope.


## Branches and commits

The project is in early development. The `main` branch may change quickly, but
each commit must still build and pass the tests on its own.

Prefer small, separable commits. Each commit should address a single concern and remain bisectable — it must build
and behave correctly on its own, without depending on a later commit in the same series to be correct.

Keep refactors, behavior changes, and new features in separate commits, so they
can be reviewed, reverted, or bisected independently.


## Signed-off-by requirement

This project encourages the use of `Signed-off-by` lines in commits to certify that you, as a contributor, adhere to
the Developer's Certificate of Origin (DCO).

- If you **use AI tools** for your contribution (as described in the "Contributions using Artificial Intelligence"
  section), adding your `Signed-off-by` line is **mandatory** to confirm your responsibility for the code.
- If your contribution **does not involve AI tools**, a `Signed-off-by` line is **optional**. While not strictly
  enforced for every commit in this scenario, it is a good practice that certifies your contribution to the project.

For your `Signed-off-by` line, you should use your real name or a pseudonym. The name and email address you use should
identify you consistently across your contributions.


## Contributions using Artificial Intelligence

We acknowledge that Artificial Intelligence (AI) tools, such as code assistants or chatbots, can be useful aids in
software development. Contributions created with the help of such tools are welcome in this project, but they are
subject to clear rules to ensure legal integrity and high code quality.

If you use AI to create your contribution, the full responsibility for the resulting code lies with you as the human
author. You must ensure that the contribution has been fully reviewed, tested, and verified by you. By adding your
`Signed-off-by` line to the commit, you confirm that you are the author of the commit and take full responsibility
for it.

We consider 'help from AI' to be any form of assistance, including, but not limited to:

- Code completion by tools like GitHub Copilot.
- Generation of code blocks by AI models.
- Research and problem-solving using AI chatbots.
- AI-assisted reviews and suggestions (e.g., GitHub Copilot PR suggestions).

Since you, as the human, take responsibility, commits must **not** contain a `Co-authored-by` signature of an AI agent.
The use of AI must be declared accordingly in the pull request.


### AI-generated pull request content

If an AI agent helps prepare a pull request, the following applies in addition to the rules above:

- The agent must not open the pull request itself. It prepares the branch and the pull request text; the human
  contributor opens the pull request and ticks the checkboxes in the template themselves.
- The agent must not tick or reword the checkboxes in the pull request template. They must be copied verbatim.
- The pull request description must not repeat commit message content. It should briefly explain why and how,
  without walls of text.
- Commit messages must stand on their own and use `Fixes:`, `Closes:`, or `Link:` trailers with the full issue or
  pull request URL, not the short `#123` form, to avoid unwanted cross-repository references if the commit is later
  carried into a fork or another tree.
- If the pull request description references its own commits, it should do so only as a bullet list of commit
  titles, not a narrative retelling of what each commit does.


## Reference

See `LICENSE.md` and the full license texts in:

- `LICENSES/LICENSE.gpl-2.0.txt`
- `LICENSES/LICENSE.gpl-3.0.txt`
- `LICENSES/LICENSE.cc0-1.0.txt`
