#!/usr/bin/env python3
"""Find every Craft app upstream and decide what needs a (re)build.

Sources of truth, both read live on every run:
  * the app list in the original launcher (crates/core/src/catalog.rs at its latest release),
  * every repository of the upstream account whose name ends in "craft" and that has a release.
A new app shows up through either one; a new version through the app's latest release.

Writes a GitHub Actions output `matrix` (JSON list of builds) and `any` (true/false), and
`build/discovered.json` with what was found, for the later jobs.

Environment: GITHUB_TOKEN, UPSTREAM (default storytold), FORCE (all | comma list | empty).
"""

from __future__ import annotations

import json
import os
import re
import sys
import urllib.request

UPSTREAM = os.environ.get("UPSTREAM", "storytold")
LAUNCHER_REPO = "craft-launcher"
NOT_APPS = {"artcraft", "crafthub"}
STATE = os.path.join(os.path.dirname(__file__), "..", "state", "versions.json")


def api(path: str):
    req = urllib.request.Request("https://api.github.com" + path, headers={"Accept": "application/vnd.github+json"})
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.load(r)
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        raise


def raw(repo: str, ref: str, path: str) -> str | None:
    url = f"https://raw.githubusercontent.com/{UPSTREAM}/{repo}/{ref}/{path}"
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            return r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError:
        return None


def catalog_apps(catalog: str) -> list[dict]:
    """id, name, tagline, repo of every `app(...)` call in the launcher's SUITE."""
    suite = catalog.split("pub const SUITE", 1)[-1].split("pub const WEBSITE_URL", 1)[0]
    apps = []
    for m in re.finditer(r'app\(\s*"([\w-]+)",\s*"([^"]+)",\s*"([^"]*)",.*?"' + UPSTREAM + r'/([\w.-]+)"', suite, re.S):
        apps.append({"id": m.group(1), "name": m.group(2), "tagline": m.group(3), "repo": m.group(4)})
    return apps


def org_craft_repos() -> list[dict]:
    out, page = [], 1
    while True:
        repos = api(f"/orgs/{UPSTREAM}/repos?per_page=100&page={page}") or api(f"/users/{UPSTREAM}/repos?per_page=100&page={page}") or []
        if not repos:
            break
        for r in repos:
            name = r["name"]
            if name.lower().endswith("craft") and name.lower() not in NOT_APPS and not r.get("archived"):
                out.append({"id": name.lower(), "name": name, "tagline": r.get("description") or "", "repo": name})
        if len(repos) < 100:
            break
        page += 1
    return out


def pretty_name(app_id: str) -> str:
    base = app_id[:-5] if app_id.endswith("craft") else app_id
    special = {"cad": "CAD", "pdf": "Pdf"}
    return special.get(base, base.capitalize()) + "Craft"


def main() -> int:
    os.makedirs("build", exist_ok=True)
    state = {}
    if os.path.exists(STATE):
        with open(STATE, encoding="utf-8") as fh:
            state = json.load(fh)
    force = os.environ.get("FORCE", "").strip()
    forced = set(force.split(",")) if force and force != "all" else set()

    launcher = api(f"/repos/{UPSTREAM}/{LAUNCHER_REPO}/releases/latest")
    if not launcher:
        sys.exit("upstream launcher has no release")
    catalog = raw(LAUNCHER_REPO, launcher["tag_name"], "crates/core/src/catalog.rs") or ""
    apps = {a["id"]: a for a in catalog_apps(catalog)}
    for a in org_craft_repos():
        apps.setdefault(a["id"], a)
    apps["artcraft-launcher"] = {"id": "artcraft-launcher", "name": "ArtCraft Launcher", "tagline": "Installs and updates the Craft App Suite", "repo": LAUNCHER_REPO}

    found, matrix = [], []
    for app_id, a in sorted(apps.items()):
        rel = launcher if app_id == "artcraft-launcher" else api(f"/repos/{UPSTREAM}/{a['repo']}/releases/latest")
        if not rel:
            continue
        tag = rel["tag_name"]
        version = tag.lstrip("vV")
        wf = raw(a["repo"], tag, ".github/workflows/release.yml") or ""
        fonts = re.search(r"craft-fonts-ref:\s*([0-9a-f]{40})", wf)
        has_pkg = raw(a["repo"], tag, "packaging/windows/package.ps1") is not None
        if not has_pkg:
            continue
        entry = {
            "id": app_id,
            "name": a["name"] if a["name"] != a["id"] else pretty_name(app_id),
            "tagline": a["tagline"],
            "repo": a["repo"],
            "tag": tag,
            "version": version,
            "fonts_ref": fonts.group(1) if fonts else "",
            "upstream_notes": (rel.get("body") or "")[:20000],
        }
        found.append(entry)
        built = state.get(app_id, {})
        if force == "all" or app_id in forced or built.get("upstream_tag") != tag or built.get("debrand_rev") != debrand_rev():
            matrix.append({k: entry[k] for k in ("id", "name", "repo", "tag", "version", "fonts_ref")})

    with open("build/discovered.json", "w", encoding="utf-8") as fh:
        json.dump(found, fh, indent=2)
    out = os.environ.get("GITHUB_OUTPUT")
    lines = [f"matrix={json.dumps(matrix)}", f"any={'true' if matrix else 'false'}"]
    if out:
        with open(out, "a", encoding="utf-8") as fh:
            fh.write("\n".join(lines) + "\n")
    print(f"{len(found)} apps upstream, {len(matrix)} to build: {', '.join(m['id'] + ' ' + m['version'] for m in matrix) or 'none'}")
    return 0


def debrand_rev() -> str:
    """Bumped by hand in tools/DEBRAND_REV when the rules change enough to rebuild everything."""
    path = os.path.join(os.path.dirname(__file__), "DEBRAND_REV")
    if os.path.exists(path):
        with open(path, encoding="utf-8") as fh:
            return fh.read().strip()
    return "1"


if __name__ == "__main__":
    sys.exit(main())
