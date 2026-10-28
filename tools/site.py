#!/usr/bin/env python3
"""Mirror the original website, debrand it, add German, and write it to _site/.

  * every page of the original site is fetched and saved as <path>/index.html, with the files it
    needs (styles, fonts, images); the site's own scripts are dropped, so the page is plain HTML
    and nothing re-renders it back to the original,
  * gone: "Launch App", Pricing (link and page), Discord and every social network / share button,
  * GitHub links point to this repo, downloads of the apps and the launcher to this repo's builds,
  * the top-right button becomes "Original on GitHub" (to the original project), a credits block
    goes into the footer,
  * German: site/i18n/de.json maps English text to German; the page switches to German when the
    browser/system language is German (a DE/EN switch overrides it),
  * release_info.json (the launcher feed) is written from state/versions.json.

Usage: site.py [--origin https://getartcraft.com] [--out _site] [--snapshot DIR]
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import posixpath
import re
import sys
import time
import urllib.parse
import urllib.request

from bs4 import BeautifulSoup, Comment, NavigableString

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
REPO = os.environ.get("ARTCRAFT_REPO", "Labtoso/Artcraft")
REPO_URL = f"https://github.com/{REPO}"
SITE = os.environ.get("ARTCRAFT_SITE", "https://labtoso.github.io/Artcraft").rstrip("/")
BASE = urllib.parse.urlparse(SITE).path.rstrip("/")  # "/Artcraft"
UPSTREAM = os.environ.get("UPSTREAM", "storytold")
ORIGINAL_GITHUB = f"https://github.com/{UPSTREAM}"
UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36"

SOCIAL = re.compile(r"discord\.(gg|com)|bsky\.app|bluesky|twitter\.com|(^|//|\.)x\.com/|linkedin\.com|youtube\.com/@|instagram\.com|tiktok\.com|reddit\.com/(submit|r/)|facebook\.com/sharer|t\.me/share", re.I)
# pages that are only about the original's AI studio or its pricing
SKIP_PATHS = re.compile(r"^/(pricing|tutorials|news|faq)(/|$)", re.I)
LAUNCH_APP = re.compile(r"^\s*launch app\s*$", re.I)
SHARE_LABELS = {"x", "reddit", "bluesky", "linkedin", "copy link", "share", "facebook"}
MAX_ASSET = 90 * 1024 * 1024


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


class Mirror:
    def __init__(self, origin: str, out: str):
        self.origin = origin.rstrip("/")
        self.host = urllib.parse.urlparse(self.origin).netloc
        self.out = out
        self.pages: dict[str, str] = {}  # path -> html
        self.assets: set[str] = set()
        self.state = load_state()

    # ---- url helpers ----
    def is_internal(self, url: str) -> bool:
        p = urllib.parse.urlparse(url)
        return (not p.netloc or p.netloc == self.host) and p.scheme in ("", "http", "https")

    def norm_page(self, path: str) -> str:
        path = urllib.parse.urlparse(path).path or "/"
        path = re.sub(r"/index\.html$", "/", path)
        if not path.endswith("/"):
            path += "/"
        return path

    def local(self, path: str) -> str:
        """Internal absolute path -> path on the mirror (under BASE)."""
        return BASE + path

    # ---- crawling ----
    def crawl(self, start: list[str]):
        queue = [self.norm_page(p) for p in start]
        seen = set(queue)
        while queue and len(self.pages) < 400:
            path = queue.pop(0)
            got = fetch(self.origin + path)
            if not got or "html" not in got[1]:
                continue
            html = got[0].decode("utf-8", "replace")
            self.pages[path] = html
            soup = BeautifulSoup(html, "html.parser")
            for a in soup.find_all("a", href=True):
                href = urllib.parse.urljoin(self.origin + path, a["href"])
                if not self.is_internal(href):
                    continue
                p = urllib.parse.urlparse(href).path
                if re.search(r"\.(png|jpe?g|gif|webp|svg|mp4|webm|zip|pdf|ico|xml|txt|json)$", p, re.I) or p.startswith("/_next/"):
                    continue
                if SKIP_PATHS.match(p):
                    continue
                np_ = self.norm_page(p)
                if np_ not in seen:
                    seen.add(np_)
                    queue.append(np_)
            print(f"page {len(self.pages)}: {path}", flush=True)

    def save_asset(self, path: str) -> bool:
        """Download an internal asset to the mirror; CSS gets its url(...) followed too.
        Videos and archives stay on the original server (False: link to it directly)."""
        path = urllib.parse.urlparse(path).path
        if re.search(r"\.(mp4|webm|mov|m4v|zip|dmg|exe|msi)$", path, re.I):
            return False
        if path in self.assets:
            return True
        # a page or folder (e.g. /apps or /apps/) is no file; leave such links to the page logic
        if path.endswith("/") or not os.path.splitext(path)[1]:
            return False
        target = os.path.join(self.out, path.lstrip("/"))
        if os.path.isdir(target):
            return False
        got = fetch(self.origin + urllib.parse.quote(path))
        if not got:
            return False
        data, ctype = got
        if "text/html" in ctype:
            return False
        self.assets.add(path)
        os.makedirs(os.path.dirname(target), exist_ok=True)
        if path.endswith(".css") or "text/css" in ctype:
            css = data.decode("utf-8", "replace")
            css = self.fix_css(css, path)
            data = css.encode("utf-8")
        with open(target, "wb") as fh:
            fh.write(data)
        return True

    def fix_css(self, css: str, css_path: str) -> str:
        def repl(m):
            raw = m.group(2).strip()
            if raw.startswith("data:") or raw.startswith("#"):
                return m.group(0)
            absu = urllib.parse.urljoin(self.origin + css_path, raw)
            if not self.is_internal(absu):
                return m.group(0)
            p = urllib.parse.urlparse(absu).path
            self.save_asset(p)
            return f"url({m.group(1)}{self.local(p)}{m.group(1)})"
        return re.sub(r"url\((['\"]?)([^)'\"]+)\1\)", repl, css)

    # ---- links to this repo's builds ----
    def map_download(self, url: str) -> str | None:
        m = re.match(r"https://github\.com/" + UPSTREAM + r"/([\w.-]+)/releases(?:/(download|tag)/([^/]+)(?:/([^/?#]+))?)?/?$", url)
        if not m:
            return None
        repo, kind, tag, name = m.groups()
        app_id = "artcraft-launcher" if repo == "craft-launcher" else repo.lower()
        st = self.state.get(app_id)
        if not st:
            return None  # not one of ours (e.g. the AI studio): leave it as it is
        if kind == "download" and name:
            # same file kind in our version: wordcraft-0.5.0-windows-x64.msi -> wordcraft-<ours>-windows-x64.msi
            up_ver = tag.lstrip("vV")
            want = name.replace(up_ver, st["version"])
            for a in st["assets"]:
                if a["name"] == want:
                    return a["download_url"]
            return f"{REPO_URL}/releases/tag/{st['tag']}"
        if app_id == "artcraft-launcher":
            return f"{REPO_URL}/releases/tag/suite"
        return f"{REPO_URL}/releases/tag/{st['tag']}"

    def map_github(self, url: str) -> str:
        m = re.match(r"https://github\.com/" + UPSTREAM + r"/([\w.-]+)(/.*)?$", url)
        if not m:
            return url if url.rstrip("/") != ORIGINAL_GITHUB else REPO_URL
        repo, rest = m.group(1), m.group(2) or ""
        mapped = self.map_download(url)
        if mapped:
            return mapped
        app_id = repo.lower()
        if rest.startswith("/releases"):
            return f"{REPO_URL}/releases/tag/suite"
        if rest.startswith("/issues"):
            return f"{REPO_URL}/issues"
        if app_id in self.state and app_id != "artcraft-launcher":
            return f"{REPO_URL}/tree/main/apps/{app_id}"
        return REPO_URL

    # ---- the page transform ----
    def transform(self, path: str, html: str, de: dict) -> str:
        soup = BeautifulSoup(html, "html.parser")

        # scripts and the hydration payload go; the page stays as rendered
        for s in soup.find_all(["script", "noscript"]):
            s.decompose()
        for l in soup.find_all("link"):
            rel = " ".join(l.get("rel", []))
            if any(k in rel for k in ("preload", "modulepreload", "prefetch", "dns-prefetch", "preconnect")) and l.get("as") != "font" and "stylesheet" not in rel:
                l.decompose()
        for c in soup.find_all(string=lambda t: isinstance(t, Comment)):
            c.extract()

        # Discord-only cards and sections: the smallest block holding a Discord heading and its button
        for h in soup.find_all(re.compile(r"^h[1-6]$")):
            if getattr(h, "decomposed", False):
                continue
            if re.search(r"discord|talk to a human", h.get_text(" "), re.I):
                node = h
                while node.parent is not None and node.parent.name not in ("body", "main", "html"):
                    if node.find("a", href=SOCIAL) or node.find("a", href=re.compile("discord", re.I)):
                        break
                    node = node.parent
                if node.find("a", href=SOCIAL):
                    node.decompose()

        remove_studio(soup, path)
        fix_nav(soup)
        if path.rstrip("/") == "/download":
            self.launcher_download_page(soup)

        header = soup.find("header") or soup.find("nav")
        original_btn_done = False

        for a in list(soup.find_all("a")):
            if a.parent is None:
                continue
            if a.get("data-artcraft"):
                continue  # links this script put in
            href = (a.get("href") or "").strip()
            text = a.get_text(" ", strip=True)
            absu = urllib.parse.urljoin(self.origin + path, href) if href else ""
            # top-right "Launch App" becomes "Original on GitHub" with the same look
            if LAUNCH_APP.match(text):
                if header is not None and a in header.find_all("a") and not original_btn_done:
                    replace_text(a, "Download")
                    a["href"] = self.local("/download/")
                    a["data-artcraft"] = "download"
                    original_btn_done = True
                else:
                    remove_link(a)
                continue
            if SOCIAL.search(absu) or text.strip().lower() in SHARE_LABELS:
                remove_link(a)
                continue
            if self.is_internal(absu):
                p = urllib.parse.urlparse(absu)
                if SKIP_PATHS.match(p.path):
                    remove_link(a)
                    continue
                if href.startswith("#"):
                    continue
                if re.search(r"\.\w{2,5}$", p.path) and not p.path.endswith(".html"):
                    a["href"] = self.local(p.path) if self.save_asset(p.path) else absu
                else:
                    a["href"] = self.local(self.norm_page(p.path)) + (("#" + p.fragment) if p.fragment else "")
                continue
            if absu.startswith("https://github.com/"):
                a["href"] = self.map_github(absu)
                continue
            if "app.getartcraft.com" in absu or absu.startswith("mailto:"):
                remove_link(a)
                continue

        # share widgets: buttons without a link
        for b in list(soup.find_all("button")):
            if b.parent is not None and b.get_text(" ", strip=True).lower() in SHARE_LABELS:
                remove_link(b)

        # assets: styles, images, media, icons
        for tag, attr in (("link", "href"), ("img", "src"), ("source", "src"), ("video", "src"), ("video", "poster"), ("image", "href"), ("meta", "content")):
            for el in soup.find_all(tag):
                val = el.get(attr)
                if not val or (tag == "meta" and not str(val).startswith("/")):
                    continue
                el[attr] = self.asset_url(val, path)
        for el in soup.find_all(["img", "source"]):
            if el.get("srcset"):
                parts = []
                for part in el["srcset"].split(","):
                    bits = part.strip().split()
                    if bits:
                        parts.append(" ".join([self.asset_url(bits[0], path)] + bits[1:]))
                el["srcset"] = ", ".join(parts)
        for el in soup.find_all(style=True):
            el["style"] = self.fix_css(el["style"], path)
        for st in soup.find_all("style"):
            if st.string:
                st.string = self.fix_css(st.string, path)

        # page descriptions that still mention Discord
        for m in soup.find_all("meta", content=re.compile("discord", re.I)):
            m["content"] = re.sub(r",?\s*(join )?(our )?discord( community)?,?", "", m["content"], flags=re.I).replace(" , ", ", ").replace(",  or", " or")

        # text that still sends people to Discord
        for t in list(soup.find_all(string=re.compile(r"discord|\(\d{3}\)\s*\d{3}-\d{4}", re.I))):
            new = re.sub(r"[^.!?]*(discord|\(\d{3}\)\s*\d{3}-\d{4})[^.!?]*[.!?]?", "", str(t), flags=re.I).strip()
            if new:
                t.replace_with(new)
            else:
                parent = t.parent
                t.extract()
                if parent is not None and not parent.get_text(strip=True) and parent.name not in ("body", "main"):
                    parent.decompose()

        add_credits(soup)
        renumber_labels(soup)
        html_tag = soup.find("html")
        if html_tag is not None:
            html_tag["lang"] = "en"
        out = str(soup)
        # leftover absolute links of the original site in inline text/attributes
        out = re.sub(r"https://github\.com/" + UPSTREAM + r"/craft-launcher/releases[^\"'\s<]*", f"{REPO_URL}/releases/tag/suite", out)
        return out

    def asset_url(self, val: str, page: str) -> str:
        val = val.strip()
        if val.startswith("data:"):
            return val
        absu = urllib.parse.urljoin(self.origin + page, val)
        if not self.is_internal(absu):
            return val
        p = urllib.parse.urlparse(absu)
        if p.path == "/_next/image":
            inner = urllib.parse.parse_qs(p.query).get("url", [""])[0]
            if inner:
                inner_abs = urllib.parse.urljoin(self.origin + "/", inner)
                if not self.is_internal(inner_abs):
                    return inner_abs
                ip = urllib.parse.urlparse(inner_abs).path
                return self.local(ip) if self.save_asset(ip) else inner_abs
            return val
        if self.save_asset(p.path):
            return self.local(p.path)
        page_path = self.norm_page(p.path)
        if page_path in self.pages:
            return self.local(page_path)
        return absu

    def launcher_file(self, pattern: str) -> str:
        st = self.state.get("artcraft-launcher") or {}
        for a in st.get("assets", []):
            if re.search(pattern, a["name"], re.I):
                return a["download_url"]
        return f"{REPO_URL}/releases/tag/suite"

    def launcher_download_page(self, soup):
        """The original page offers the AI studio; ours offers ArtCraft Launcher, same layout."""
        st = self.state.get("artcraft-launcher") or {}
        win = self.launcher_file(r"x64.*\.msi$|\.msi$")
        lin = self.launcher_file(r"\.AppImage$")
        main = soup.find("main") or soup.body
        if main is None:
            return
        sections = main.find_all(recursive=False)
        for sec in sections:
            label = section_label(sec)
            if label == "download":
                h1 = sec.find("h1")
                if h1:
                    em = h1.find("span")
                    for t in [x for x in h1.find_all(string=True) if x.strip()]:
                        if em is None or t.parent is not em:
                            t.replace_with(re.sub(r"ArtCraft Studio|Studio", "ArtCraft ", str(t)))
                    if em is not None:
                        em.string = "Launcher"
                    for t in h1.find_all(string=True):
                        if "ArtCraft" in t and "  " in t:
                            t.replace_with(str(t).replace("  ", " "))
                set_text(sec.find("p", class_="text-lg"), "One installer for every Crafting App. It installs the apps you pick and keeps them all up to date.")
                for p in sec.find_all("p", class_="hud-label"):
                    if "Studio" in p.get_text():
                        set_text(p, "ArtCraft Launcher · Windows · Linux")
                    elif re.search(r"v\d", p.get_text()):
                        set_text(p, "Free · Open source" + (f" · v{st['version']}" if st.get("version") else ""))
                for a in sec.find_all("a"):
                    t = a.get_text(" ", strip=True)
                    if re.search(r"windows", t, re.I):
                        a["href"] = win
                        a["data-artcraft"] = "download-win"
                    elif re.search(r"use on web", t, re.I):
                        replace_text(a, "Download for Linux")
                        a["href"] = lin
                        a["data-artcraft"] = "download-linux"
            elif label == "platforms":
                for p in sec.find_all("p", class_="hud-label"):
                    if "macOS" in p.get_text():
                        set_text(p, "x64 Windows · x64 Linux")
                for art in sec.find_all("article"):
                    h2 = art.find("h2")
                    name = h2.get_text(strip=True) if h2 else ""
                    items = art.find_all("li")
                    if name == "macOS":
                        set_text(h2, "Linux")
                        if items:
                            set_text(items[0], "AppImage, deb, rpm or tar (x86_64)")
                        apple = art.find("svg", class_=re.compile("apple"))
                        mon = sec.find("svg", class_=re.compile("monitor"))
                        if apple is not None and mon is not None:
                            apple.replace_with(BeautifulSoup(str(mon), "html.parser"))
                    for li, text in zip(items[1:], ["Installs and updates every Craft App", "Free, no account needed"]):
                        set_text(li, text)
                    for a in art.find_all("a"):
                        if name == "macOS":
                            replace_text(a, "Download for Linux")
                            a["href"] = lin
                        else:
                            a["href"] = win
                        a["data-artcraft"] = "download"
            elif label == "quick start":
                steps = sec.find_all("li")
                texts = [("Download", "Grab the installer for your platform."),
                         ("Install", "Run the installer. No account, no sign-in."),
                         ("Pick your apps", "The launcher installs the Craft Apps you choose and keeps them up to date.")]
                for li, (h, t) in zip(steps, texts):
                    set_text(li.find("h3"), h)
                    set_text(li.find("p", class_=re.compile("text-muted")), t)
            elif sec.find("figure") and "Viewport" in sec.get_text():
                sec.decompose()

    def render(self) -> dict:
        """Every page after the debrand transform (without the language script yet)."""
        out = {path: self.transform(path, html, {}) for path, html in self.pages.items()}
        if "/" in out:
            out["/404"] = out["/"]
        return out

    def write(self, rendered: dict, de: dict):
        script = "<script>" + RUNTIME_JS.replace("__DE__", json.dumps(de, ensure_ascii=False).replace("</", "<\\/")) + "</script>"
        for path, html in rendered.items():
            target = os.path.join(self.out, "404.html") if path == "/404" else os.path.join(self.out, path.lstrip("/"), "index.html")
            os.makedirs(os.path.dirname(target), exist_ok=True)
            html = html.replace("</body>", script + "</body>", 1) if "</body>" in html else html + script
            with open(target, "w", encoding="utf-8") as fh:
                fh.write(html)


def set_text(el, text: str):
    if el is not None:
        el.clear()
        el.append(text)


def section_label(sec) -> str:
    """'01 / Crafting Apps' -> 'crafting apps' (the label of a page section)."""
    p = sec.find("p", class_="hud-label")
    if p is None:
        return ""
    m = re.match(r"^\s*\d+\s*/\s*(.+?)\s*$", " ".join(p.get_text(" ").split()))
    return m.group(1).lower() if m else ""


def top_section(el):
    while el.parent is not None and el.parent.name not in ("main", "body"):
        el = el.parent
    return el


STUDIO_SECTIONS = {"artcraft studio", "studio features", "ownership", "proof", "promotional videos"}


def remove_studio(soup, path: str):
    """Everything about the original's AI studio goes; the Craft Apps stay."""
    main = soup.find("main") or soup.body
    if main is None:
        return
    for sec in list(main.find_all(recursive=False)):
        if section_label(sec) in STUDIO_SECTIONS:
            sec.decompose()
    for mq in soup.find_all(class_="marquee"):
        mq.decompose()
    for p in soup.find_all("p"):
        if p.parent is not None and re.search(r"Looking for ArtCraft Studio", p.get_text(" ")):
            p.decompose()
    # "Talk to a human" (press kit)
    for h in soup.find_all(re.compile(r"^h[1-6]$")):
        if h.parent is not None and re.search(r"talk to a human", h.get_text(" "), re.I):
            top_section(h).decompose()
    # cards and footer columns about the studio
    for el in soup.find_all(["article", "nav"]):
        if el.parent is None:
            continue
        first = el.find("p", class_="hud-label")
        if first is not None and first.get_text(" ", strip=True) == "ArtCraft Studio":
            grid = el.parent
            footer_column = el.name == "nav"
            el.decompose()
            if grid is None or not grid.get("class"):
                continue
            if footer_column:
                # one column fewer; the site's CSS only has the classes it used, so say it here
                grid["data-artcraft"] = "footer-grid"
                style = soup.new_tag("style")
                style.string = ("@media (min-width:768px){[data-artcraft=footer-grid]{grid-template-columns:repeat(3,minmax(0,1fr))}"
                                "[data-artcraft=footer-grid]>:first-child{grid-column:span 3/span 3}}"
                                "@media (min-width:1024px){[data-artcraft=footer-grid]{grid-template-columns:2fr 1fr 1fr 1fr}"
                                "[data-artcraft=footer-grid]>:first-child{grid-column:span 1/span 1}}")
                (soup.head or soup).append(style)
            else:
                grid["class"] = [c for c in grid["class"] if not re.match(r"^md:grid-cols-\d$", c)]
    for p in soup.find_all("p"):
        t = " ".join(p.get_text(" ").split())
        if t.startswith("ArtCraft makes two things"):
            set_text(p, "Thirteen native, open-source desktop apps. ArtCraft Launcher installs them all and keeps them up to date.")
        elif t.startswith("Open-source tools for artists: the Craft Apps and ArtCraft Studio"):
            set_text(p, "Open-source tools for artists: the Craft Apps.")
        elif t.startswith("Find answers, join the community"):
            set_text(p, "Report a problem or ask a question on GitHub.")
        elif "the world's only open source precision AI tool for artists" in t:
            set_text(p, t.replace("the world's only open source precision AI tool for artists", "the Crafting Apps"))
    # support: the mail card becomes the GitHub issues card
    for a in soup.find_all("a", href=re.compile(r"^mailto:")):
        h2 = a.find("h2")
        if h2 is not None:
            set_text(h2, "Report a problem")
            mail = a.find("p", class_=re.compile("text-muted"))
            set_text(mail, "Open an issue on GitHub.")
            for sp in a.find_all("span", class_="hud-label"):
                replace_text(sp, "Open an issue")
            a["href"] = f"{REPO_URL}/issues"
            a["target"] = "_blank"
            a["rel"] = "noopener"
            a["data-artcraft"] = "issues"
    # copyright
    for t in soup.find_all(string=re.compile(r"©\s*\d{4}\s*ArtCraft")):
        t.replace_with(re.sub(r"©\s*(\d{4})\s*ArtCraft", r"© \1 LabtosoStudios", str(t)))


