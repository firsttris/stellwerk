# Modelled on firsttris/haben scripts/doku-aenderungsdatum (via Speicherstadt/fixpunkt)
"""The date of the last change under every page of the docs, from the last commit of its file.

Under MkDocs Material the plugin git-revision-date-localized wrote the date under every page.
Zensical 0.1 can't do that yet, it is "Planned" in its compatibility list
(https://zensical.org/docs/compatibility/mkdocs/plugins/). This Markdown extension takes the
page's path from the context Zensical gives every page (zensical.extensions.context) and appends
the date. Once Zensical does it itself, the extension goes.
"""

from __future__ import annotations

import subprocess
from collections.abc import Callable
from functools import cache
from pathlib import Path

from markdown import Extension, Markdown
from markdown.preprocessors import Preprocessor

MONTHS = (
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
)

DateLookup = Callable[[Path], "str | None"]


def format_date(iso: str) -> str | None:
    """YYYY-MM-DD as a German date ("7. November 2026"), None for any other format."""
    parts = iso.split("-")
    if len(parts) != 3 or not all(part.isdigit() for part in parts):
        return None
    year, month, day = (int(part) for part in parts)
    if not 1 <= month <= 12 or not 1 <= day <= 31:
        return None
    return f"{day}. {MONTHS[month - 1]} {year}"


def with_date(lines: list[str], iso: str | None) -> list[str]:
    """The page's lines with the date at the end; unchanged without a valid date."""
    text = format_date(iso) if iso else None
    if text is None:
        return lines
    # data-search-exclude: keeps the date out of the search index, else "Update" finds every page
    return [
        *lines,
        "",
        f'<p class="revision-date" data-search-exclude><small>Letztes Update: <time datetime="{iso}">{text}</time></small></p>',
    ]


def last_commit_date(path: Path, cwd: Path | None = None) -> str | None:
    """Date of the last commit that changed the file (YYYY-MM-DD), None without git or history."""
    try:
        result = subprocess.run(
            ["git", "log", "-1", "--format=%cs", "--", str(path)],
            capture_output=True,
            text=True,
            check=True,
            cwd=cwd,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return result.stdout.strip() or None


def page_path(md: Markdown) -> str | None:
    """The page's path relative to docs_dir from Zensical's context; None outside Zensical."""
    try:
        from zensical.extensions.context import ContextPreprocessor
    except ImportError:
        return None
    context = ContextPreprocessor.from_markdown(md)
    return context.page.path if context else None


class _DatePreprocessor(Preprocessor):
    def __init__(self, md: Markdown, docs_dir: str, lookup: DateLookup):
        super().__init__(md)
        self.docs_dir = docs_dir
        self.lookup = lookup

    def run(self, lines: list[str]) -> list[str]:
        path = page_path(self.md)
        if path is None:
            return lines
        return with_date(lines, self.lookup(Path(self.docs_dir) / path))


class RevisionDateExtension(Extension):
    def __init__(self, **kwargs):
        self.config = {"docs_dir": ["docs", "Directory of the docs relative to the working directory"]}
        super().__init__(**kwargs)

    def extendMarkdown(self, md: Markdown) -> None:
        # Before html_block (20), so the appended line counts as an HTML block, and after the
        # normalization (30)
        lookup = cache(last_commit_date)
        md.preprocessors.register(_DatePreprocessor(md, self.getConfig("docs_dir"), lookup), "revision_date", 25)


def makeExtension(**kwargs) -> RevisionDateExtension:
    return RevisionDateExtension(**kwargs)
