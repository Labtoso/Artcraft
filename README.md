# Artcraft

> **Dies ist ein Fork.** Alle Apps, der Launcher und die Website stammen vom ArtCraft-Team:
> **Original: [github.com/storytold](https://github.com/storytold)** · [getartcraft.com](https://getartcraft.com)
>
> Hier gibt es dieselben Apps, nur ohne Discord- und Social-Media-Links, automatisch aus jedem neuen Original-Release gebaut.

**[Website (Deutsch/Englisch)](https://labtoso.github.io/Artcraft/)** · **[Alle Apps herunterladen](https://github.com/Labtoso/Artcraft/releases/tag/suite)** · **[Launcher](apps/artcraft-launcher/README.md)**

## Was ist das?

Die **Craft App Suite** ist eine Sammlung kostenloser Open-Source-Desktop-Apps für kreative Arbeit, komplett in Rust geschrieben: Bildbearbeitung, Vektorgrafik, Videoschnitt, Textverarbeitung, Tabellen, Präsentationen, CAD und mehr. Sie laufen nativ auf Windows und Linux (das Original auch auf macOS und im Browser). Der **ArtCraft Launcher** installiert und aktualisiert alle Apps mit einem Klick.

Dieses Repo nimmt jede Version dieser Apps, entfernt die Community-Werbung daraus und stellt die Ergebnisse als fertige Downloads bereit.

## Warum dieser Fork?

In den Original-Apps sind an vielen Stellen Buttons und Links zu Discord und sozialen Netzwerken eingebaut, zum Beispiel in der Titelleiste, in der Seitenleiste, im Hilfe-Menü und im Info-Fenster. Wer die Apps einfach nur benutzen will, braucht das nicht. Hier sind die Apps genauso, wie sie sind, nur ohne diese Links:

- **Entfernt:** Discord-Buttons und -Menüeinträge, Links zu X, Bluesky, LinkedIn, YouTube, Instagram, TikTok und Reddit.
- **Umgeleitet:** GitHub-Links, Fehlermeldungen und Release-Seiten zeigen auf dieses Repo, Links zur Website auf die [gespiegelte Website](https://labtoso.github.io/Artcraft/).
- **Unverändert:** alles andere, also Funktionen, Aussehen, Dateiformate und die Lizenzhinweise.

## Die Apps

<!-- apps:start -->
| App | Wofür | Version | Quelle |
|---|---|---|---|
| [ArtCraft Launcher](apps/artcraft-launcher/README.md) | Installiert, aktualisiert und startet alle Craft-Apps | [0.9.0](https://github.com/Labtoso/Artcraft/releases/tag/artcraft-launcher-v0.9.0) | [Original](https://github.com/storytold/craft-launcher) |
| [CADCraft](apps/cadcraft/README.md) | CAD und technisches Zeichnen | [0.14.0](https://github.com/Labtoso/Artcraft/releases/tag/cadcraft-v0.14.0) | [Original](https://github.com/storytold/cadcraft) |
| [DeckCraft](apps/deckcraft/README.md) | Präsentationen | [0.12.0](https://github.com/Labtoso/Artcraft/releases/tag/deckcraft-v0.12.0) | [Original](https://github.com/storytold/deckcraft) |
| [DesignCraft](apps/designcraft/README.md) | Seitenlayout und Publishing | [0.14.0](https://github.com/Labtoso/Artcraft/releases/tag/designcraft-v0.14.0) | [Original](https://github.com/storytold/designcraft) |
| [EffectCraft](apps/effectcraft/README.md) | Motion Graphics und VFX | [0.18.0](https://github.com/Labtoso/Artcraft/releases/tag/effectcraft-v0.18.0) | [Original](https://github.com/storytold/effectcraft) |
| [FigCraft](apps/figcraft/README.md) | Interface-Design | [0.12.1](https://github.com/Labtoso/Artcraft/releases/tag/figcraft-v0.12.1) | [Original](https://github.com/storytold/figcraft) |
| [FilmCraft](apps/filmcraft/README.md) | Videoschnitt | [0.14.0](https://github.com/Labtoso/Artcraft/releases/tag/filmcraft-v0.14.0) | [Original](https://github.com/storytold/filmcraft) |
| [GridCraft](apps/gridcraft/README.md) | Tabellenkalkulation | [0.13.0](https://github.com/Labtoso/Artcraft/releases/tag/gridcraft-v0.13.0) | [Original](https://github.com/storytold/gridcraft) |
| [LightCraft](apps/lightcraft/README.md) | Fotobibliothek und RAW-Entwickler | [0.15.0](https://github.com/Labtoso/Artcraft/releases/tag/lightcraft-v0.15.0) | [Original](https://github.com/storytold/lightcraft) |
| [PdfCraft](apps/pdfcraft/README.md) | PDF-Werkbank | [0.15.0](https://github.com/Labtoso/Artcraft/releases/tag/pdfcraft-v0.15.0) | [Original](https://github.com/storytold/pdfcraft) |
| [PhotoCraft](apps/photocraft/README.md) | Bildbearbeitung | [0.17.0](https://github.com/Labtoso/Artcraft/releases/tag/photocraft-v0.17.0) | [Original](https://github.com/storytold/photocraft) |
| [SoundCraft](apps/soundcraft/README.md) | Digitale Audio-Workstation | [0.14.0](https://github.com/Labtoso/Artcraft/releases/tag/soundcraft-v0.14.0) | [Original](https://github.com/storytold/soundcraft) |
| [VectorCraft](apps/vectorcraft/README.md) | Vektorillustration | [0.18.0](https://github.com/Labtoso/Artcraft/releases/tag/vectorcraft-v0.18.0) | [Original](https://github.com/storytold/vectorcraft) |
| [WordCraft](apps/wordcraft/README.md) | Textverarbeitung | [0.12.0](https://github.com/Labtoso/Artcraft/releases/tag/wordcraft-v0.12.0) | [Original](https://github.com/storytold/wordcraft) |
<!-- apps:end -->

Jede App hat einen eigenen Ordner unter [`apps/`](apps/) mit Beschreibung, aktueller Version, Downloads und eventuellen Zusatzregeln.

## Installieren

1. **Am einfachsten:** Im [Suite-Release](https://github.com/Labtoso/Artcraft/releases/tag/suite) den Launcher herunterladen, `artcraft-launcher-…-windows-x64.msi` für Windows oder `artcraft-launcher-…-linux-x86_64.AppImage` für Linux. Starten, App auswählen, installieren. Der Launcher hält danach alles aktuell.
2. **Einzelne App:** Im Suite-Release oder im Release der App (`<app>-v<version>`) die passende Datei nehmen: `.msi` (Installer) oder `-portable.zip` für Windows, `.AppImage`, `.deb`, `.rpm` oder `.tar.gz` für Linux.

Die Windows-Dateien sind nicht signiert. Windows SmartScreen fragt deshalb beim ersten Start nach („Weitere Informationen“ → „Trotzdem ausführen“).

## So funktioniert es

Alles läuft automatisch über GitHub Actions, ohne dass jemand etwas anstoßen muss:

1. **Erkennen** ([`tools/discover.py`](tools/discover.py)), alle 6 Stunden: Welche Apps gibt es im Original (App-Liste des originalen Launchers und alle `*craft`-Repos), und welche hat eine neue Version? Neue Apps kommen dadurch von selbst dazu.
2. **Debranden** ([`tools/debrand.py`](tools/debrand.py)): Der Rust-Code jeder App wird nach Discord und sozialen Netzwerken durchsucht. Der Code, der den Button, Menüeintrag oder Link anzeigt, wird als ganze Anweisung bzw. als ganzer Listeneintrag entfernt. Danach werden die übrigen Links umgeschrieben. Zusatzregeln pro App liegen in `apps/<app>/debrand.toml`.
3. **Prüfen** ([`tools/prepare.py`](tools/prepare.py)): `cargo check` muss durchlaufen. Wenn eine Datei nach dem Entfernen nicht mehr kompiliert, bleibt ihr Code original, und es werden dort nur die Links umgeschrieben. So landet nie eine kaputte Version im Release.
4. **Bauen**: Windows x64 und Linux x86_64, mit den Original-Packaging-Skripten der jeweiligen App.
5. **Veröffentlichen** ([`tools/publish.py`](tools/publish.py)): ein Release pro App-Version (`wordcraft-v0.5.0` usw.) und der große Release **[`suite`](https://github.com/Labtoso/Artcraft/releases/tag/suite)**, der immer alle aktuellen Apps und den Launcher enthält. Schlägt ein Build fehl, bleibt die vorherige Version online und es wird ein Issue angelegt.
6. **Launcher** ([`tools/patch_launcher.py`](tools/patch_launcher.py)): Der Launcher sieht genauso aus wie das Original. Er holt die Apps aber aus den Releases dieses Repos, über den Feed [`release_info.json`](https://labtoso.github.io/Artcraft/release_info.json).
7. **Website** ([`tools/site.py`](tools/site.py)), täglich und nach jedem Update: [getartcraft.com](https://getartcraft.com) wird 1:1 gespiegelt, ohne „Launch App“, Pricing, Discord und Social-Links. Die Download-Buttons zeigen auf diese Releases, oben rechts gibt es den Button „Original auf GitHub“ und unten die Credits. Die Seite erscheint automatisch auf Deutsch, wenn Browser oder System auf Deutsch eingestellt sind, sonst auf Englisch, und lässt sich mit DE/EN umschalten. Eigene Übersetzungen stehen in [`site/i18n/de.json`](site/i18n/de.json), neue Texte werden automatisch übersetzt ([`de.auto.json`](site/i18n/de.auto.json)).

Ein Update von Hand anstoßen: *Actions → Update apps → Run workflow* (optional mit `all` oder App-Namen, um neu zu bauen).

## Credits und Lizenz

Alle Apps, der Launcher und die Website wurden vom **ArtCraft-Team und den Mitwirkenden** des [Originalprojekts](https://github.com/storytold) entwickelt. Der Verdienst für die Apps, ihr Design und die Website gehört ihnen.

Die Apps stehen unter **MIT oder Apache-2.0** (nach Wahl). Die Lizenzdateien (`LICENSE-MIT`, `LICENSE-APACHE`) und Hinweise (`NOTICE`, `ATTRIBUTION.md`) der Originale bleiben in jedem Build enthalten. Der Code dieses Repos steht ebenfalls unter MIT oder Apache-2.0, siehe [LICENSE-MIT](LICENSE-MIT) und [LICENSE-APACHE](LICENSE-APACHE). Der Name „ArtCraft“ und die Logos sind Marken des ArtCraft-Teams und fallen nicht unter diese Lizenzen. Dieses Projekt ist nicht mit dem ArtCraft-Team verbunden.
