#!/usr/bin/env python3
"""Debrand a Craft app source tree in place.

Two layers:

1. Removal: every piece of UI code that mentions Discord or another social network (a button, a
   menu entry, a link row, a match arm, an element of a link list) is cut out at the level of the
   Rust statement or list element that holds it. Declarations (`let`, `const`, `fn`, ...) are never
   removed, so the code keeps compiling; anything left over is caught by layer 2.
2. Rewrite: links to the original GitHub repo, issues and releases point to this repo, links to
   getartcraft.com point to the mirrored website, and any Discord or social URL that is still
   left points to this repo instead.

Per-app rules (apps/<app>/debrand.toml) add exact replacements and extra removal markers for
spots the generic pass can't see. `--keep FILE` turns removal off for a file (used when a build
check fails), so a broken guess never ships: that file then only gets the rewrites.

Usage: debrand.py SRC_DIR --app NAME [--rules apps/NAME/debrand.toml] [--keep FILE ...]
       [--report report.json]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys

try:
    import tomllib
except ModuleNotFoundError:  # Python < 3.11
    tomllib = None

OWNER_REPO = os.environ.get("ARTCRAFT_REPO", "Labtoso/Artcraft")
SITE = os.environ.get("ARTCRAFT_SITE", "https://artcraft.labtoso.de")
REPO_URL = f"https://github.com/{OWNER_REPO}"

# What counts as branding to remove from the UI.
MARKER = re.compile(
    r"discord|bsky\.app|bluesky|x\.com/get_artcraft|twitter\.com|linkedin|instagram|tiktok|youtube\.com/@|reddit\.com",
    re.IGNORECASE,
)

SKIP_DIRS = {".git", "target", "node_modules", "tests", "benches", "fuzz", "docs", "examples"}
ITEM_KEYWORDS = ("let", "fn", "const", "static", "struct", "enum", "impl", "use", "mod", "pub", "type", "trait", "extern", "macro_rules")
BLOCK_STARTERS = ("if", "for", "while", "loop", "match", "unsafe", "{")


# ---- lexing --------------------------------------------------------------------------------------

def scan(text: str):
    """Mask per char (1 code, 2 string, 0 comment) and matched bracket pairs."""
    n = len(text)
    code = bytearray(b"\x01") * n
    pairs: dict[int, int] = {}
    stack: list[int] = []
    i = 0
    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            j = text.find("\n", i)
            j = n if j < 0 else j
            code[i:j] = bytes(j - i)
            i = j
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            code[i:j] = bytes(j - i)
            i = j
            continue
        m = re.compile(r'(b|c)?r(#*)"').match(text, i)
        if m and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            end = text.find('"' + m.group(2), m.end())
            end = n if end < 0 else end + 1 + len(m.group(2))
            code[i:end] = b"\x02" * (end - i)
            i = end
            continue
        if c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            j = min(j + 1, n)
            code[i:j] = b"\x02" * (j - i)
            i = j
            continue
        if c == "'":
            # char literal ('a', '\n', '\u{1F600}') vs lifetime ('a)
            m = re.compile(r"'(\\u\{[0-9a-fA-F]+\}|\\.|[^\\'])'").match(text, i)
            if m:
                code[i:m.end()] = b"\x02" * (m.end() - i)
                i = m.end()
                continue
        if c in "([{":
            stack.append(i)
        elif c in ")]}" and stack:
            pairs[stack.pop()] = i
        i += 1
    return code, pairs


class Tree:
    def __init__(self, text: str):
        self.text = text
        self.code, self.pairs = scan(text)
        self.opens = sorted(self.pairs)

    def enclosing(self, pos: int):
        """Bracket groups containing pos, innermost first, as (open, close)."""
        out = [(o, c) for o, c in self.pairs.items() if o < pos < c]
        out.sort(key=lambda oc: oc[0], reverse=True)
        return out

    def prev_code_char(self, pos: int):
        j = pos - 1
        while j >= 0 and (self.code[j] == 0 or self.text[j].isspace()):
            j -= 1
        return j

    def next_code_char(self, pos: int):
        j = pos
        n = len(self.text)
        while j < n and (self.code[j] == 0 or self.text[j].isspace()):
            j += 1
        return j

    def split_top(self, start: int, end: int, seps: str):
        """Split text[start:end] at top-level separator chars; yields (a, b, sep_index or None)."""
        depth, a, j = 0, start, start
        while j < end:
            if self.code[j] == 1:
                ch = self.text[j]
                if ch in "([{":
                    j = self.pairs.get(j, j)
                elif ch in seps:
                    yield a, j, j
                    a = j + 1
            j += 1
        if self.text[a:end].strip():
            yield a, end, None


# ---- deciding what to cut -----------------------------------------------------------------------

def is_list_bracket(tree: Tree, o: int) -> bool:
    if tree.text[o] != "[":
        return False
    p = tree.prev_code_char(o)
    if p < 0:
        return True
    ch = tree.text[p]
    if ch in "=(,[!&{:;>" or tree.text[max(0, p - 1):p + 1] == "in":
        return True
    return False


def statements(tree: Tree, o: int, c: int):
    """Top-level statements of the block {o..c}: (start, end_inclusive, terminated)."""
    text = tree.text
    j = o + 1
    start = None
    out = []
    while j < c:
        if tree.code[j] == 0 or text[j].isspace():
            j += 1
            continue
        if start is None:
            start = j
        ch = text[j] if tree.code[j] == 1 else ""
        if ch and ch in "([":
            j = tree.pairs.get(j, j) + 1
            continue
        if ch == "{":
            close = tree.pairs.get(j, j)
            head = re.sub(r"^(#\[[^\]]*\]\s*)+", "", text[start:j].strip())
            k = tree.next_code_char(close + 1)
            nxt = text[k:k + 5] if k < c else ""
            block_stmt = head == "" or re.match(r"(if|for|while|loop|match|unsafe|else)\b", head)
            if block_stmt and not nxt.startswith(("else", ".", "?", ";")):
                out.append((start, close, False))
                start = None
            j = close + 1
            continue
        if ch == ";":
            out.append((start, j, True))
            start = None
        j += 1
    if start is not None:
        out.append((start, c - 1, False))
    return out


def is_match_body(tree: Tree, o: int) -> bool:
    head = tree.text[max(0, o - 300):o]
    stmt = re.split(r"[;{}]", head)[-1]
    return bool(re.search(r"\bmatch\b", stmt))


def match_arms(tree: Tree, o: int, c: int):
    """Arms of the match body {o..c}: (start, end_inclusive)."""
    text = tree.text
    arms = []
    j = o + 1
    start = None
    while j < c:
        if tree.code[j] == 0 or text[j].isspace():
            j += 1
            continue
        if start is None:
            start = j
        ch = text[j] if tree.code[j] == 1 else ""
        if ch and ch in "([{":
            close = tree.pairs.get(j, j)
            # `=> { ... }` arm body ends the arm (comma optional)
            if ch == "{" and text[start:j].rstrip().endswith("=>"):
                k = tree.next_code_char(close + 1)
                end = k if k < c and text[k] == "," else close
                arms.append((start, end))
                start = None
                j = end + 1
                continue
            j = close + 1
            continue
        if ch == ",":
            arms.append((start, j))
            start = None
        j += 1
    if start is not None:
        arms.append((start, c - 1))
    return arms


def find_cut(tree: Tree, pos: int):
    """The span to remove for a marker at pos, or None when nothing safe is found."""
    text = tree.text
    for o, c in tree.enclosing(pos):
        ch = text[o]
        if ch == "[" and is_list_bracket(tree, o):
            for a, b, sep in tree.split_top(o + 1, c, ","):
                if a <= pos < (b if sep is None else b + 1):
                    end = sep if sep is not None else b
                    return ("element", a, end, o)
            return None
        if ch == "{":
            if is_match_body(tree, o):
                for a, b in match_arms(tree, o, c):
                    if a <= pos <= b:
                        arrow = text.find("=>", a, b + 1)
                        if arrow < 0:
                            return None
                        pattern = text[a:arrow]
                        if "|" in pattern and MARKER.search(pattern):
                            return ("alternative", a, arrow, None)
                        if re.match(r"\s*_\s*$", pattern):
                            return None
                        return ("arm", a, b, None)
                return None
            # struct literals and json!({..}) are not blocks: look further out
            p = tree.prev_code_char(o)
            before = text[max(0, o - 80):o]
            if p >= 0 and (text[p] in "(!,:" or (text[p] == "=" and text[p - 1] != ">")):
                continue
            if re.search(r"(?<![\w])[A-Z][\w:]*\s*$", before):
                continue
            stmts = statements(tree, o, c)
            for idx, (a, b, terminated) in enumerate(stmts):
                if a <= pos <= b:
                    stmt = text[a:b + 1]
                    head = re.sub(r"^(#\[[^\]]*\]\s*)+", "", stmt)
                    word = re.match(r"[A-Za-z_]+|\{", head)
                    word = word.group(0) if word else ""
                    if word == "let" and terminated:
                        # a `let` goes too when nothing but other branding uses its variable
                        m = re.match(r"let\s+(mut\s+)?([a-z_][a-z0-9_]*)\s*(:|=)", head)
                        if not m or m.group(2) == "_":
                            return ("statement", a, b, None) if m else None
                        name = re.compile(r"\b" + m.group(2) + r"\b")
                        users = []
                        for a2, b2, _t in stmts[idx + 1:]:
                            later = text[a2:b2 + 1]
                            # a nested `let <same name>` shadows it from there on
                            rebind = re.search(r"\blet\s+(mut\s+)?" + m.group(2) + r"\b", later)
                            if rebind:
                                later = later[:rebind.start()]
                            if name.search(later):
                                if not MARKER.search(later):
                                    return None
                                # a user that binds its own variable must be unused elsewhere too
                                um = re.match(r"\s*let\s+(mut\s+)?([a-z_][a-z0-9_]*)\s*(:|=)", later)
                                if um and um.group(2) != m.group(2):
                                    uname = re.compile(r"\b" + um.group(2) + r"\b")
                                    for a3, b3, _t3 in stmts:
                                        if a3 > b2 and uname.search(text[a3:b3 + 1]) and not MARKER.search(text[a3:b3 + 1]):
                                            return None
                                users.append((a2, b2))
                        return ("let", a, b, users)
                    if word in ITEM_KEYWORDS:
                        return None
                    if word in ("if", "for", "while", "loop") or terminated:
                        return ("statement", a, b, None)
                    return None
            return None
    return None


def fixup_array_len(text: str, open_idx: int, removed: int) -> str:
    """`const X: [T; 5] = [ ... ]` keeps its declared length in step with the elements."""
    head = text[max(0, open_idx - 400):open_idx]
    m = re.search(r";\s*(\d+)\s*\]\s*=\s*&?\s*$", head)
    if not m:
        return text
    start = max(0, open_idx - 400) + m.start(1)
    end = max(0, open_idx - 400) + m.end(1)
    return text[:start] + str(max(0, int(text[start:end]) - removed)) + text[end:]


def test_spans(text: str):
    spans = []
    for m in re.finditer(r"#\[cfg\(test\)\]\s*(pub\s+)?mod\s+\w+\s*\{", text):
        o = m.end() - 1
        _, pairs = scan(text[o:])
        close = pairs.get(0)
        if close is not None:
            spans.append((m.start(), o + close))
    return spans


def else_part(tree: Tree, a: int, b: int):
    """Index of the first top-level `else` of the if-statement text[a..b], or None."""
    text = tree.text
    j = a
    while j <= b:
        if tree.code[j] == 1 and text[j] == "{":
            close = tree.pairs.get(j, j)
            k = tree.next_code_char(close + 1)
            if k <= b and text.startswith("else", k):
                return k
            return None
        if tree.code[j] == 1 and text[j] in "([":
            j = tree.pairs.get(j, j)
        j += 1
    return None


def drop_statement(text: str, needle: str):
    """Remove the whole statement that contains `needle` (per-app `drop` rule)."""
    p = text.find(needle)
    if p < 0:
        return text, False
    tree = Tree(text)
    for o, c in tree.enclosing(p):
        if text[o] != "{":
            continue
        for a, b, _t in statements(tree, o, c):
            if a <= p <= b:
                ls = text.rfind("\n", 0, a) + 1
                start = ls if not text[ls:a].strip() else a
                end = b + 1
                le = text.find("\n", end)
                if le >= 0 and not text[end:le].strip():
                    end = le + 1
                return text[:start] + text[end:], True
    return text, False


def remove_branding(text: str, extra_markers: list[str]):
    """Repeatedly cut the innermost removable unit around each marker. Returns (text, cuts)."""
    cuts = []
    pattern = MARKER if not extra_markers else re.compile(MARKER.pattern + "|" + "|".join(re.escape(m) for m in extra_markers), re.IGNORECASE)
    skip_positions: set[int] = set()
    for _ in range(500):
        tree = Tree(text)
        tests = test_spans(text)
        target = None
        for m in pattern.finditer(text):
            p = m.start()
            if p in skip_positions or any(a <= p <= b for a, b in tests):
                continue
            # only markers in code or in string literals count, not in comments
            if tree.code[p] == 0:
                continue
            cut = find_cut(tree, p)
            if cut is None:
                skip_positions.add(p)
                continue
            target = cut
            break
        if target is None:
            break
        kind, a, b, list_open = target
        if kind == "element":
            # take the trailing comma (or the leading one, for a last element)
            end = b + 1 if text[b:b + 1] == "," else b
            start = a
            if text[b:b + 1] != ",":
                k = a - 1
                while k > 0 and text[k].isspace():
                    k -= 1
                if text[k] == ",":
                    start = k
            removed_text = text[start:end]
            text = text[:start] + text[end:]
            text = fixup_array_len(text, list_open, 1)
        elif kind == "alternative":
            pat = text[a:b]
            alts = [x for x in re.split(r"\|", pat)]
            keep = [x for x in alts if not pattern.search(x)]
            new = " | ".join(x.strip() for x in keep) + " "
            removed_text = pat
            text = text[:a] + new + text[b:]
        elif kind == "let":
            removed_text = text[a:b + 1]
            for s, e in sorted([(a, b)] + list_open, reverse=True):
                ls = text.rfind("\n", 0, s) + 1
                start = ls if not text[ls:s].strip() else s
                end = e + 1
                le = text.find("\n", end)
                if le >= 0 and not text[end:le].strip():
                    end = le + 1
                text = text[:start] + text[end:]
        elif kind == "statement" and re.match(r"if\b", text[a:b + 1]) and else_part(Tree(text), a, b) is not None:
            # `if <branding> { .. } else { rest }` keeps the else branch
            e = else_part(Tree(text), a, b)
            removed_text = text[a:e]
            body = re.sub(r"^else\s*", "", text[e:b + 1])
            text = text[:a] + body + text[b + 1:]
        else:
            ls = text.rfind("\n", 0, a) + 1
            start = ls if not text[ls:a].strip() else a
            end = b + 1
            le = text.find("\n", end)
            if le >= 0 and not text[end:le].strip():
                end = le + 1
            removed_text = text[start:end]
            text = text[:start] + text[end:]
        cuts.append({"kind": kind, "code": removed_text.strip()[:300]})
        # positions shift after a cut; recompute skips from scratch
        skip_positions = set()
    return text, cuts


# ---- link rewrites --------------------------------------------------------------------------------

def rewrite_links(text: str, app: str) -> str:
    site_app = f"{SITE}/apps/{app}/"
    rules = [
        # repo pages
        (r"https://github\.com/storytold/" + re.escape(app) + r"/issues[^\s\"')<>`]*", f"{REPO_URL}/issues"),
        (r"https://github\.com/storytold/" + re.escape(app) + r"/releases/tag/[^\s\"')<>`]*", f"{REPO_URL}/releases"),
        (r"https://github\.com/storytold/" + re.escape(app) + r"/releases(?![/\w])", f"{REPO_URL}/releases"),
        (r"https://github\.com/storytold/" + re.escape(app) + r"(?![\w/-])", f"{REPO_URL}/tree/main/apps/{app}"),
        (r"https://github\.com/storytold/\{", f"{REPO_URL}/tree/main/apps/{{"),
        (r"(?<![/\w.])github\.com/storytold/" + re.escape(app) + r"(?![\w/-])", f"github.com/{OWNER_REPO}"),
        # website
        (r"https://getartcraft\.com/apps/" + re.escape(app) + r"/?(?![\w-])", site_app),
        (r"https://getartcraft\.com/apps/\{", f"{SITE}/apps/{{"),
        (r"https://getartcraft\.com/apps/?(?![\w-])", f"{SITE}/apps/"),
        (r"https://getartcraft\.com/?(?![\w/-])", f"{SITE}/"),
        (r"(?<![/\w.])getartcraft\.com/apps/" + re.escape(app), f"{SITE.split('://')[1]}/apps/{app}"),
        (r"(?<![/\w.])getartcraft\.com(?![\w/-])", SITE.split("://")[1]),
        # whatever social link is left
        (r"discord://-/invite/\w+", REPO_URL),
        (r"https://discord\.(gg|com)/[\w/-]+", REPO_URL),
        (r"(?<![/\w.])discord\.gg/\w+", f"github.com/{OWNER_REPO}"),
        (r"https://(www\.)?(x|twitter)\.com/get_artcraft[\w/]*", REPO_URL),
        (r"https://(www\.)?(instagram|tiktok|linkedin)\.com/[\w@./-]+", REPO_URL),
        (r"https://(www\.)?youtube\.com/@[\w./-]+", REPO_URL),
        (r"https://bsky\.app/[\w./-]+", REPO_URL),
    ]
    # "show the Discord button" style switches default to off
    rules.append((r"(\b\w*discord\w*\s*:\s*)true\b", None))
    for pat, repl in rules:
        if repl is None:
            text = re.sub(pat, lambda m: m.group(1) + "false", text, flags=re.I)
            continue
        text = re.sub(pat, lambda _m, r=repl: r, text)
    return text


# ---- driver -----------------------------------------------------------------------------------------

def source_files(root: str):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS and not d.startswith(".")]
        for f in filenames:
            if f.endswith(".rs") and not f.endswith(("_tests.rs", "_test.rs")) and f != "tests.rs":
                yield os.path.join(dirpath, f)


def load_rules(path: str | None) -> dict:
    if not path or not os.path.exists(path):
        return {}
    with open(path, "rb") as fh:
        if tomllib is None:
            sys.exit("Python 3.11+ is needed to read " + path)
        return tomllib.load(fh)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("--app", required=True)
    ap.add_argument("--rules")
    ap.add_argument("--keep", action="append", default=[], help="file (relative to src) that gets no removals")
    ap.add_argument("--report")
    ap.add_argument("--links-only", action="store_true", help="no removals at all, only link rewrites (fallback build)")
    args = ap.parse_args()

    rules = load_rules(args.rules)
    keep = {os.path.normpath(k) for k in args.keep}
    report = {"app": args.app, "files": {}, "replace_missed": [], "leftovers": []}

    # exact per-app replacements come first, while the code still looks like upstream
    for rule in ([] if args.links_only else rules.get("replace", [])):
        path = os.path.join(args.src, rule["file"])
        if not os.path.exists(path):
            report["replace_missed"].append(rule)
            continue
        with open(path, encoding="utf-8") as fh:
            text = fh.read()
        if rule["find"] not in text:
            report["replace_missed"].append(rule)
            continue
        text = text.replace(rule["find"], rule.get("with", ""))
        with open(path, "w", encoding="utf-8", newline="") as fh:
            fh.write(text)

    drops = {}
    for rule in rules.get("drop", []):
        drops.setdefault(os.path.normpath(rule["file"]), []).append(rule["at"])
    extra = {}
    for rule in rules.get("remove", []):
        extra.setdefault(os.path.normpath(rule["file"]), []).append(rule["marker"])

    for path in source_files(args.src):
        rel = os.path.normpath(os.path.relpath(path, args.src))
        with open(path, encoding="utf-8", errors="surrogateescape") as fh:
            original = fh.read()
        text = original
        cuts = []
        if rel not in keep and not args.links_only:
            for needle in drops.get(rel, []):
                text, ok = drop_statement(text, needle)
                if ok:
                    cuts.append({"kind": "drop", "code": needle})
                else:
                    report["replace_missed"].append({"file": rel, "drop": needle})
            # menus written as text ("Join the Discord… | help.discord") lose that line
            text, n = re.subn(r"(?im)^[ \t]*[^\"\n|]*discord[^\"\n|]*\|[ \t]*[\w.]+[ \t]*\n", "", text)
            cuts += [{"kind": "menu-line", "code": "discord"}] * n
            more, more_cuts = remove_branding(text, extra.get(rel, []))
            text, cuts = more, cuts + more_cuts
        text = rewrite_links(text, args.app)
        if text != original:
            with open(path, "w", encoding="utf-8", errors="surrogateescape", newline="") as fh:
                fh.write(text)
            report["files"][rel] = cuts
        for i, line in enumerate(text.splitlines(), 1):
            if re.search(r"discord", line, re.I) and not line.lstrip().startswith("//"):
                report["leftovers"].append(f"{rel}:{i}: {line.strip()[:160]}")

    # Non-Rust files the app ships: desktop entries, metainfo, packaging texts
    for dirpath, dirnames, filenames in os.walk(args.src):
        dirnames[:] = [d for d in dirnames if d not in {".git", "target", "node_modules"}]
        for f in filenames:
            if f.endswith((".xml", ".desktop", ".toml", ".json", ".ftl", ".wxs", ".plist", ".html", ".sh", ".ps1", ".yml", ".in")) and "test" not in f:
                path = os.path.join(dirpath, f)
                try:
                    with open(path, encoding="utf-8") as fh:
                        original = fh.read()
                except (UnicodeDecodeError, OSError):
                    continue
                text = rewrite_links(original, args.app)
                if os.sep + "packaging" in path:
                    # AppImage update info and similar point at this repo's releases
                    text = re.sub(r"\bstorytold/" + re.escape(args.app) + r"\b", OWNER_REPO, text)
                if text != original:
                    with open(path, "w", encoding="utf-8", newline="") as fh:
                        fh.write(text)

    if args.report:
        with open(args.report, "w", encoding="utf-8") as fh:
            json.dump(report, fh, indent=2)
    total = sum(len(c) for c in report["files"].values())
    print(f"{args.app}: {total} cuts in {len(report['files'])} files, {len(report['leftovers'])} leftover mentions, {len(report['replace_missed'])} rules missed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
