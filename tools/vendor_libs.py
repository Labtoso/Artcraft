#!/usr/bin/env python3
"""Give an app (or the launcher) this copy's update library.

Every Craft App and the launcher update themselves with `craft-update` from storytold/craft-libs.
Left as it is, that library
  * asks GitHub for new versions of storytold/<app>, so an app here would offer the original's
    build and replace this copy with it,
  * trusts downloads only from github.com/storytold/<app>/releases,
  * on macOS installs only apps signed by the original team's Apple Developer ID.

This script copies the library's crate at the revision the app pins into `vendor-craft-update/`,
points it at this copy (the release feed on this copy's website, this repo's releases, no Apple
team pin) and wires it in with a `[patch]` section. Cargo.lock is updated to match, so builds with
`--locked` work. Nothing is done when the app doesn't use craft-update.

Usage: vendor_libs.py SRC_DIR
"""

from __future__ import annotations

import glob
import os
import re
import shutil
import subprocess
import sys
import tomllib

OWNER, _, NAME = os.environ.get("ARTCRAFT_REPO", "Labtoso/Artcraft").partition("/")
SITE = os.environ.get("ARTCRAFT_SITE", "https://artcraft.labtoso.de")
UPSTREAM = os.environ.get("UPSTREAM", "storytold")
LIBS = f"https://github.com/{UPSTREAM}/craft-libs"
DEST = "vendor-craft-update"
CACHE = os.environ.get("CRAFT_LIBS_CACHE", os.path.join(os.environ.get("RUNNER_TEMP", "/tmp"), "craft-libs"))


def run(cmd, cwd=None):
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=cwd, check=True)


def sub(text: str, pattern: str, repl: str, what: str, flags=0) -> str:
    new, n = re.subn(pattern, repl, text, flags=flags)
    if not n:
        sys.exit(f"vendor_libs: '{pattern}' not found in {what}")
    return new


def pinned_rev(src: str) -> str | None:
    for path in glob.glob(os.path.join(src, "**", "Cargo.toml"), recursive=True):
        if os.sep + "target" + os.sep in path or DEST in path:
            continue
        with open(path, encoding="utf-8") as fh:
            m = re.search(r'^craft-update\s*=\s*\{[^}\n]*rev\s*=\s*"([0-9a-f]{7,40})"', fh.read(), re.M)
        if m:
            return m[1]
    return None


def checkout(rev: str) -> str:
    if not os.path.isdir(os.path.join(CACHE, ".git")):
        run(["git", "clone", "-q", "--filter=blob:none", "--no-checkout", LIBS, CACHE])
    try:
        run(["git", "checkout", "-q", "--force", rev], CACHE)
    except subprocess.CalledProcessError:
        run(["git", "fetch", "-q", "origin"], CACHE)
        run(["git", "checkout", "-q", "--force", rev], CACHE)
    return CACHE


def inline(value) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str):
        return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'
    if isinstance(value, list):
        return "[" + ", ".join(inline(v) for v in value) + "]"
    if isinstance(value, dict):
        return "{ " + ", ".join(f"{k} = {inline(v)}" for k, v in value.items()) + " }"
    return str(value)


def standalone_manifest(text: str, libs_root: str) -> str:
    """The crate's Cargo.toml without anything inherited from the craft-libs workspace."""
    with open(os.path.join(libs_root, "Cargo.toml"), "rb") as fh:
        ws = tomllib.load(fh)["workspace"]
    package, deps = ws.get("package", {}), ws.get("dependencies", {})

    def field(m):
        return f"{m[1]} = {inline(package[m[1]])}"

    text = re.sub(r"^([\w-]+)\.workspace\s*=\s*true", field, text, flags=re.M)
    text = re.sub(r"^\[lints\]\s*\nworkspace\s*=\s*true\s*\n", "", text, flags=re.M)

    def dep(m):
        name, extra = m[1], tomllib.loads(f"x = {{ {m[2]} }}")["x"]
        extra.pop("workspace", None)
        spec = deps[name]
        spec = {"version": spec} if isinstance(spec, str) else dict(spec)
        if "path" in spec:
            sys.exit(f"vendor_libs: craft-update now needs the craft-libs crate {name}; vendor it too")
        feats = spec.get("features", []) + [f for f in extra.pop("features", []) if f not in spec.get("features", [])]
        spec.update(extra)
        if feats:
            spec["features"] = feats
        return f"{name} = {inline(spec)}"

    return re.sub(r"^([\w-]+)\s*=\s*\{\s*(workspace\s*=\s*true[^}\n]*)\}", dep, text, flags=re.M)


