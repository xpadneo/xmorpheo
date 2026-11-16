<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Licensing

This repository contains components under different licenses.


## Summary

- BPF programs that are loaded into the kernel, especially files under:
  - `bpf/**`
  are licensed under **GPL-2.0-only**. The kernel only exposes the HID-BPF
  interfaces to GPL-compatible programs.

- The Rust user-space tools (`xmorpheo/**`) and the Cargo manifests are
  licensed under **GPL-2.0-or-later**, so their code can move into GPL-2.0
  projects such as xpadneo or the kernel.

- Example configurations and curve profiles meant to be copied are dedicated
  to the public domain under **CC0-1.0**, especially files under:
  - `examples/**`

- Everything else is GPL-3.0-or-later **unless file headers specify otherwise**. This includes:
  - Documentation (`docs/**`)
  - Tooling and helper scripts


## Canonical license texts

The full license texts are provided in:

- [GPL-2.0-only and GPL-2.0-or-later](LICENSES/LICENSE.gpl-2.0.txt)
- [GPL-3.0-or-later](LICENSES/LICENSE.gpl-3.0.txt)
- [CC0-1.0](LICENSES/LICENSE.cc0-1.0.txt)


## File-level license identifiers (SPDX)

This project uses SPDX license identifiers in file headers.

- For C, BPF, and Rust files:
  - `// SPDX-License-Identifier: GPL-2.0-only` (BPF programs)
  - or `// SPDX-License-Identifier: GPL-2.0-or-later` (Rust user-space tools)
- For Markdown files:
  - `<!-- SPDX-License-Identifier: ... -->`
- For TOML and shell files:
  - `# SPDX-License-Identifier: ...`

The SPDX identifier in each file is the authoritative license marker for that file.


## Contributor guidance

By submitting contributions, you agree that your changes are licensed under the license indicated by the SPDX header
of the file(s) you modify.

If a new file is added, include an SPDX license identifier at the top of the file and choose the appropriate license
for its scope.
