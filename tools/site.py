#!/usr/bin/env python3
"""The website: the original getartcraft.com, debranded, with German, under /Artcraft/.

The original runs with its own scripts (animations, 3D, menus), so it looks and moves exactly
like the original. tools/capture.js saves it with every file a browser loads; this script
  * copies that capture to _site/ and points every path at /Artcraft/ (videos stay on the
    original server),
  * leaves out the pages that are only about the AI studio (pricing, tutorials, news, FAQ) and
    the original's analytics,
  * adds tools/site_runtime.js to every page: it hides the AI studio, pricing, Discord and social
    links once the original scripts have built the page, turns "Launch App" into "Download", puts
    "Original on GitHub" into the menu, points GitHub links and downloads at this repository's
    builds, and switches to German when the browser asks for it (DE/EN switch in the top bar),
  * writes the launcher feed release_info.json from state/versions.json.

  site.py build --raw RAW_DIR --out _site            copy and prepare the pages
  site.py translate --texts texts.json --out _site   German for every text tools/collect.js found
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import sys
import time
import urllib.parse
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
REPO = os.environ.get("ARTCRAFT_REPO", "Labtoso/Artcraft")
REPO_URL = f"https://github.com/{REPO}"
SITE = os.environ.get("ARTCRAFT_SITE", "https://labtoso.github.io/Artcraft").rstrip("/")
BASE = urllib.parse.urlparse(SITE).path.rstrip("/")  # "/Artcraft"
UPSTREAM = os.environ.get("UPSTREAM", "storytold")
ORIGINAL_GITHUB = f"https://github.com/{UPSTREAM}"
ORIGIN = "https://getartcraft.com"
UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36"
MAX_ASSET = 90 * 1024 * 1024

# pages that are only about the original's AI studio or its pricing
SKIP_PATHS = re.compile(r"^/(pricing|tutorials|news|faq)(/|$)", re.I)
# the section index on the home page (drawn by the original's 3D script) without the studio parts
JS_PATCHES = [
    (re.compile(r'\{id:"(?:studio|features|ownership|made-with|pricing)",label:"[^"]*"\},?'), ""),
]
# files that stay on the original server (large media)
REMOTE = re.compile(r"\.(mp4|webm|mov|m4v|zip|dmg|exe|msi)$", re.I)

CSS = r"""
[data-ac-hide],#studio,#features,#ownership,#made-with,.marquee,
a[href="#studio"],a[href="#features"],a[href="#ownership"],a[href="#made-with"],
html[data-ac-page="/press-kit/"] #articles,html[data-ac-page="/press-kit/"] #contact,
html[data-ac-page="/apps/"] #start,
html[data-ac-page="/download/"] main>section:nth-of-type(2){display:none!important}
#start .grid:has(>article[data-ac-hide]){grid-template-columns:minmax(0,1fr)!important}
@media (min-width:768px){.md\:grid-cols-3:has(>article[data-ac-hide]){grid-template-columns:repeat(2,minmax(0,1fr))!important}}
footer .grid:has(>nav[data-ac-hide]){grid-template-columns:repeat(auto-fit,minmax(10rem,1fr))!important}
footer .grid:has(>nav[data-ac-hide])>:first-child{grid-column:1/-1!important}
@media (min-width:1024px){footer .grid:has(>nav[data-ac-hide]){grid-template-columns:2fr 1fr 1fr!important}footer .grid:has(>nav[data-ac-hide])>:first-child{grid-column:auto!important}}
html.ac-wait body{opacity:0!important}
"""

CREDITS = (
    '<p><strong class="text-ink">Credits.</strong> ArtCraft and every Craft app were created by the ArtCraft team '
    'and the contributors of the <a href="{orig}" target="_blank" rel="noopener" class="underline">original project on GitHub</a>. '
    'All credit for the apps, their design and this website goes to them.</p>'
    '<p class="mt-2">This is a fork: the same apps without community links, built automatically from every original release. '
    'Source, builds and how it works: <a href="{repo}" target="_blank" rel="noopener" class="underline">github.com/{name}</a>. '
    "The apps are licensed under MIT or Apache-2.0; ArtCraft's name and logos are the ArtCraft team's trademarks.</p>"
)

def fetch(url: str) -> tuple[bytes, str] | None:
    for attempt in range(2):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept-Language": "en"})
            with urllib.request.urlopen(req, timeout=30) as r:
                length = int(r.headers.get("Content-Length") or 0)
                if length > MAX_ASSET:
                    return None
                return r.read(), r.headers.get("Content-Type", "")
        except urllib.error.HTTPError as e:
            if e.code in (404, 410, 403):
                return None
        except Exception:
            pass
        time.sleep(1)
    return None


def load_state() -> dict:
    path = os.path.join(ROOT, "state", "versions.json")
    if os.path.exists(path):
        with open(path, encoding="utf-8") as fh:
            return json.load(fh)
    return {}


def write_feed(out: str, state: dict):
    """The launcher's release feed, in the format the original launcher reads."""
    apps = {}
    for app_id, s in state.items():
        apps[app_id] = {
            "repo": REPO,
            "app_last_fetched": s["published_at"],
            "latest_release": {
                "tag": "v" + s["version"],
                "name": f"{s['name']} {s['version']}",
                "published_at": s["published_at"],
                "html_url": f"{REPO_URL}/releases/tag/{s['tag']}",
                "prerelease": False,
                "notes_markdown": s.get("notes", ""),
                "assets": [{"name": a["name"], "size_bytes": a["size_bytes"], "download_url": a["download_url"], "sha256": a["sha256"]} for a in s["assets"]],
            },
        }
    oldest = min((s["published_at"] for s in state.values()), default=None)
    feed = {
        "release_info": {"apps": apps, "last_fetched": oldest},
        "app_renames": [],
        "repo_renames": [],
        "important_notices": {"announcements": [], "alerts": []},
    }
    with open(os.path.join(out, "release_info.json"), "w", encoding="utf-8") as fh:
        json.dump(feed, fh, indent=1, ensure_ascii=False)


