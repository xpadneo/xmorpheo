#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the website into target/site: the landing page from index.html.in,
# and the documentation under docs/ with mdBook. The Markdown files stay
# the single source: they are copied into a generated book below target/
# at build time.
#
#   python3 site/build.py [--tag vX.Y.Z] [--out target/site] [--mdbook mdbook]
#
# The documentation comes from a release tag, by default the latest v* tag,
# and links that leave the book point to GitHub at that tag. The landing
# page and the build itself come from the working tree. Any other git
# revision works too, like HEAD for a preview of unreleased documentation.
#
# Only the Python standard library is used.

import argparse
import html
import html.parser
import io
import os
import posixpath
import re
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "site"
TARGET = ROOT / "target"
REPO = "https://github.com/xpadneo/xmorpheo"
SITE_URL = "https://xpadneo.github.io/xmorpheo/"
# The exact version of mdBook, the workflow installs it with
# "build.py --mdbook-version".
MDBOOK_VERSION = "0.5.4"
# Pages of mdBook that are not for search engines. docs/index.html is a
# copy of the first chapter.
NOT_IN_SITEMAP = {"docs/404.html", "docs/index.html", "docs/print.html", "docs/toc.html"}

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


def git(*args, binary=False):
    result = subprocess.run(
        ["git", "-C", str(ROOT), *args], capture_output=True, text=not binary
    )
    if result.returncode != 0:
        stderr = result.stderr
        if binary:
            stderr = stderr.decode(errors="replace")
        fail(f"git {' '.join(args)} failed: {stderr.strip()}")
    return result.stdout


class Revision:
    """The documented revision: its name, the ref for GitHub links, and
    the line every page shows."""

    def __init__(self, name):
        if name is None:
            tags = git("tag", "--list", "v*", "--sort=-v:refname").split()
            if not tags:
                fail("no v* tag, give a revision with --tag")
            name = tags[0]
        self.name = name
        self.commit = git("rev-parse", "--verify", f"{name}^{{commit}}").strip()
        tag = subprocess.run(
            ["git", "-C", str(ROOT), "rev-parse", "--verify", "-q", f"refs/tags/{name}"],
            capture_output=True,
        )
        self.is_tag = tag.returncode == 0
        self.ref = name if self.is_tag else self.commit
        if self.is_tag:
            self.label = name
            self.url = f"{REPO}/releases/tag/{name}"
        else:
            self.label = f"commit {self.commit[:12]}"
            self.url = f"{REPO}/tree/{self.commit}"

    def version_text(self):
        link = f'<a href="{html.escape(self.url)}">{html.escape(self.label)}</a>'
        if self.is_tag:
            return f"Documentation for xmorpheo {link}"
        return f"Documentation for xmorpheo at {link}, not a release"

    def version_line(self):
        return f'<p class="doc-version">{self.version_text()}</p>'

    def extract(self, dest, paths):
        """Writes the given paths of the revision below dest."""
        data = git("archive", "--format=tar", self.commit, *paths, binary=True)
        with tarfile.open(fileobj=io.BytesIO(data)) as archive:
            archive.extractall(dest, filter="data")

    def kind(self, path):
        """blob or tree for a path in the revision, None if it is missing."""
        result = subprocess.run(
            ["git", "-C", str(ROOT), "cat-file", "-t", f"{self.commit}:{path}"],
            capture_output=True,
            text=True,
        )
        return result.stdout.strip() if result.returncode == 0 else None


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


# Inline links and images, and reference definitions. Code is masked first.
LINK = re.compile(r"(!?)\[([^\]]*)\]\(([^)\s]+)((?:\s+\"[^\"]*\")?)\)")
REFERENCE = re.compile(r"^( {0,3}\[[^\]]+\]:[ \t]*)(\S+)", re.MULTILINE)
FENCE = re.compile(r"^ {0,3}(`{3,}|~{3,})")
CODE_SPAN = re.compile(r"(`+)(?:(?!\1).)+?\1")