def fix_nav(soup):
    """Top bar: no web-editor items, Resources becomes Press Kit, Original on GitHub on the left."""
    nav = soup.find("nav")
    if nav is None:
        return
    ul = nav.find("ul")
    if ul is None:
        return
    # a plain (not highlighted) entry to copy the look from
    model = None
    for li in ul.find_all("li", recursive=False):
        a, sp = li.find("a"), li.find("span")
        if a is None or li.find("button") is not None or "app.getartcraft.com" in (a.get("href") or ""):
            continue
        if sp is not None and "bg-invert-bg" in (sp.get("class") or []):
            continue
        if "border-l" in (li.get("class") or []):
            continue
        model = li
        break
    if model is None:
        return
    for li in list(ul.find_all("li", recursive=False)):
        if li.find("button") is not None and "Resources" in li.get_text():
            new = BeautifulSoup(str(model), "html.parser").li
            a = new.find("a")
            a["href"] = "/press-kit/"
            replace_text(a.find("span") or a, "Press Kit")
            li.replace_with(new)
    if not ul.find("a", attrs={"data-artcraft": "original"}):
        new = BeautifulSoup(str(model), "html.parser").li
        new["class"] = (new.get("class") or []) + ["ml-1.5", "border-l", "border-line", "pl-1.5"]
        a = new.find("a")
        a["href"] = ORIGINAL_GITHUB
        a["target"] = "_blank"
        a["rel"] = "noopener"
        a["data-artcraft"] = "original"
        replace_text(a.find("span") or a, "Original on GitHub")
        ul.append(new)