# names that stay as they are in German
KEEP_NAMES = re.compile(r"\b(ArtCraft Launcher|ArtCraft Studio|ArtCraft|Crafting Apps?|Craft Apps?|[A-Z][A-Za-z]*Craft|LabtosoStudios|GitHub|Rust|WebAssembly|macOS|Windows|Linux)\b")


def fix_names(de_text: str) -> str:
    """Undo what translation does to names: 'Crafting-Apps' -> 'Crafting Apps', 'Pdf Craft' -> 'PdfCraft'."""
    t = re.sub(r"\b(Crafting|Craft)-(Apps?)\b", r"\1 \2", de_text)
    t = re.sub(r"\b(Photo|Vector|Film|Light|Pdf|Effect|Design|Fig|Word|Sound|Deck|CAD|Grid|Art)\s+Craft\b", r"\1Craft", t)
    return t


AUTO = os.path.join(ROOT, "site", "i18n", "de.auto.json")
KEEP_ENGLISH = re.compile(r"^(ArtCraft|[A-Z][a-z]+Craft|CADCraft|PdfCraft|macOS|Windows|Linux|Web|GitHub|Rust|MCP|v?\d[\d.]*|[\w.+-]+@[\w.-]+|https?://\S+|©.*)$")


def _gtx_single(text: str) -> str:
    body = urllib.parse.urlencode({"q": text}).encode("utf-8")
    req = urllib.request.Request(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=en&tl=de&dt=t",
        data=body, headers={"User-Agent": UA, "Content-Type": "application/x-www-form-urlencoded;charset=utf-8"})
    with urllib.request.urlopen(req, timeout=20) as r:
        data = json.loads(r.read().decode("utf-8"))
    return "".join(part[0] for part in data[0] if part and part[0])


def _gtx_chrome(text: str) -> str:
    body = urllib.parse.urlencode({"q": text}).encode("utf-8")
    req = urllib.request.Request(
        "https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=en&tl=de",
        data=body, headers={"User-Agent": UA, "Content-Type": "application/x-www-form-urlencoded;charset=utf-8"})
    with urllib.request.urlopen(req, timeout=20) as r:
        data = json.loads(r.read().decode("utf-8"))
    first = data[0] if isinstance(data, list) else data
    if isinstance(first, list):
        first = first[0]
    if isinstance(first, dict):
        first = first.get("trans") or ""
    return str(first)


