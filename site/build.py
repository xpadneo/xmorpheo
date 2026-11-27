#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the website into target/site: the documentation under docs/ with
# mdBook. The Markdown files stay the single source: they are copied into
# a generated book below target/ at build time.
#
#   python3 site/build.py [--out target/site] [--mdbook mdbook]
#
# Only the Python standard library is used.

import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "site"
TARGET = ROOT / "target"

# The chapters of the book in this order, by their path in the repository.
# Other docs/*.md files follow them, docs/descriptors/*.md come in their
# own part, and the changelog last.
CHAPTERS = [
    "docs/INSTALL.md",
    "docs/CONFIG.md",
    "docs/FORMULAS.md",
    "docs/RECIPES.md",
    "docs/CLI.md",
    "docs/PACKAGING.md",
    "docs/DESIGN.md",
    "docs/IDEAS.md",
]
CHANGELOG = "CHANGELOG.md"
# Guides for agents, not for readers.
SKIP = {"docs/AGENTS.md"}


def fail(message):
    print(f"build.py: {message}", file=sys.stderr)
    sys.exit(1)


def below_target(path):
    """Resolves path and makes sure it is below target/, so it is safe to
    delete."""
    path = path.resolve()
    if TARGET.resolve() not in path.parents:
        fail(f"{path} is not below {TARGET}")
    return path


def book_path(repo_path):
    """The path of a repository file in the book: docs/ is the root of the
    book, the changelog sits next to it."""
    return repo_path.removeprefix("docs/")


def title(text, repo_path):
    match = re.search(r"^# (.+)$", text, re.MULTILINE)
    if not match:
        fail(f"{repo_path} has no title")
    return match.group(1).strip()


def collect(content):
    """The repository paths of the book pages below content, in the order
    of the book: chapters, descriptors, changelog."""
    for path in CHAPTERS + [CHANGELOG]:
        if not (content / path).is_file():
            fail(f"{path} is missing")
    docs = sorted(
        p.relative_to(content).as_posix() for p in (content / "docs").glob("*.md")
    )
    chapters = CHAPTERS + [p for p in docs if p not in CHAPTERS and p not in SKIP]
    descriptors = sorted(
        p.relative_to(content).as_posix()
        for p in (content / "docs/descriptors").glob("*.md")
    )
    return chapters, descriptors


def summary(content, chapters, descriptors):
    def entry(path):
        text = (content / path).read_text(encoding="utf-8")
        return f"- [{title(text, path)}]({book_path(path)})"

    lines = ["# Summary", ""]
    lines += [entry(p) for p in chapters]
    if descriptors:
        lines += ["", "# Device Descriptors", ""]
        lines += [entry(p) for p in descriptors]
    lines += ["", "---", "", f"[Changelog]({book_path(CHANGELOG)})", ""]
    return "\n".join(lines)


def build_book(content, work, out, mdbook):
    chapters, descriptors = collect(content)
    src = work / "src"
    src.mkdir(parents=True)
    for path in chapters + descriptors + [CHANGELOG]:
        dest = src / book_path(path)
        dest.parent.mkdir(parents=True, exist_ok=True)
        text = (content / path).read_text(encoding="utf-8")
        dest.write_text(text, encoding="utf-8")
    (src / "SUMMARY.md").write_text(
        summary(content, chapters, descriptors), encoding="utf-8"
    )
    shutil.copy(SITE / "book.toml", work / "book.toml")
    result = subprocess.run(
        [mdbook, "build", "--dest-dir", str(out / "docs"), str(work)],
        capture_output=True,
        text=True,
    )
    sys.stderr.write(result.stderr)
    if result.returncode != 0:
        fail("mdbook failed")
    # mdBook reports broken links and other problems as warnings, and the
    # site must build without them.
    if re.search(r"\b(WARN|ERROR)\b", result.stderr):
        fail("mdbook printed warnings")


def main():
    parser = argparse.ArgumentParser(description="Builds the website.")
    parser.add_argument("--out", default="target/site", help="output directory")
    parser.add_argument("--mdbook", default="mdbook", help="the mdbook command")
    args = parser.parse_args()

    out = below_target(ROOT / args.out)
    work = below_target(TARGET / "site-build")
    for path in (out, work):
        if path.exists():
            shutil.rmtree(path)
    out.mkdir(parents=True)

    build_book(ROOT, work / "book", out, args.mdbook)
    print(f"built {out.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