# The in-app update check: this copy's release feed instead of the original's GitHub releases.
FEED_CHECK = '''/// Checks for an update against this copy's release feed (every app is published in one
/// repository here, so its GitHub releases can't tell the apps apart).
pub fn check_for_update(app: &App, http: &Http, current_version: &str) -> Result<Option<Release>, String> {
    let text = http.get_feed(FEED_URL).map_err(|e| e.why)?;
    let feed: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("unreadable release feed ({e})"))?;
    let Some(entry) = feed["release_info"]["apps"].get(app.id) else { return Ok(None) };
    let r = &entry["latest_release"];
    let assets: Vec<serde_json::Value> = r["assets"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|a| {
                    serde_json::json!({
                        "name": a["name"],
                        "size": a["size_bytes"],
                        "browser_download_url": a["download_url"],
                        "digest": a["sha256"].as_str().map(|h| format!("sha256:{h}")),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let github = serde_json::json!({
        "tag_name": r["tag"],
        "name": r["name"],
        "published_at": r["published_at"],
        "html_url": r["html_url"],
        "body": r["notes_markdown"],
        "prerelease": r["prerelease"],
        "assets": assets,
    });
    let release = crate::release::parse_github_release(&github.to_string(), app.repo)?;
    if let Some(rel_v) = &release.version
        && let Some(cur_v) = crate::version::parse(current_version)
        && crate::version::is_newer(rel_v, &cur_v)
    {
        return Ok(Some(release));
    }
    Ok(None)
}

/// This copy's release feed, the same one the launcher reads.
pub const FEED_URL: &str = "{SITE}/release_info.json";
'''


def patch_sources(crate: str):
    path = os.path.join(crate, "src", "suite.rs")
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    text = sub(text, r'"' + UPSTREAM + r'/[\w.-]+"', f'"{OWNER}/{NAME}"', path)
    with open(path, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)

    path = os.path.join(crate, "src", "install.rs")
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    # builds here are signed ad hoc (no Apple Developer ID); the checksum from the feed and the
    # bundle's version still have to match
    text = sub(text, r"Self \{ team: Some\(TEAM_ID\), version: release\.version\.as_ref\(\) \}",
               "Self { team: None, version: release.version.as_ref() }", path)
    with open(path, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)

    path = os.path.join(crate, "src", "lib.rs")
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    text = sub(text, r"/// Checks for an update against GitHub releases for the given app\.\npub fn check_for_update\(.*?\n\}\n",
               lambda _: FEED_CHECK.replace("{SITE}", SITE), path, re.S)
    with open(path, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)


def wire(src: str):
    path = os.path.join(src, "Cargo.toml")
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    header = f'[patch."{LIBS}"]'
    line = f'craft-update = {{ path = "{DEST}" }}'
    if header in text:
        text = text.replace(header, header + "\n" + line, 1)
    else:
        text = text.rstrip("\n") + f"\n\n# this copy's update library (tools/vendor_libs.py)\n{header}\n{line}\n"
    with open(path, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)


def main() -> int:
    src = os.path.abspath(sys.argv[1])
    rev = pinned_rev(src)
    if not rev:
        print("vendor_libs: no craft-update dependency, nothing to do")
        return 0
    libs = checkout(rev)
    crate = os.path.join(src, DEST)
    shutil.rmtree(crate, ignore_errors=True)
    shutil.copytree(os.path.join(libs, "crates", "update"), crate)
    manifest = os.path.join(crate, "Cargo.toml")
    with open(manifest, encoding="utf-8") as fh:
        text = standalone_manifest(fh.read(), libs)
    with open(manifest, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)
    patch_sources(crate)
    wire(src)
    # the lock now takes craft-update from the folder; nothing else moves
    run(["cargo", "update", "-p", "craft-update", "--offline"], src) if os.environ.get("CARGO_OFFLINE") else run(["cargo", "update", "-p", "craft-update"], src)
    print(f"vendor_libs: craft-update {rev[:12]} now checks {SITE}/release_info.json and installs from {OWNER}/{NAME}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