GTX_ERRORS = []
GTX_WAIT = [0.0]


def gtx(text: str) -> str | None:
    """Free Google endpoints, one after the other; None when none answers.
    A 429 (too many requests) makes the next calls wait longer."""
    for fn in (_gtx_single, _gtx_chrome):
        if GTX_WAIT[0]:
            time.sleep(GTX_WAIT[0])
        try:
            out = fn(text)
            if out:
                GTX_WAIT[0] = max(0.3, GTX_WAIT[0] / 2)
                return out
        except Exception as e:
            if getattr(e, "code", None) == 429:
                GTX_WAIT[0] = min(30.0, max(2.0, GTX_WAIT[0] * 2))
            if len(GTX_ERRORS) < 4:
                GTX_ERRORS.append(repr(e))
                print(f"translation request failed ({fn.__name__}): {e!r}", flush=True)
    return None


def gtx_protected(text: str) -> str | None:
    """Translate with names swapped for tokens Google leaves alone, then put them back."""
    names = []
    def keep(m):
        names.append(m.group(0))
        return f"QX{len(names) - 1}Q"
    out = gtx(KEEP_NAMES.sub(keep, text))
    if out is None:
        return None
    out = re.sub(r"QX\s*(\d+)\s*Q", lambda m: names[int(m.group(1))] if int(m.group(1)) < len(names) else m.group(0), out)
    return fix_names(out)


