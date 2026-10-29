# Artcraft

> **This is a fork.** All apps, the launcher and the website were made by the ArtCraft team:
> **Original: [github.com/storytold](https://github.com/storytold)** · [getartcraft.com](https://getartcraft.com)
>
> Here you get the same apps without the Discord and social media links, built automatically from every new upstream release.

**[Website (English/German)](https://artcraft.labtoso.de/)** · **[Download all apps](https://github.com/Labtoso/Artcraft/releases/tag/suite)** · **[Launcher](apps/artcraft-launcher/README.md)**

## What is this?

The **Craft App Suite** is a collection of free, open-source desktop apps for creative work, written entirely in Rust: image editing, vector graphics, video editing, word processing, spreadsheets, presentations, CAD and more. They run natively on Windows, Linux and macOS (the original also in the browser). The **ArtCraft Launcher** installs and updates every app with one click.

This repository takes every version of these apps, removes the community promotion from them and publishes the results as ready-to-use downloads.

## Why this fork?

The original apps have buttons and links to Discord and social networks in many places, for example in the title bar, the sidebar, the help menu and the about window. If you just want to use the apps, you do not need them. Here the apps are exactly as they are, only without those links:

- **Removed:** Discord buttons and menu entries, links to X, Bluesky, LinkedIn, YouTube, Instagram, TikTok and Reddit.
- **Redirected:** GitHub links, bug reports and release pages point to this repository, website links point to the [mirrored website](https://artcraft.labtoso.de/).
- **Unchanged:** everything else, including features, look, file formats and license notices.

## The apps

<!-- apps:start -->
| App | What for | Version | Source |
|---|---|---|---|
| [ArtCraft Launcher](apps/artcraft-launcher/README.md) | Installs, updates and starts every Craft app | [0.9.2](https://github.com/Labtoso/Artcraft/releases/tag/artcraft-launcher-v0.9.2) | [Original](https://github.com/storytold/craft-launcher) |
| [CADCraft](apps/cadcraft/README.md) | CAD and technical drawing | [0.14.0](https://github.com/Labtoso/Artcraft/releases/tag/cadcraft-v0.14.0) | [Original](https://github.com/storytold/cadcraft) |
| [DeckCraft](apps/deckcraft/README.md) | Presentations | [0.12.0](https://github.com/Labtoso/Artcraft/releases/tag/deckcraft-v0.12.0) | [Original](https://github.com/storytold/deckcraft) |
| [DesignCraft](apps/designcraft/README.md) | Page layout and publishing | [0.14.0](https://github.com/Labtoso/Artcraft/releases/tag/designcraft-v0.14.0) | [Original](https://github.com/storytold/designcraft) |
| [EffectCraft](apps/effectcraft/README.md) | Motion graphics and VFX | [0.18.0](https://github.com/Labtoso/Artcraft/releases/tag/effectcraft-v0.18.0) | [Original](https://github.com/storytold/effectcraft) |
| [FigCraft](apps/figcraft/README.md) | Interface design | [0.12.1](https://github.com/Labtoso/Artcraft/releases/tag/figcraft-v0.12.1) | [Original](https://github.com/storytold/figcraft) |
| [FilmCraft](apps/filmcraft/README.md) | Video editing | [0.14.0](https://github.com/Labtoso/Artcraft/releases/tag/filmcraft-v0.14.0) | [Original](https://github.com/storytold/filmcraft) |
| [GridCraft](apps/gridcraft/README.md) | Spreadsheets | [0.13.0](https://github.com/Labtoso/Artcraft/releases/tag/gridcraft-v0.13.0) | [Original](https://github.com/storytold/gridcraft) |
| [LightCraft](apps/lightcraft/README.md) | Photo library and raw developer | [0.15.0](https://github.com/Labtoso/Artcraft/releases/tag/lightcraft-v0.15.0) | [Original](https://github.com/storytold/lightcraft) |
| [PdfCraft](apps/pdfcraft/README.md) | PDF workbench | [0.15.0](https://github.com/Labtoso/Artcraft/releases/tag/pdfcraft-v0.15.0) | [Original](https://github.com/storytold/pdfcraft) |
| [PhotoCraft](apps/photocraft/README.md) | Image editing | [0.17.0](https://github.com/Labtoso/Artcraft/releases/tag/photocraft-v0.17.0) | [Original](https://github.com/storytold/photocraft) |
| [SoundCraft](apps/soundcraft/README.md) | Digital audio workstation | [0.14.0](https://github.com/Labtoso/Artcraft/releases/tag/soundcraft-v0.14.0) | [Original](https://github.com/storytold/soundcraft) |
| [VectorCraft](apps/vectorcraft/README.md) | Vector illustration | [0.18.0](https://github.com/Labtoso/Artcraft/releases/tag/vectorcraft-v0.18.0) | [Original](https://github.com/storytold/vectorcraft) |
| [WordCraft](apps/wordcraft/README.md) | Word processing | [0.12.0](https://github.com/Labtoso/Artcraft/releases/tag/wordcraft-v0.12.0) | [Original](https://github.com/storytold/wordcraft) |
<!-- apps:end -->

Every app has its own folder under [`apps/`](apps/) with a description, the current version, downloads and any extra rules.

## Install

1. **Easiest:** Download the launcher from the [suite release](https://github.com/Labtoso/Artcraft/releases/tag/suite), `artcraft-launcher-…-windows-x64.msi` for Windows, `artcraft-launcher-…-linux-x86_64.AppImage` for Linux or `artcraft-launcher-…-macos-universal.dmg` for macOS. Start it, pick an app, install. The launcher keeps everything up to date, itself included.
2. **Single app:** Take the matching file from the suite release or from the app's own release (`<app>-v<version>`): `.msi` (installer) or `-portable.zip` for Windows, `.AppImage`, `.deb`, `.rpm` or `.tar.gz` for Linux, `macos-universal.dmg` for macOS (Apple silicon and Intel).
3. **Website:** The download buttons on [artcraft.labtoso.de](https://artcraft.labtoso.de/) download the file for your system directly.

The files are not signed by Microsoft or Apple. Windows SmartScreen asks on first start ("More info" → "Run anyway"). On macOS, open *System Settings → Privacy & Security* after the first start and click *Open Anyway*, or run `xattr -dr com.apple.quarantine "/Applications/<App>.app"`.

## How it works

Everything runs automatically on GitHub Actions, nobody has to start anything:

1. **Discover** ([`tools/discover.py`](tools/discover.py)), every day at 01:00 German time: which apps exist upstream (the original launcher's app list and all `*craft` repositories), and which one has a new version? New apps are picked up on their own.
2. **Debrand** ([`tools/debrand.py`](tools/debrand.py)): the Rust code of every app is searched for Discord and social networks. The code that shows the button, menu entry or link is removed as a whole statement or list entry. Then the remaining links are rewritten. Extra rules per app live in `apps/<app>/debrand.toml`.
3. **Check** ([`tools/prepare.py`](tools/prepare.py)): `cargo check` has to pass. If a file no longer compiles after a removal, its code stays original and only its links are rewritten, so a broken version never reaches a release.
4. **Build:** Windows x64, Linux x86_64 and macOS (universal, Apple silicon and Intel), with each app's original packaging scripts.
5. **Publish** ([`tools/publish.py`](tools/publish.py)): one release per app version (`wordcraft-v0.5.0` and so on) and the **[`suite`](https://github.com/Labtoso/Artcraft/releases/tag/suite)** release, which always holds every current app and the launcher. If a build fails, the previous version stays online and an issue is opened.
6. **Launcher** ([`tools/patch_launcher.py`](tools/patch_launcher.py)): the launcher looks exactly like the original, without the community box. It gets apps and its own updates from this repository's releases through the feed [`release_info.json`](https://artcraft.labtoso.de/release_info.json).
7. **Website** ([`tools/site.py`](tools/site.py)), every day at 01:00 German time and after every update: [getartcraft.com](https://getartcraft.com) is mirrored 1:1 to [artcraft.labtoso.de](https://artcraft.labtoso.de/), without ArtCraft Studio, pricing, support, Discord, social links and browser versions. Download buttons point to these releases, "Original on GitHub" links to the original, and the footer holds the credits. The site switches to German automatically when the browser or system is set to German, otherwise it shows English, and DE/EN switches by hand. Hand-made translations are in [`site/i18n/de.json`](site/i18n/de.json), new texts are translated automatically ([`de.auto.json`](site/i18n/de.auto.json)).

To start an update by hand: *Actions → Update apps → Run workflow* (optionally with `all` or app names to rebuild).

## Credits and license

All apps, the launcher and the website were created by the **ArtCraft team and the contributors** of the [original project](https://github.com/storytold). The credit for the apps, their design and the website belongs to them.

The apps are licensed under **MIT or Apache-2.0** (at your option). The original license files (`LICENSE-MIT`, `LICENSE-APACHE`) and notices (`NOTICE`, `ATTRIBUTION.md`) stay in every build. The code in this repository is also MIT or Apache-2.0, see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). The name "ArtCraft" and the logos are trademarks of the ArtCraft team and are not covered by these licenses. This project is not affiliated with the ArtCraft team.

© LabtosoStudios