def renumber_labels(soup):
    """Section labels count 01, 02, ... again after sections were taken out."""
    n = 0
    for p in soup.find_all("p", class_="hud-label"):
        spans = p.find_all("span", recursive=False)
        if len(spans) >= 2 and re.fullmatch(r"\d{2}", spans[0].get_text(strip=True)) and spans[1].get_text(strip=True) == "/":
            n += 1
            spans[0].string = f"{n:02d}"


def replace_text(el, text: str):
    """Swap an element's visible text, keeping icons (svg/img) it holds."""
    strings = [s for s in el.find_all(string=True) if s.strip()]
    if strings:
        strings[0].replace_with(text)
        for s in strings[1:]:
            s.extract()
    else:
        el.append(text)


def remove_link(el):
    parent = el.parent
    el.decompose()
    # an empty list item / wrapper goes with it
    while parent is not None and parent.name in ("li", "span", "div") and not parent.get_text(strip=True) and not parent.find(["img", "svg", "video", "a", "button", "input"]):
        up = parent.parent
        parent.decompose()
        parent = up


def add_credits(soup):
    footer = soup.find("footer") or soup.find("body")
    if footer is None:
        return
    block = BeautifulSoup(
        f"""
<div data-artcraft="credits" style="margin:2.5rem auto 0;padding:1.25rem 1rem;max-width:72rem;border-top:1px solid rgba(127,127,127,.25);font-size:.875rem;line-height:1.6;opacity:.85">
  <p><strong>Credits.</strong> ArtCraft and every Craft app were created by the ArtCraft team and the contributors of the
  <a href="{ORIGINAL_GITHUB}" target="_blank" rel="noopener" style="text-decoration:underline">original project on GitHub</a>.
  All credit for the apps, their design and this website goes to them.</p>
  <p>This is a fork: the same apps without community links, built automatically from every original release. Source, builds and how it works:
  <a href="{REPO_URL}" target="_blank" rel="noopener" style="text-decoration:underline">github.com/{REPO}</a>.
  The apps are licensed under MIT or Apache-2.0; ArtCraft's name and logos are the ArtCraft team's trademarks.</p>
</div>""",
        "html.parser",
    )
    footer.append(block)


