#!/usr/bin/env python3
"""Point the launcher at this repo's builds and take out the community links.

The launcher's look and behaviour stay as they are, except:
  * every app and the launcher itself are installed from this repo's releases,
  * the release feed is the one this repo publishes next to the website,
  * that feed never goes stale (it is regenerated on every build),
  * the Community box in the settings (Discord, website, GitHub, social icons) is gone,
  * the status bar keeps only the Website and GitHub icons, which open this copy's website and
    this repo; Discord and the social accounts are gone.

Fails loudly when a pattern is gone upstream, so a changed launcher never ships half-patched.

Usage: patch_launcher.py SRC_DIR
"""

from __future__ import annotations

import os
import re
import sys

OWNER, _, NAME = os.environ.get("ARTCRAFT_REPO", "Labtoso/Artcraft").partition("/")
SITE = os.environ.get("ARTCRAFT_SITE", "https://labtoso.github.io/Artcraft")
UPSTREAM = os.environ.get("UPSTREAM", "storytold")


def sub(text: str, pattern: str, repl: str, path: str, count_min: int = 1) -> str:
    new, n = re.subn(pattern, repl, text)
    if n < count_min:
        sys.exit(f"patch_launcher: '{pattern}' not found in {path}")
    return new


def main() -> int:
    src = sys.argv[1]
    catalog = os.path.join(src, "crates/core/src/catalog.rs")
    feed = os.path.join(src, "crates/core/src/feed.rs")
    settings = os.path.join(src, "crates/ui-egui/src/settings_dialog.rs")
    views = os.path.join(src, "crates/ui-egui/src/views.rs")

    with open(catalog, encoding="utf-8") as fh:
        text = fh.read()
    # every "storytold/<repo>" in the app list and the launcher's own repo
    text = sub(text, r'"' + UPSTREAM + r'/[\w.-]+"', f'"{OWNER}/{NAME}"', catalog, 5)
    text = sub(text, r'(pub const GITHUB_OWNER: &str = )"[^"]*"', rf'\1"{OWNER}"', catalog)
    text = sub(text, r'(pub const WEBSITE_URL: &str = )"[^"]*"', rf'\1"{SITE}"', catalog)
    text = sub(text, r'(pub const GITHUB_URL: &str = )"[^"]*"', rf'\1"https://github.com/{OWNER}/{NAME}"', catalog)
    with open(catalog, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)

    with open(feed, encoding="utf-8") as fh:
        text = fh.read()
    text = sub(text, r'(pub const FEED_URL: &str = )"[^"]*"', rf'\1"{SITE}/release_info.json"', feed)
    text = sub(text, r"(pub const MAX_AGE: i64 = )[^;]+;", r"\g<1>10 * 365 * 24 * 3600;", feed)
    with open(feed, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)
    with open(settings, encoding="utf-8") as fh:
        text = fh.read()
    text = sub(text, r"\n[ \t]*this\.settings_community\(ui\);", "", settings)
    text = sub(text, r"\n([ \t]*)fn settings_community\(", r"\n\1#[allow(dead_code)]\n\1fn settings_community(", settings)
    with open(settings, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)

    with open(views, encoding="utf-8") as fh:
        text = fh.read()
    text = sub(text, r"\n([ \t]*)let icon = 13\.0;",
               r'\n\1links.retain(|l| l.name == "Website" || l.name == "GitHub");\n\1let icon = 13.0;', views)
    with open(views, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)

    print("launcher now installs from", f"{OWNER}/{NAME}", "with the feed at", f"{SITE}/release_info.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
