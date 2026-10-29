#!/usr/bin/env python3
"""Write apps/<app>/README.md for every app and the app table in README.md.

Reads state/versions.json (what is published) and build/discovered.json when present (what
exists upstream). A new app gets its folder and page here automatically.
"""

from __future__ import annotations

import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
REPO = os.environ.get("GITHUB_REPOSITORY", "Labtoso/Artcraft")
SITE = os.environ.get("ARTCRAFT_SITE", "https://artcraft.labtoso.de")
UPSTREAM = "storytold"

# What each app is for. A new app falls back to its upstream description.
ABOUT = {
    "artcraft-launcher": ("Installs, updates and starts every Craft app", "One window for all apps: shows what is installed, downloads the right build for your computer with one click and keeps everything up to date. This build installs the apps from this repository, so without community links."),
    "photocraft": ("Image editing", "The image editor you already know: layers, masks, selections, adjustment layers, layer styles and color management, on a GPU canvas with real layered files."),
    "vectorcraft": ("Vector illustration", "Vector graphics with the pen tool, boolean operations, live effects, image tracing, perspective grids and open formats such as SVG and PDF."),
    "filmcraft": ("Video editing", "Professional video editing with a timeline, frame-accurate time, its own codecs, color correction, keyframed effects and broadcast-grade audio."),
    "lightcraft": ("Photo library and raw developer", "Organize photos and develop raw files, non-destructively, with masks, color wheels, albums and ratings, all locally on your computer."),
    "pdfcraft": ("PDF workbench", "View and search PDFs, sort pages like cards, merge, split and save them safely."),
    "effectcraft": ("Motion graphics and VFX", "Animation and visual effects with layers, keyframes, a graph editor, more than 250 effects, real 3D and expressions."),
    "designcraft": ("Page layout and publishing", "Layout for books, magazines and flyers: text flow, styles, swatches, fine typography and print-ready PDFs."),
    "figcraft": ("Interface design", "Design interfaces with auto layout, components and variants, vector networks, prototypes and live sessions with others."),
    "wordcraft": ("Word processing", "Word processing with a ribbon, real document formats (DOCX), fast layout, track changes, tables of contents and citations."),
    "soundcraft": ("Digital audio workstation", "Record and mix with a mixing console, automation, its own plugins and CLAP support, MIDI, notation and surround."),
    "deckcraft": ("Presentations", "Slides with masters and themes, shapes, transitions and animations, a presenter view and PowerPoint compatibility (PPTX)."),
    "cadcraft": ("CAD and technical drawing", "Technical drawing with paper space and viewports, dimensions, hatches, block libraries and parametric constraints."),
    "gridcraft": ("Spreadsheets", "Spreadsheets with more than 500 functions, charts, pivot tables, conditional formatting and native XLSX compatibility."),
}


def load(path, default):
    if os.path.exists(path):
        with open(path, encoding="utf-8") as fh:
            return json.load(fh)
    return default


def main() -> int:
    state = load(os.path.join(ROOT, "state", "versions.json"), {})
    discovered = {a["id"]: a for a in load(os.path.join(ROOT, "build", "discovered.json"), [])}
    ids = sorted(set(state) | set(discovered), key=lambda i: (i != "artcraft-launcher", i))
    rows = []
    for app_id in ids:
        s = state.get(app_id, {})
        d = discovered.get(app_id, {})
        name = s.get("name") or d.get("name") or app_id
        repo = s.get("repo") or d.get("repo") or app_id
        short, long = ABOUT.get(app_id, (s.get("tagline") or d.get("tagline") or "", s.get("tagline") or d.get("tagline") or ""))
        version = s.get("version")
        folder = os.path.join(ROOT, "apps", app_id)
        os.makedirs(folder, exist_ok=True)
        files = ""
        if s.get("assets"):
            files = "\n".join(f"- [{a['name']}]({a['download_url']})" for a in s["assets"])
        page = f"{SITE}/apps/{app_id}/" if app_id != "artcraft-launcher" else f"{SITE}/apps/"
        with open(os.path.join(folder, "README.md"), "w", encoding="utf-8") as fh:
            fh.write(
                f"# {name}\n\n"
                f"**{short}.** {long}\n\n"
                f"| | |\n|---|---|\n"
                f"| Current version here | {('[' + version + '](https://github.com/' + REPO + '/releases/tag/' + s['tag'] + ')') if version else 'being built'} |\n"
                f"| Original | [github.com/{UPSTREAM}/{repo}](https://github.com/{UPSTREAM}/{repo}) |\n"
                f"| Website | [{page}]({page}) |\n"
                f"| License | MIT or Apache-2.0 (like the original; the license files are in every download) |\n\n"
                f"## What is different here\n\n"
                f"The same code as the original {d.get('tag') or s.get('upstream_tag') or ''}, only without the Discord and social media buttons. "
                f"Links to GitHub, error reports and updates point to this repository, website links to this fork's website. "
                f"Everything is built automatically from every new original release ([how it works](../../README.md#how-it-works)).\n\n"
                + (f"## Downloads ({version})\n\n{files}\n\n" if files else "")
                + ("The easiest way to install it is the [ArtCraft Launcher](../artcraft-launcher/README.md).\n" if app_id != "artcraft-launcher" else "")
            )
            if os.path.exists(os.path.join(folder, "debrand.toml")):
                fh.write("\nExtra rules for this app: [debrand.toml](debrand.toml).\n")
        status = f"[{version}](https://github.com/{REPO}/releases/tag/{s['tag']})" if version else "coming"
        rows.append(f"| [{name}](apps/{app_id}/README.md) | {short} | {status} | [Original](https://github.com/{UPSTREAM}/{repo}) |")

    readme = os.path.join(ROOT, "README.md")
    with open(readme, encoding="utf-8") as fh:
        text = fh.read()
    table = "| App | What for | Version | Source |\n|---|---|---|---|\n" + "\n".join(rows)
    text = re.sub(r"(<!-- apps:start -->\n).*?(\n<!-- apps:end -->)", lambda m: m.group(1) + table + m.group(2), text, flags=re.S)
    with open(readme, "w", encoding="utf-8") as fh:
        fh.write(text)
    print(f"{len(ids)} app pages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
