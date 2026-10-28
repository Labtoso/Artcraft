#!/usr/bin/env python3
"""Publish freshly built apps: one release per app version plus the big suite release.

Reads build/discovered.json and dist/<app>/* (the files the build jobs produced). For every app
with files:
  * release `<app>-v<version>` holds that version's files and SHA256SUMS,
  * release `suite` (always "Latest") swaps that app's old files for the new ones,
  * state/versions.json records what is published (the launcher feed and the website read it).
Apps without files (their build failed) keep their previous version everywhere.

Writes build/failed.txt with the apps that should have been built but weren't.
Needs GH_TOKEN and GITHUB_REPOSITORY; uses the gh CLI.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
STATE = os.path.join(ROOT, "state", "versions.json")
REPO = os.environ.get("GITHUB_REPOSITORY", "Labtoso/Artcraft")
SUITE_TAG = "suite"


def gh(*args, check=True, capture=False):
    print("+ gh", " ".join(args), flush=True)
    return subprocess.run(["gh", *args], check=check, text=True, capture_output=capture)


def sha256(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def release_exists(tag: str) -> bool:
    return gh("release", "view", tag, "--repo", REPO, "--json", "tagName", check=False, capture=True).returncode == 0


def clean_notes(text: str) -> str:
    """The original notes without community lines."""
    lines = [l for l in text.strip().splitlines() if not re.search(r"discord|bsky|bluesky|linkedin|instagram|tiktok|x\.com/", l, re.I)]
    return "\n".join(lines)


def app_notes(app: dict, upstream_url: str) -> str:
    return (
        f"## Added\n"
        f"- {app['name']} {app['version']}, built from the original release "
        f"[{app['tag']}]({upstream_url}) without community links (Discord, social networks).\n\n"
        f"## Fixed\n"
        f"- Everything the original {app['tag']} fixed, see its release notes below.\n\n"
        f"## Install\n"
        f"- Easiest: install the **ArtCraft Launcher** from the [suite release](https://github.com/{REPO}/releases/tag/{SUITE_TAG}); it installs and updates every app.\n"
        f"- Or download the file for your system below: `windows-x64.msi` (installer) or `windows-x64.zip` (portable) for Windows, "
        f"`.AppImage`, `.deb`, `.rpm` or `.tar.gz` for Linux. Checksums are in `SHA256SUMS`.\n\n"
        f"---\n\n### Original release notes\n\n{clean_notes(app.get('upstream_notes', ''))}\n"
    )


def suite_notes(state: dict) -> str:
    rows = []
    for app_id, s in sorted(state.items(), key=lambda kv: (kv[0] != "artcraft-launcher", kv[0])):
        rows.append(f"| {s['name']} | {s['version']} | [{s['upstream_tag']}](https://github.com/storytold/{s['repo']}/releases/tag/{s['upstream_tag']}) | [{app_id}-v{s['version']}](https://github.com/{REPO}/releases/tag/{app_id}-v{s['version']}) |")
    return (
        "## Added\n"
        "- The current version of every Craft app in one place, without community links.\n\n"
        "## Fixed\n"
        "- Updated automatically whenever an app gets a new version or a new app comes out.\n\n"
        "## Install\n"
        "- **Recommended:** download `artcraft-launcher-…-windows-x64.msi` (Windows) or `artcraft-launcher-…-linux-x86_64.AppImage` (Linux), "
        "start it and install any app with one click. It keeps everything up to date by itself.\n"
        "- Or pick a single app's file below. Checksums are in `SHA256SUMS`.\n\n"
        "| App | Version | Original | Single release |\n|---|---|---|---|\n" + "\n".join(rows) + "\n"
    )


def main() -> int:
    with open(os.path.join(ROOT, "build", "discovered.json"), encoding="utf-8") as fh:
        discovered = {a["id"]: a for a in json.load(fh)}
    wanted = json.loads(os.environ.get("MATRIX", "[]"))
    state = {}
    if os.path.exists(STATE):
        with open(STATE, encoding="utf-8") as fh:
            state = json.load(fh)
    rev = open(os.path.join(HERE, "DEBRAND_REV"), encoding="utf-8").read().strip()
    now = dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")

    if not release_exists(SUITE_TAG):
        gh("release", "create", SUITE_TAG, "--repo", REPO, "--title", "Craft App Suite", "--notes", "Wird gerade befüllt.", "--latest")

    failed = []
    for item in wanted:
        app_id = item["id"]
        app = discovered[app_id]
        folder = os.path.join(ROOT, "dist", app_id)
        files = sorted(os.path.join(folder, f) for f in os.listdir(folder)) if os.path.isdir(folder) else []
        files = [f for f in files if os.path.isfile(f) and not f.endswith("SHA256SUMS")]
        if not files:
            failed.append(f"{app_id} {app['version']}")
            continue
        sums = os.path.join(folder, "SHA256SUMS")
        assets = []
        with open(sums, "w", encoding="utf-8", newline="\n") as fh:
            for f in files:
                digest = sha256(f)
                fh.write(f"{digest}  {os.path.basename(f)}\n")
                assets.append({"name": os.path.basename(f), "size_bytes": os.path.getsize(f), "sha256": digest})
        tag = f"{app_id}-v{app['version']}"
        upstream_url = f"https://github.com/storytold/{app['repo']}/releases/tag/{app['tag']}"
        notes = os.path.join(ROOT, "build", f"{app_id}.notes.md")
        with open(notes, "w", encoding="utf-8") as fh:
            fh.write(app_notes(app, upstream_url))
        title = f"{app['name']} {app['version']}"
        if release_exists(tag):
            gh("release", "edit", tag, "--repo", REPO, "--title", title, "--notes-file", notes, "--latest=false")
        else:
            gh("release", "create", tag, "--repo", REPO, "--title", title, "--notes-file", notes, "--latest=false")
        gh("release", "upload", tag, "--repo", REPO, "--clobber", *files, sums)
        for a in assets:
            a["download_url"] = f"https://github.com/{REPO}/releases/download/{tag}/{a['name']}"

        # the suite release: this app's old files out, the new ones in
        listing = gh("release", "view", SUITE_TAG, "--repo", REPO, "--json", "assets", "--jq", ".assets[].name", capture=True).stdout.split()
        for name in listing:
            if name.startswith(app_id + "-") and name not in {a["name"] for a in assets}:
                gh("release", "delete-asset", SUITE_TAG, name, "--repo", REPO, "--yes")
        gh("release", "upload", SUITE_TAG, "--repo", REPO, "--clobber", *files)

        state[app_id] = {
            "name": app["name"],
            "tagline": app.get("tagline", ""),
            "repo": app["repo"],
            "upstream_tag": app["tag"],
            "version": app["version"],
            "tag": tag,
            "debrand_rev": rev,
            "published_at": now,
            "notes": app_notes(app, upstream_url),
            "assets": assets,
        }
        with open(STATE, "w", encoding="utf-8") as fh:
            json.dump(state, fh, indent=2, ensure_ascii=False)
            fh.write("\n")

    # one SHA256SUMS for the whole suite
    lines = []
    for app_id, s in sorted(state.items()):
        lines += [f"{a['sha256']}  {a['name']}" for a in s["assets"]]
    suite_sums = os.path.join(ROOT, "build", "SHA256SUMS")
    with open(suite_sums, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(lines) + "\n")
    gh("release", "upload", SUITE_TAG, "--repo", REPO, "--clobber", suite_sums)
    notes = os.path.join(ROOT, "build", "suite.notes.md")
    with open(notes, "w", encoding="utf-8") as fh:
        fh.write(suite_notes(state))
    gh("release", "edit", SUITE_TAG, "--repo", REPO, "--title", f"Craft App Suite ({now[:10]})", "--notes-file", notes, "--latest")

    with open(os.path.join(ROOT, "build", "failed.txt"), "w", encoding="utf-8") as fh:
        fh.write("\n".join(failed))
    print(f"published {len(wanted) - len(failed)}, failed {len(failed)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
