#!/usr/bin/env python3
"""Point the launcher at this repo's builds and take out the community links.

The launcher's look and behaviour stay as they are, except:
  * every app and the launcher itself are installed from this repo's releases,
  * the release feed is the one this repo publishes next to the website,
  * that feed never goes stale (it is regenerated on every build),
  * the version is this copy's own (original patch * 1000 + tools/LAUNCHER_REV), so a changed
    launcher reaches installed ones as an update,
  * the feed is the only source of releases, also with pre-releases switched on: every app lives
    in the same repository here, so GitHub's "latest release" can't tell the apps apart,
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
    backend = os.path.join(src, "crates/native/src/backend.rs")
    cargo_toml = os.path.join(src, "Cargo.toml")
    cargo_lock = os.path.join(src, "Cargo.lock")

    # this copy's own version (see launcher_version in discover.py), in Cargo.toml and Cargo.lock
    with open(cargo_toml, encoding="utf-8") as fh:
        text = fh.read()
    m = re.search(r'(\[workspace\.package\][^\[]*?\nversion = )"(\d+)\.(\d+)\.(\d+)"', text)
    if not m:
        sys.exit("patch_launcher: [workspace.package] version not found in Cargo.toml")
    upstream = f"{m[2]}.{m[3]}.{m[4]}"
    with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "LAUNCHER_REV"), encoding="utf-8") as fh:
        rev = int(fh.read().strip() or 0)
    ours = f"{m[2]}.{m[3]}.{int(m[4]) * 1000 + rev}"
    text = text[:m.start()] + m[1] + f'"{ours}"' + text[m.end():]
    with open(cargo_toml, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)
    members = set()
    for root, _, names in os.walk(src):
        if "Cargo.toml" in names and "target" not in root and ".git" not in root:
            with open(os.path.join(root, "Cargo.toml"), encoding="utf-8") as fh:
                t = fh.read()
            if "version.workspace = true" in t:
                n = re.search(r'^name = "([^"]+)"', t, re.M)
                if n:
                    members.add(n[1])
    with open(cargo_lock, encoding="utf-8") as fh:
        text = fh.read()
    for name in members:
        text = text.replace(f'name = "{name}"\nversion = "{upstream}"', f'name = "{name}"\nversion = "{ours}"')
    with open(cargo_lock, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)

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

    with open(backend, encoding="utf-8") as fh:
        text = fh.read()
    # the saved feed answers at start, whatever the pre-release setting
    text = sub(text, r"let feed = if pre \{ None \} else \{ load_feed\(&self\.paths\) \};",
               "let feed = if pre && !FEED_ONLY { None } else { load_feed(&self.paths) };", backend)
    # every check asks the feed, whatever the pre-release setting
    text = sub(text, r"\} else if pre \{\n(\s*)// The feed serves stable releases only",
               r"} else if pre && !FEED_ONLY {\n\1// The feed serves stable releases only", backend)
    # and only the feed answers: GitHub would mix up the apps of the one shared repository
    text = sub(text, r"(\) -> \(Option<LatestRelease>, Origin, Option<Source>\) \{\n)(\s*)let repo = feed_repo\(feed, app\);",
               r"\1\2if FEED_ONLY {\n\2    return feed_only_latest(app, feed, now);\n\2}\n\2let repo = feed_repo(feed, app);", backend)
    text += """

/// This copy publishes every app in one repository, so the release feed is the only source of
/// releases (GitHub's "latest release" belongs to whichever app was published last).
const FEED_ONLY: bool = true;

/// `app`'s latest release according to the feed alone.
fn feed_only_latest(app: &CraftApp, feed: Option<&Feed>, now: i64) -> (Option<LatestRelease>, Origin, Option<Source>) {
    let latest = match feed.map(|f| f.answer(app, now)) {
        Some(FeedAnswer::Fresh(Some(r))) => LatestRelease::Found(r),
        Some(FeedAnswer::Fresh(None) | FeedAnswer::AskGitHub) => LatestRelease::None,
        None => LatestRelease::Failed("the release list couldn't be loaded; check your internet connection".into()),
    };
    (Some(latest), Origin::ArtCraftApi, Some(Source::Feed))
}
"""
    with open(backend, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)

    print("launcher", ours, "now installs from", f"{OWNER}/{NAME}", "with the feed at", f"{SITE}/release_info.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