RUNTIME_JS = r"""
(function () {
  var DE = __DE__;
  var KEY = 'artcraft-lang';
  function pref() {
    try { var s = localStorage.getItem(KEY); if (s) return s; } catch (e) {}
    var langs = navigator.languages || [navigator.language || 'en'];
    return /^de\b/i.test(langs[0] || '') ? 'de' : 'en';
  }
  var originals = new Map();
  function norm(s) { return s.replace(/\s+/g, ' ').trim(); }
  var BLOCKS = 'p,h1,h2,h3,h4,h5,h6,li,a,button,figcaption,dt,dd,label,td,th,span,strong';
  var INLINE = /^(SPAN|A|EM|STRONG|B|I|CODE|SMALL|MARK|U)$/;
  var SKIP_IN = /^(svg|path|rect|line|circle|polyline|polygon|g|img)$/i;
  function textNodes(el) {
    var out = [], w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, null), n;
    while ((n = w.nextNode())) if (n.nodeValue.trim()) out.push(n);
    return out;
  }
  // 'One [[craft]].' for text mixed with simple inline elements (same rule as the build script)
  function blockKey(el) {
    var parts = [], slots = [], inline = 0, plain = false;
    for (var c = el.firstChild; c; c = c.nextSibling) {
      if (c.nodeType === 3) { if (c.nodeValue.trim()) plain = true; parts.push(originals.has(c) ? originals.get(c) : c.nodeValue); slots.push({ text: c }); continue; }
      if (c.nodeType !== 1) continue;
      if (/^(svg|img|br)$/i.test(c.tagName)) continue;
      if (!INLINE.test(c.tagName)) return null;
      var tn = textNodes(c);
      if (tn.length !== 1) return null;
      var bad = false;
      c.querySelectorAll('*').forEach(function (e) { if (!SKIP_IN.test(e.tagName)) bad = true; });
      if (bad) return null;
      parts.push('[[' + norm(originals.has(tn[0]) ? originals.get(tn[0]) : tn[0].nodeValue) + ']]');
      slots.push({ inline: tn[0] });
      inline++;
    }
    if (!plain || !inline) return null;
    return { key: norm(parts.join('')), slots: slots };
  }
  function remember(n) { if (!originals.has(n)) originals.set(n, n.nodeValue); }
  function blocks(done) {
    document.querySelectorAll(BLOCKS).forEach(function (el) {
      var b = blockKey(el);
      if (!b || !DE[b.key]) return;
      var pieces = DE[b.key].split(/\[\[([\s\S]*?)\]\]/);
      var inlineCount = b.slots.filter(function (s) { return s.inline; }).length;
      if ((pieces.length - 1) / 2 !== inlineCount) return;
      var i = 0, firstText = true;
      b.slots.forEach(function (s) {
        if (s.inline) {
          remember(s.inline);
          var o = originals.get(s.inline);
          s.inline.nodeValue = o.match(/^\s*/)[0] + pieces[i + 1].trim() + o.match(/\s*$/)[0];
          done.add(s.inline);
          i += 2; firstText = true;
        } else {
          remember(s.text);
          var orig = originals.get(s.text);
          if (firstText) {
            var piece = pieces[i];
            var lead = /^\s/.test(orig) || (i > 0 && /^\s/.test(piece)) ? ' ' : '';
            var trail = /\s$/.test(orig) || (i + 1 < pieces.length && /\s$/.test(piece)) ? ' ' : '';
            s.text.nodeValue = lead + piece.trim() + trail;
            firstText = false;
          } else {
            s.text.nodeValue = '';
          }
          done.add(s.text);
        }
      });
    });
  }
  function translate(lang) {
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
    var n, all = [];
    while ((n = walker.nextNode())) { remember(n); n.nodeValue = originals.get(n); all.push(n); }
    var done = new Set();
    if (lang === 'de') blocks(done);
    all.forEach(function (n) {
      if (done.has(n)) return;
      var orig = originals.get(n), key = norm(orig);
      if (!key) return;
      if (lang === 'de' && DE[key]) {
        var lead = orig.match(/^\s*/)[0], trail = orig.match(/\s*$/)[0];
        n.nodeValue = lead + DE[key] + trail;
      }
    });
    ['placeholder', 'title', 'aria-label', 'alt'].forEach(function (attr) {
      document.querySelectorAll('[' + attr + ']').forEach(function (el) {
        var o = el.getAttribute('data-en-' + attr);
        if (o === null) { o = el.getAttribute(attr); el.setAttribute('data-en-' + attr, o); }
        el.setAttribute(attr, lang === 'de' && DE[norm(o)] ? DE[norm(o)] : o);
      });
    });
    if (!document.documentElement.getAttribute('data-en-title')) document.documentElement.setAttribute('data-en-title', document.title);
    var t = document.documentElement.getAttribute('data-en-title');
    document.title = lang === 'de' && DE[norm(t)] ? DE[norm(t)] : t;
    document.documentElement.lang = lang;
    var sw = document.querySelectorAll('[data-artcraft-lang]');
    sw.forEach(function (b) { b.setAttribute('aria-pressed', b.getAttribute('data-artcraft-lang') === lang ? 'true' : 'false'); b.style.opacity = b.getAttribute('data-artcraft-lang') === lang ? '1' : '.55'; });
  }
  function switcher() {
    var host = document.querySelector('[data-artcraft="download"]') || document.querySelector('[data-artcraft="original"]');
    var box = document.createElement('span');
    box.setAttribute('data-artcraft', 'lang');
    box.style.cssText = 'display:inline-flex;gap:.25rem;align-items:center;margin:0 .5rem;font-size:.8rem;font-weight:600';
    ['de', 'en'].forEach(function (l) {
      var b = document.createElement('button');
      b.type = 'button'; b.textContent = l.toUpperCase(); b.setAttribute('data-artcraft-lang', l);
      b.style.cssText = 'background:none;border:0;color:inherit;cursor:pointer;padding:.25rem .3rem';
      b.addEventListener('click', function () { try { localStorage.setItem(KEY, l); } catch (e) {} translate(l); });
      box.appendChild(b);
    });
    if (host && host.parentNode) host.parentNode.insertBefore(box, host);
    else { box.style.cssText += ';position:fixed;right:1rem;bottom:1rem;z-index:99;background:rgba(0,0,0,.6);color:#fff;border-radius:999px;padding:.25rem .5rem'; document.body.appendChild(box); }
  }
  // menus that used to open with the site's own script
  function menus() {
    document.querySelectorAll('button[aria-expanded], [aria-haspopup]').forEach(function (btn) {
      if (/main menu/i.test(btn.textContent)) return;
      var id = btn.getAttribute('aria-controls');
      var panel = id ? document.getElementById(id) : (btn.nextElementSibling || null);
      if (!panel) return;
      btn.addEventListener('click', function (e) {
        e.preventDefault();
        var open = btn.getAttribute('aria-expanded') === 'true';
        btn.setAttribute('aria-expanded', open ? 'false' : 'true');
        panel.hidden = open;
        panel.style.display = open ? 'none' : '';
        panel.classList.toggle('hidden', open);
      });
    });
  }
  // videos that the site's own script started once they scrolled into view
  function videos() {
    var vids = document.querySelectorAll('video');
    if (!vids.length) return;
    var play = function (v) { v.muted = true; v.preload = 'auto'; var p = v.play(); if (p && p.catch) p.catch(function () {}); };
    if (!('IntersectionObserver' in window)) { vids.forEach(play); return; }
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) { if (e.isIntersecting) play(e.target); else if (!e.target.paused) e.target.pause(); });
    }, { rootMargin: '200px' });
    vids.forEach(function (v) { if (v.hasAttribute('controls')) return; v.setAttribute('playsinline', ''); io.observe(v); });
  }
  // light/dark switch (the moon button), remembered like on the original
  var TKEY = 'artcraft-theme';
  function currentTheme() {
    var t = document.documentElement.getAttribute('data-theme');
    if (t) return t;
    return window.matchMedia && matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  try { var saved = localStorage.getItem(TKEY); if (saved) document.documentElement.setAttribute('data-theme', saved); } catch (e) {}
  function theme() {
    document.querySelectorAll('button[aria-label*="theme" i]').forEach(function (b) {
      b.addEventListener('click', function () {
        var next = currentTheme() === 'dark' ? 'light' : 'dark';
        var html = document.documentElement;
        html.classList.add('theme-anim');
        html.setAttribute('data-theme', next);
        try { localStorage.setItem(TKEY, next); } catch (e) {}
        setTimeout(function () { html.classList.remove('theme-anim'); }, 450);
      });
    });
  }
  // the phone menu: the same links as the top bar, in a panel under it
  function phoneMenu() {
    var btn = [].slice.call(document.querySelectorAll('button')).filter(function (b) { return /main menu/i.test(b.textContent); })[0];
    var list = document.querySelector('nav ul');
    var bar = btn && btn.closest('nav, header');
    if (!btn || !list || !bar) return;
    var panel = document.createElement('div');
    panel.setAttribute('data-artcraft', 'phone-menu');
    panel.className = 'border-b border-line bg-bg';
    panel.style.cssText = 'display:none;position:absolute;left:0;right:0;top:100%;z-index:60';
    list.querySelectorAll('a').forEach(function (a) {
      var c = a.cloneNode(true);
      c.className = 'hud-label block px-6 py-4 border-t border-line text-muted hover:text-ink';
      panel.appendChild(c);
    });
    var langs = document.createElement('div');
    langs.className = 'hud-label flex gap-4 px-6 py-4 border-t border-line';
    document.querySelectorAll('[data-artcraft-lang]').forEach(function (b) {
      var c = document.createElement('button'); c.type = 'button'; c.textContent = b.textContent;
      c.addEventListener('click', function () { b.click(); });
      langs.appendChild(c);
    });
    panel.appendChild(langs);
    if (getComputedStyle(bar).position === 'static') bar.style.position = 'relative';
    bar.appendChild(panel);
    btn.addEventListener('click', function () {
      var open = panel.style.display !== 'none';
      panel.style.display = open ? 'none' : 'block';
      btn.setAttribute('aria-expanded', open ? 'false' : 'true');
    });
  }
  document.addEventListener('DOMContentLoaded', function () {
    switcher(); menus(); videos(); theme(); phoneMenu(); translate(pref());
  });
})();
"""