def translate_missing(missing: list[str]) -> dict:
    """German for texts nobody translated by hand yet; cached in site/i18n/de.auto.json.
    Texts go in batches of lines; the whole step stops after 10 minutes or 5 failures in a row
    (whatever is left stays English until the next run)."""
    cache = {}
    if os.path.exists(AUTO):
        with open(AUTO, encoding="utf-8") as fh:
            cache = json.load(fh)
    # older translations that lost a name (Rust -> Rost) are done again
    for en in list(cache):
        if any(name not in cache[en] for name in KEEP_NAMES.findall(en)):
            del cache[en]
    todo = [t for t in missing if t not in cache and not KEEP_ENGLISH.match(t) and "\n" not in t]
    deadline = time.time() + 600
    fails = done = 0
    i = 0
    per = 40
    while i < len(todo) and time.time() < deadline and fails < 8:
        batch, size = [], 0
        while i < len(todo) and size + len(todo[i]) < 3000 and len(batch) < per:
            batch.append(todo[i])
            size += len(todo[i]) + 1
            i += 1
        if not batch:
            batch = [todo[i]]
            i += 1
        out = gtx_protected("\n".join(batch))
        lines = out.split("\n") if out is not None else None
        if lines is None:
            fails += 1
            if len(batch) > 1:
                # same texts again, in smaller batches
                i -= len(batch)
                per = max(1, len(batch) // 2)
            continue
        fails = 0
        if len(lines) != len(batch):
            lines = [gtx_protected(t) for t in batch]
        for en, de_text in zip(batch, lines):
            if not de_text:
                continue
            de_text = de_text.strip()
            for name in re.findall(r"\b(?:ArtCraft|[A-Z][a-z]+Craft|CADCraft|PdfCraft)\b", en):
                de_text = re.sub(re.escape(name), name, de_text, flags=re.I)
            cache[en] = de_text
            done += 1
    os.makedirs(os.path.dirname(AUTO), exist_ok=True)
    with open(AUTO, "w", encoding="utf-8") as fh:
        json.dump(dict(sorted(cache.items())), fh, indent=1, ensure_ascii=False)
        fh.write("\n")
    print(f"German: {done} new machine translations, {len(cache)} cached, {len(todo) - done} left for next time", flush=True)
    return cache


# ---- build ------------------------------------------------------------------------------------

def runtime_config(state: dict) -> dict:
    apps = {}
    for app_id, s in state.items():
        apps[app_id] = {
            "version": s["version"],
            "release": f"{REPO_URL}/releases/tag/{s['tag']}",
            "assets": {a["name"]: a["download_url"] for a in s["assets"]},
        }
    known = {}
    for d in os.listdir(os.path.join(ROOT, "apps")) if os.path.isdir(os.path.join(ROOT, "apps")) else []:
        known[d] = True
    return {
        "base": BASE,
        "repo": REPO_URL,
        "original": ORIGINAL_GITHUB,
        "apps": apps,
        "known": known,
        "credits": CREDITS.format(orig=ORIGINAL_GITHUB, repo=REPO_URL, name=REPO),
    }


def head_block(config: dict) -> str:
    with open(os.path.join(HERE, "site_runtime.js"), encoding="utf-8") as fh:
        js = fh.read().replace("__CONFIG__", json.dumps(config, ensure_ascii=False))
    js = js.replace("</", "<\\/")
    csp = ("default-src 'self' https: data: blob:; script-src 'self' 'unsafe-inline' 'unsafe-eval' blob:; "
           "connect-src 'self' blob: data:; img-src 'self' https: data: blob:; media-src 'self' https: blob: data:; "
           "worker-src 'self' blob:; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' data: https://fonts.gstatic.com")
    return (f'<meta http-equiv="Content-Security-Policy" content="{csp}"/>'
            f'<style data-ac-own="css">{CSS}</style>'
            f'<script data-ac-own="runtime">{js}</script>')


class Builder:
    def __init__(self, raw: str, out: str):
        self.raw = raw
        self.out = out
        self.dst = os.path.join(out, BASE.strip("/"))
        self.top = set()  # first path parts of the files we have: _next, images, ...
        self.state = load_state()

    def files(self):
        for root, _, names in os.walk(self.raw):
            for n in names:
                full = os.path.join(root, n)
                yield os.path.relpath(full, self.raw).replace(os.sep, "/"), full

    def fix_paths(self, text: str) -> str:
        """Root paths the copy has -> under BASE; media that stays remote -> the original server.
        Works in HTML, CSS, JS and the JSON inside the page's data scripts."""
        for d in sorted(self.top, key=len, reverse=True):
            pat = re.escape("/" + d)
            text = re.sub(r'(?<=["\'`(=])' + pat + r'(?=[/"\'`)?\\\s])', BASE + "/" + d, text)
        for d in ("videos",):
            text = re.sub(r'(?<=["\'`(=])/' + d + r'/', ORIGIN + "/" + d + "/", text)
            text = re.sub(r'(?<=\\")/' + d + r'/', ORIGIN + "/" + d + "/", text)
        return text

    def fetch_missing_chunks(self):
        """Scripts the page loads only later (named inside other scripts) that the capture missed."""
        seen = set()
        while True:
            want = set()
            for rel, full in list(self.files()):
                if not rel.endswith((".js", ".html")):
                    continue
                text = open(full, encoding="utf-8", errors="replace").read()
                for m in re.findall(r'static/(?:chunks|media|css)/[\w./~-]+\.(?:js|css|woff2?|png|svg|webp|jpg)', text):
                    if not os.path.exists(os.path.join(self.raw, "_next", m)):
                        want.add(m)
            want -= seen
            if not want:
                return
            for m in sorted(want):
                seen.add(m)
                got = fetch(f"{ORIGIN}/_next/{m}")
                if got:
                    target = os.path.join(self.raw, "_next", m)
                    os.makedirs(os.path.dirname(target), exist_ok=True)
                    with open(target, "wb") as fh:
                        fh.write(got[0])
                    print(f"fetched _next/{m}", flush=True)

    def page_html(self, text: str, head: str) -> str:
        # the original's analytics stay out
        text = re.sub(r'<link[^>]+googletagmanager[^>]*>', "", text)
        text = re.sub(r'<script[^>]*googletagmanager[^>]*>\s*</script>', "", text)
        text = self.fix_paths(text)
        text = re.sub(r"(<head[^>]*>)", lambda m: m.group(1) + head, text, count=1)
        return text

    def build(self):
        self.fetch_missing_chunks()
        for rel, _ in self.files():
            self.top.add(rel.split("/")[0])
        # only file folders: page links stay as the original scripts expect them (the runtime
        # fixes them on the page), so the scripts' own path checks keep working
        pages = {rel.split("/")[0] for rel, _ in self.files() if rel.endswith("index.html") and "/" in rel}
        self.top = {t for t in self.top if t != "index.html" and t not in pages}
        shutil.rmtree(self.dst, ignore_errors=True)
        head = head_block(runtime_config(self.state))
        pages = 0
        for rel, full in self.files():
            if rel.endswith("index.html") and SKIP_PATHS.match("/" + rel):
                continue
            target = os.path.join(self.dst, rel)
            os.makedirs(os.path.dirname(target), exist_ok=True)
            if rel.endswith((".html", ".js", ".css")):
                text = open(full, encoding="utf-8", errors="surrogateescape").read()
                if rel.endswith(".html"):
                    text = self.page_html(text, head)
                    pages += 1
                else:
                    if rel.endswith(".js"):
                        # the script loader's own base path
                        text = text.replace('="/_next/"', f'="{BASE}/_next/"')
                        for pat, repl in JS_PATCHES:
                            text = pat.sub(repl, text)
                    text = self.fix_paths(text)
                with open(target, "w", encoding="utf-8", errors="surrogateescape") as fh:
                    fh.write(text)
            else:
                shutil.copyfile(full, target)
        home = os.path.join(self.dst, "index.html")
        if os.path.exists(home):
            shutil.copyfile(home, os.path.join(self.dst, "404.html"))
        with open(os.path.join(self.dst, ".nojekyll"), "w") as fh:
            fh.write("")
        write_feed(self.dst, self.state)
        write_german(self.dst, {})
        print(f"{pages} pages")
        return pages


def write_german(dst: str, de: dict):
    """i18n/de.json: the hand-made German (site/i18n/de.json) over the machine translations."""
    with open(os.path.join(ROOT, "site", "i18n", "de.json"), encoding="utf-8") as fh:
        hand = {norm_text(k): v for k, v in json.load(fh).items() if not k.startswith("_")}
    auto = {}
    if os.path.exists(AUTO):
        with open(AUTO, encoding="utf-8") as fh:
            auto = json.load(fh)
    merged = {k: fix_names(v) for k, v in {**auto, **de, **hand}.items()}
    os.makedirs(os.path.join(dst, "i18n"), exist_ok=True)
    with open(os.path.join(dst, "i18n", "de.json"), "w", encoding="utf-8") as fh:
        json.dump(merged, fh, ensure_ascii=False)
    return merged


def norm_text(t: str) -> str:
    return re.sub(r"\s+", " ", t).strip()


def main() -> int:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build")
    b.add_argument("--raw", required=True)
    b.add_argument("--out", default=os.path.join(ROOT, "_site"))
    t = sub.add_parser("translate")
    t.add_argument("--texts", required=True)
    t.add_argument("--out", default=os.path.join(ROOT, "_site"))
    args = ap.parse_args()
    dst = os.path.join(args.out, BASE.strip("/"))

    if args.cmd == "build":
        if not os.path.exists(os.path.join(args.raw, "index.html")):
            print("the capture has no home page; keeping the previous website", file=sys.stderr)
            return 1
        pages = Builder(args.raw, args.out).build()
        print(f"::notice title=Website::{pages} pages")
        return 0

    with open(args.texts, encoding="utf-8") as fh:
        texts = {norm_text(t) for t in json.load(fh)}
    with open(os.path.join(ROOT, "site", "i18n", "de.json"), encoding="utf-8") as fh:
        hand = {norm_text(k) for k in json.load(fh) if not k.startswith("_")}
    auto = translate_missing(sorted(t for t in texts if t and t not in hand))
    merged = write_german(dst, auto)
    left = [t for t in texts if t not in merged and not KEEP_ENGLISH.match(t)]
    print(f"::notice title=German::{len(texts)} texts, {len(left)} still English")
    return 0


if __name__ == "__main__":
    sys.exit(main())
