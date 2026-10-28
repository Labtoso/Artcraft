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
SKIP_PATHS = re.compile(r"^/pricing(/|$)", re.I)
LAUNCH_APP = re.compile(r"^\s*launch app\s*$", re.I)
SHARE_LABELS = {"x", "reddit", "bluesky", "linkedin", "copy link", "share", "facebook"}
MAX_ASSET = 90 * 1024 * 1024


def fetch(url: str) -> tuple[bytes, str] | None:
    for attempt in range(3):
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
        time.sleep(2 * (attempt + 1))
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
        target = os.path.join(self.out, path.lstrip("/"))
        got = fetch(self.origin + urllib.parse.quote(path))
        if not got:
            return False
        self.assets.add(path)
        data, ctype = got
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

        header = soup.find("header") or soup.find("nav")
        original_btn_done = False

        for a in list(soup.find_all("a")):
            if a.parent is None:
                continue
            href = (a.get("href") or "").strip()
            text = a.get_text(" ", strip=True)
            absu = urllib.parse.urljoin(self.origin + path, href) if href else ""
            # top-right "Launch App" becomes "Original on GitHub" with the same look
            if LAUNCH_APP.match(text):
                if header is not None and a in header.find_all("a") and not original_btn_done:
                    replace_text(a, "Original on GitHub")
                    a["href"] = ORIGINAL_GITHUB
                    a["target"] = "_blank"
                    a["rel"] = "noopener"
                    a["data-artcraft"] = "original"
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
                    self.save_asset(p.path)
                    a["href"] = self.local(p.path)
                else:
                    a["href"] = self.local(self.norm_page(p.path)) + (("#" + p.fragment) if p.fragment else "")
                continue
            if absu.startswith("https://github.com/"):
                a["href"] = self.map_github(absu)
                continue
            if "app.getartcraft.com" in absu:
                continue  # the AI studio's own web app stays as it is

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
        add_runtime(soup, de)
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
        return absu

    def write(self, de: dict):
        for path, html in self.pages.items():
            target = os.path.join(self.out, path.lstrip("/"), "index.html")
            os.makedirs(os.path.dirname(target), exist_ok=True)
            with open(target, "w", encoding="utf-8") as fh:
                fh.write(self.transform(path, html, de))
        # 404 page: the home page
        if "/" in self.pages:
            with open(os.path.join(self.out, "404.html"), "w", encoding="utf-8") as fh:
                fh.write(self.transform("/", self.pages["/"], de))


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
  function translate(lang) {
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
    var n;
    while ((n = walker.nextNode())) {
      if (!originals.has(n)) originals.set(n, n.nodeValue);
      var orig = originals.get(n), key = norm(orig);
      if (!key) continue;
      if (lang === 'de' && DE[key]) {
        var lead = orig.match(/^\s*/)[0], trail = orig.match(/\s*$/)[0];
        n.nodeValue = lead + DE[key] + trail;
      } else {
        n.nodeValue = orig;
      }
    }
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
    var host = document.querySelector('[data-artcraft="original"]');
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
  document.addEventListener('DOMContentLoaded', function () {
    switcher(); menus(); translate(pref());
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


AUTO = os.path.join(ROOT, "site", "i18n", "de.auto.json")
KEEP_ENGLISH = re.compile(r"^(ArtCraft|[A-Z][a-z]+Craft|CADCraft|PdfCraft|macOS|Windows|Linux|Web|GitHub|Rust|MCP|v?\d[\d.]*|[\w.+-]+@[\w.-]+|https?://\S+|©.*)$")


def translate_missing(missing: list[str]) -> dict:
    """German for texts nobody translated by hand yet; cached in site/i18n/de.auto.json."""
    cache = {}
    if os.path.exists(AUTO):
        with open(AUTO, encoding="utf-8") as fh:
            cache = json.load(fh)
    todo = [t for t in missing if t not in cache and not KEEP_ENGLISH.match(t)]
    for i, text in enumerate(todo):
        q = urllib.parse.urlencode({"client": "gtx", "sl": "en", "tl": "de", "dt": "t", "q": text})
        got = fetch("https://translate.googleapis.com/translate_a/single?" + q)
        if not got:
            continue
        try:
            data = json.loads(got[0].decode("utf-8"))
            de_text = "".join(part[0] for part in data[0] if part and part[0])
        except (ValueError, IndexError, TypeError):
            continue
        # product names stay as they are
        for name in re.findall(r"\b(?:ArtCraft|[A-Z][a-z]+Craft|CADCraft|PdfCraft)\b", text):
            de_text = re.sub(re.escape(name), name, de_text, flags=re.I)
        cache[text] = de_text
        if i % 25 == 0:
            time.sleep(1)
    os.makedirs(os.path.dirname(AUTO), exist_ok=True)
    with open(AUTO, "w", encoding="utf-8") as fh:
        json.dump(dict(sorted(cache.items())), fh, indent=1, ensure_ascii=False)
        fh.write("\n")
    print(f"German: {len(todo)} new machine translations, {len(cache)} cached")
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
    m.crawl(["/", "/apps", "/download", "/support", "/tutorials", "/news", "/faq", "/press-kit"])
    if not m.pages:
        print("original site unreachable; keeping the previous mirror", file=sys.stderr)
        return 1

    # every visible text; whatever has no hand-made German gets a machine translation (cached)
    texts = set()
    for path, html in m.pages.items():
        soup = BeautifulSoup(html, "html.parser")
        for s in soup(["script", "style", "noscript", "svg"]):
            s.decompose()
        for t in soup.find_all(string=True):
            if isinstance(t, Comment):
                continue
            k = re.sub(r"\s+", " ", str(t)).strip()
            if k and re.search(r"[A-Za-z]{2}", k) and len(k) < 2000:
                texts.add(k)
        for el in soup.find_all(attrs={"alt": True}):
            texts.add(re.sub(r"\s+", " ", el["alt"]).strip())
        if soup.title and soup.title.string:
            texts.add(re.sub(r"\s+", " ", soup.title.string).strip())
    texts.discard("")
    auto = translate_missing(sorted(t for t in texts if t not in de))
    de = {**auto, **de}

    m.write(de)
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
    return 0


if __name__ == "__main__":
    sys.exit(main())