def add_runtime(soup, de: dict):
    body = soup.find("body")
    if body is None:
        return
    script = soup.new_tag("script")
    script.string = RUNTIME_JS.replace("__DE__", json.dumps(de, ensure_ascii=False))
    body.append(script)


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


BLOCKS = ["p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "a", "button", "figcaption", "dt", "dd", "label", "td", "th", "span", "strong"]
INLINE = {"span", "a", "em", "strong", "b", "i", "code", "small", "mark", "u"}


def norm_text(t: str) -> str:
    return re.sub(r"\s+", " ", t).strip()


def block_key(el) -> str:
    """'One [[craft]].' for an element mixing text with simple inline elements, else ''.
    The page script builds the same key in the browser."""
    parts, inline, plain = [], 0, False
    for c in el.children:
        if isinstance(c, Comment):
            continue
        if isinstance(c, str):
            if c.strip():
                plain = True
            parts.append(str(c))
            continue
        if c.name in ("svg", "img", "br"):
            continue
        if c.name not in INLINE:
            return ""
        strings = [x for x in c.find_all(string=True) if x.strip() and not isinstance(x, Comment)]
        if len(strings) != 1 or c.find(lambda e: e.name not in ("svg", "path", "rect", "line", "circle", "polyline", "polygon", "g", "img")):
            return ""
        parts.append("[[" + norm_text(strings[0]) + "]]")
        inline += 1
    if not (plain and inline):
        return ""
    return norm_text("".join(parts))


# names that stay as they are in German
KEEP_NAMES = re.compile(r"\b(ArtCraft Launcher|ArtCraft Studio|ArtCraft|Crafting Apps?|Craft Apps?|[A-Z][A-Za-z]*Craft|LabtosoStudios|GitHub|Rust|WebAssembly|macOS|Windows|Linux)\b")


def fix_names(de_text: str) -> str:
    """Undo what translation does to names: 'Crafting-Apps' -> 'Crafting Apps', 'Pdf Craft' -> 'PdfCraft'."""
    t = re.sub(r"\b(Crafting|Craft)-(Apps?)\b", r"\1 \2", de_text)
    t = re.sub(r"\b(Photo|Vector|Film|Light|Pdf|Effect|Design|Fig|Word|Sound|Deck|CAD|Grid|Art)\s+Craft\b", r"\1Craft", t)
    return t


AUTO = os.path.join(ROOT, "site", "i18n", "de.auto.json")
KEEP_ENGLISH = re.compile(r"^(ArtCraft|[A-Z][a-z]+Craft|CADCraft|PdfCraft|macOS|Windows|Linux|Web|GitHub|Rust|MCP|v?\d[\d.]*|[\w.+-]+@[\w.-]+|https?://\S+|©.*)$")


GTX_ERRORS = []


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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--origin", default="https://getartcraft.com")
    ap.add_argument("--out", default=os.path.join(ROOT, "_site"))
    ap.add_argument("--snapshot", help="also save the original pages here (for the German texts)")
    args = ap.parse_args()

    with open(os.path.join(ROOT, "site", "i18n", "de.json"), encoding="utf-8") as fh:
        de = {re.sub(r"\s+", " ", k).strip(): v for k, v in json.load(fh).items() if not k.startswith("_")}

    os.makedirs(args.out, exist_ok=True)
    m = Mirror(args.origin, args.out)
    m.crawl(["/", "/apps", "/download", "/support", "/press-kit"])
    if not m.pages:
        print("original site unreachable; keeping the previous mirror", file=sys.stderr)
        return 1

    # every visible text of the finished pages; whatever has no hand-made German gets a
    # machine translation (cached). Sentences with an italic word or a link inside also go as a
    # whole, the inline parts marked [[like this]], so the German keeps its words in order.
    rendered = m.render()
    texts = set()
    for path, html in rendered.items():
        soup = BeautifulSoup(html, "html.parser")
        for x in soup(["script", "style", "noscript", "svg"]):
            x.decompose()
        for t in soup.find_all(string=True):
            if isinstance(t, Comment):
                continue
            k = norm_text(str(t))
            if k and re.search(r"[A-Za-z]{2}", k) and len(k) < 2000:
                texts.add(k)
        for el in soup.find_all(BLOCKS):
            k = block_key(el)
            if k:
                texts.add(k)
        for attr in ("alt", "title", "aria-label", "placeholder"):
            for el in soup.find_all(attrs={attr: True}):
                texts.add(norm_text(el[attr]))
        if soup.title and soup.title.string:
            texts.add(norm_text(soup.title.string))
    texts.discard("")
    auto = translate_missing(sorted(t for t in texts if t not in de))
    de = {k: fix_names(v) for k, v in {**auto, **de}.items() if k in texts}

    m.write(rendered, de)
    write_feed(args.out, m.state)
    with open(os.path.join(args.out, ".nojekyll"), "w") as fh:
        fh.write("")
    if args.snapshot:
        os.makedirs(args.snapshot, exist_ok=True)
        for path, html in m.pages.items():
            name = (path.strip("/") or "home").replace("/", "__")
            with open(os.path.join(args.snapshot, name + ".html"), "w", encoding="utf-8") as fh:
                fh.write(html)
    print(f"{len(m.pages)} pages, {len(m.assets)} files")
    print(f"::notice title=Website::{len(m.pages)} pages, {len(m.assets)} files, {len(de)} German texts")
    return 0


if __name__ == "__main__":
    sys.exit(main())