def split_code(text):
    """Splits text into (is_code, part) pairs: fenced code blocks and code
    spans are code."""
    parts = []
    prose = []
    fence = None
    for line in text.splitlines(keepends=True):
        match = FENCE.match(line)
        if fence is None and match:
            fence = match.group(1)
            parts.append((False, "".join(prose)))
            prose = []
            parts.append((True, line))
        elif fence is not None:
            parts.append((True, line))
            if match and match.group(1)[0] == fence[0] and len(match.group(1)) >= len(fence):
                fence = None
        else:
            prose.append(line)
    parts.append((False, "".join(prose)))
    result = []
    for is_code, part in parts:
        if is_code:
            result.append((True, part))
            continue
        last = 0
        for match in CODE_SPAN.finditer(part):
            result.append((False, part[last : match.start()]))
            result.append((True, match.group(0)))
            last = match.end()
        result.append((False, part[last:]))
    return result


def rewrite_url(url, repo_path, pages, revision, image=False):
    """The URL of a link in repo_path, for the book: links within the book
    stay relative, links to other files of the repository go to GitHub at
    the documented revision."""
    if re.match(r"[A-Za-z][A-Za-z0-9+.-]*:", url) or url.startswith(("#", "/")):
        return url
    path, hash_, fragment = url.partition("#")
    target = posixpath.normpath(posixpath.join(posixpath.dirname(repo_path), path))
    if target.startswith("../"):
        fail(f"{repo_path}: {url} leaves the repository")
    if target in pages:
        here = posixpath.dirname(book_path(repo_path)) or "."
        return posixpath.relpath(book_path(target), here) + hash_ + fragment
    kind = revision.kind(target)
    if kind is None:
        fail(f"{repo_path}: {url} does not exist in {revision.name}")
    if image and kind == "blob":
        return f"{REPO}/raw/{revision.ref}/{target}"
    return f"{REPO}/{kind}/{revision.ref}/{target}{hash_}{fragment}"


def rewrite_links(text, repo_path, pages, revision):
    def inline(match):
        image, label, url, link_title = match.groups()
        url = rewrite_url(url, repo_path, pages, revision, image=bool(image))
        return f"{image}[{label}]({url}{link_title})"

    def reference(match):
        return match.group(1) + rewrite_url(match.group(2), repo_path, pages, revision)

    parts = []
    for is_code, part in split_code(text):
        if not is_code:
            part = REFERENCE.sub(reference, LINK.sub(inline, part))
        parts.append(part)
    return "".join(parts)


def add_version_line(text, revision):
    """Puts the version line below the title of a page."""
    return re.sub(
        r"^# .+$",
        lambda match: f"{match.group(0)}\n\n{revision.version_line()}",
        text,
        count=1,
        flags=re.MULTILINE,
    )


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


def check_mdbook(mdbook):
    try:
        result = subprocess.run([mdbook, "--version"], capture_output=True, text=True)
    except FileNotFoundError:
        fail(f"{mdbook} not found, install mdBook {MDBOOK_VERSION}")
    version = result.stdout.split()[-1].removeprefix("v") if result.stdout else ""
    if version != MDBOOK_VERSION:
        fail(f"{mdbook} is version {version or 'unknown'}, the site needs {MDBOOK_VERSION}")


def build_book(revision, work, out, mdbook):
    content = work / "content"
    revision.extract(content, ["docs", CHANGELOG])
    chapters, descriptors = collect(content)
    pages = set(chapters + descriptors + [CHANGELOG])
    src = work / "src"
    src.mkdir(parents=True)
    for path in chapters + descriptors + [CHANGELOG]:
        dest = src / book_path(path)
        dest.parent.mkdir(parents=True, exist_ok=True)
        text = (content / path).read_text(encoding="utf-8")
        text = add_version_line(rewrite_links(text, path, pages, revision), revision)
        dest.write_text(text, encoding="utf-8")
    (src / "SUMMARY.md").write_text(
        summary(content, chapters, descriptors), encoding="utf-8"
    )
    shutil.copy(SITE / "book.toml", work / "book.toml")
    shutil.copy(SITE / "docs.css", work / "docs.css")
    env = dict(os.environ, MDBOOK_BOOK__TITLE=f"xmorpheo {revision.label}")
    result = subprocess.run(
        [mdbook, "build", "--dest-dir", str(out / "docs"), str(work)],
        capture_output=True,
        text=True,
        env=env,
    )
    sys.stderr.write(result.stderr)
    if result.returncode != 0:
        fail("mdbook failed")
    # mdBook reports broken links and other problems as warnings, and the
    # site must build without them.
    if re.search(r"\b(WARN|ERROR)\b", result.stderr):
        fail("mdbook printed warnings")


