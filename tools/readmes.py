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

# What each app is for, in German. A new app falls back to its upstream description.
ABOUT = {
    "artcraft-launcher": ("Installiert, aktualisiert und startet alle Craft-Apps", "Ein Fenster für alle Apps: zeigt, was installiert ist, lädt mit einem Klick die passende Version für deinen Rechner und hält alles aktuell. Diese Version holt die Apps aus diesem Repo, also ohne Community-Links."),
    "photocraft": ("Bildbearbeitung", "Ein Bildeditor, wie man ihn kennt: Ebenen, Masken, Auswahlen, Einstellungsebenen, Ebenenstile und Farbverwaltung, mit GPU-Leinwand und echten Ebenen-Dateien."),
    "vectorcraft": ("Vektorillustration", "Vektorgrafik mit Zeichenstift, Boolean-Operationen, Live-Effekten, Bildnachzeichnung, Perspektivraster und offenen Formaten wie SVG und PDF."),
    "filmcraft": ("Videoschnitt", "Professioneller Videoschnitt mit Timeline, bildgenauer Zeit, eigenen Codecs, Farbkorrektur, Effekten mit Keyframes und Audio auf Sendeniveau."),
    "lightcraft": ("Fotobibliothek und RAW-Entwickler", "Fotos verwalten und RAW-Dateien entwickeln, nicht-destruktiv, mit Masken, Farbrädern, Alben und Bewertungen, alles lokal auf deinem Rechner."),
    "pdfcraft": ("PDF-Werkbank", "PDFs ansehen, durchsuchen, Seiten wie Karten sortieren, zusammenführen, aufteilen und sicher speichern."),
    "effectcraft": ("Motion Graphics und VFX", "Animation und visuelle Effekte mit Ebenen, Keyframes, Graph-Editor, über 250 Effekten, echtem 3D und Expressions."),
    "designcraft": ("Seitenlayout und Publishing", "Layout für Bücher, Magazine und Flyer: Textfluss, Formate, Farbfelder, schöne Typografie und druckfertige PDFs."),
    "figcraft": ("Interface-Design", "Oberflächen gestalten mit Auto-Layout, Komponenten und Varianten, Vektornetzen, Prototypen und Live-Sessions zu mehreren."),
    "wordcraft": ("Textverarbeitung", "Textverarbeitung mit Menüband, echten Dokumentformaten (DOCX), schnellem Layout, Änderungsverfolgung, Inhaltsverzeichnissen und Zitaten."),
    "soundcraft": ("Digitale Audio-Workstation", "Aufnehmen und Mischen mit Mischpult, Automation, eigenen Plugins und CLAP-Unterstützung, MIDI, Notation und Surround."),
    "deckcraft": ("Präsentationen", "Folien mit Mastern und Themes, Formen, Übergängen und Animationen, Referentenansicht und PowerPoint-Kompatibilität (PPTX)."),
    "cadcraft": ("CAD und technisches Zeichnen", "Technisches Zeichnen mit Papierbereich und Ansichtsfenstern, Bemaßungen, Schraffuren, Blockbibliotheken und parametrischen Abhängigkeiten."),
    "gridcraft": ("Tabellenkalkulation", "Tabellen mit über 500 Funktionen, Diagrammen, Pivot-Tabellen, bedingter Formatierung und nativer XLSX-Kompatibilität."),
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
                f"| Aktuelle Version hier | {('[' + version + '](https://github.com/' + REPO + '/releases/tag/' + s['tag'] + ')') if version else 'wird gerade gebaut'} |\n"
                f"| Original | [github.com/{UPSTREAM}/{repo}](https://github.com/{UPSTREAM}/{repo}) |\n"
                f"| Website | [{page}]({page}) |\n"
                f"| Lizenz | MIT oder Apache-2.0 (wie das Original, Lizenzdateien liegen in jedem Download) |\n\n"
                f"## Was hier anders ist\n\n"
                f"Gleicher Code wie das Original {d.get('tag') or s.get('upstream_tag') or ''}, nur ohne Discord- und Social-Media-Buttons. "
                f"Links zu GitHub, Fehlermeldungen und Updates zeigen auf dieses Repo, die Website auf die gespiegelte Seite. "
                f"Alles wird automatisch aus jedem neuen Original-Release gebaut ([so funktioniert es](../../README.md#so-funktioniert-es)).\n\n"
                + (f"## Downloads ({version})\n\n{files}\n\n" if files else "")
                + ("Am einfachsten installierst du sie über den [ArtCraft Launcher](../artcraft-launcher/README.md).\n" if app_id != "artcraft-launcher" else "")
            )
            if os.path.exists(os.path.join(folder, "debrand.toml")):
                fh.write("\nZusätzliche Regeln für diese App: [debrand.toml](debrand.toml).\n")
        status = f"[{version}](https://github.com/{REPO}/releases/tag/{s['tag']})" if version else "kommt"
        rows.append(f"| [{name}](apps/{app_id}/README.md) | {short} | {status} | [Original](https://github.com/{UPSTREAM}/{repo}) |")

    readme = os.path.join(ROOT, "README.md")
    with open(readme, encoding="utf-8") as fh:
        text = fh.read()
    table = "| App | Wofür | Version | Quelle |\n|---|---|---|---|\n" + "\n".join(rows)
    text = re.sub(r"(<!-- apps:start -->\n).*?(\n<!-- apps:end -->)", lambda m: m.group(1) + table + m.group(2), text, flags=re.S)
    with open(readme, "w", encoding="utf-8") as fh:
        fh.write(text)
    print(f"{len(ids)} app pages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