def build_landing(revision, out):
    """Writes the landing page with its style and the logo."""
    page = (SITE / "index.html.in").read_text(encoding="utf-8")
    # Links to files at the documented revision must exist there.
    for path in re.findall(rf"{re.escape(REPO)}/blob/@REF@/([^\"#]+)", page):
        if revision.kind(path) != "blob":
            fail(f"index.html.in: {path} does not exist in {revision.name}")
    page = page.replace("@VERSION_LINE@", revision.version_text())
    page = page.replace("@REF@", revision.ref)
    page = page.replace("@SITE_URL@", SITE_URL)
    left = re.findall(r"@[A-Z_]+@", page)
    if left:
        fail(f"index.html.in: unknown placeholders {', '.join(sorted(set(left)))}")
    (out / "index.html").write_text(page, encoding="utf-8")
    shutil.copy(SITE / "style.css", out / "style.css")
    (out / "img").mkdir()
    for logo in ("logo-light.png", "logo-dark.png"):
        shutil.copy(ROOT / "docs/img" / logo, out / "img" / logo)


def build_sitemap(revision, out):
    """Lists the landing page and the pages of the book, with the date of
    the documented commit."""
    date = git("show", "-s", "--format=%cs", revision.commit).strip()
    paths = sorted(
        p.relative_to(out).as_posix()
        for p in out.rglob("*.html")
        if p.relative_to(out).as_posix() not in NOT_IN_SITEMAP
    )
    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
    ]
    for path in paths:
        url = SITE_URL + path.removesuffix("index.html")
        lines.append(f"  <url><loc>{html.escape(url)}</loc><lastmod>{date}</lastmod></url>")
    lines += ["</urlset>", ""]
    (out / "sitemap.xml").write_text("\n".join(lines), encoding="utf-8")


class Anchors(html.parser.HTMLParser):
    """The ids and the links of an HTML page."""

    def __init__(self):
        super().__init__()
        self.ids = set()
        self.links = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if "id" in attrs:
            self.ids.add(attrs["id"])
        for name in ("href", "src"):
            if attrs.get(name):
                self.links.append(attrs[name])


def check_links(out):
    """Fails on links within the site that point to a missing file or
    anchor, and on links to Markdown files."""
    pages = {}
    for page in out.rglob("*.html"):
        parser = Anchors()
        parser.feed(page.read_text(encoding="utf-8"))
        pages[page] = parser
    broken = []
    for page, parser in pages.items():
        for link in parser.links:
            if re.match(r"[A-Za-z][A-Za-z0-9+.-]*:", link) or link.startswith("//"):
                continue
            path, _, fragment = link.partition("#")
            path = path.partition("?")[0]
            if link.startswith("/"):
                continue  # site-url links of the 404 page
            target = (page.parent / path).resolve() if path else page
            if target.is_dir():
                target = target / "index.html"
            if path.endswith(".md") or not target.exists():
                broken.append(f"{page.relative_to(out)}: {link}")
            elif fragment and target.suffix == ".html":
                if fragment not in pages[target].ids:
                    broken.append(f"{page.relative_to(out)}: {link}")
    if broken:
        fail("broken links:\n  " + "\n  ".join(sorted(set(broken))))


def main():
    parser = argparse.ArgumentParser(description="Builds the website.")
    parser.add_argument(
        "--tag", help="the documented git revision, default the latest v* tag"
    )
    parser.add_argument("--out", default="target/site", help="output directory")
    parser.add_argument("--mdbook", default="mdbook", help="the mdbook command")
    parser.add_argument(
        "--mdbook-version",
        action="store_true",
        help="print the needed version of mdBook and exit",
    )
    args = parser.parse_args()
    if args.mdbook_version:
        print(MDBOOK_VERSION)
        return

    check_mdbook(args.mdbook)

    revision = Revision(args.tag)
    out = below_target(ROOT / args.out)
    work = below_target(TARGET / "site-build")
    for path in (out, work):
        if path.exists():
            shutil.rmtree(path)
    out.mkdir(parents=True)

    build_landing(revision, out)
    build_book(revision, work / "book", out, args.mdbook)
    build_sitemap(revision, out)
    check_links(out)
    print(f"built {out.relative_to(ROOT)} for {revision.label}")


if __name__ == "__main__":
    main()
