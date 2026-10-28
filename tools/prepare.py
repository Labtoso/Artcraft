#!/usr/bin/env python3
"""Debrand an upstream checkout and prove it still compiles; write the result as a patch.

1. debrand.py with the app's rules,
2. `cargo check` of the app's binary,
3. on a compile error, files named in the errors lose their removals (links are still
   rewritten) and it starts over, up to five rounds,
4. `git diff` becomes build/<app>.patch, the debrand report build/<app>.report.json.

The launcher is not debranded, only switched to this repo's builds (patch_launcher.py).

Usage: prepare.py SRC_DIR APP_ID
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)


def run(cmd, cwd, check=True, capture=False):
    print("+", " ".join(cmd), flush=True)
    return subprocess.run(cmd, cwd=cwd, check=check, text=True, capture_output=capture)


def reset(src):
    run(["git", "checkout", "--", "."], src)
    run(["git", "clean", "-fdq"], src)


def links_only(src, app, out, why) -> int:
    """Last resort: ship the app with only its links rewritten (it compiles like the original)."""
    print(f"::warning title={app}::{why}; building with links rewritten only", flush=True)
    reset(src)
    run([sys.executable, "-I", os.path.join(HERE, "debrand.py"), src, "--app", app, "--links-only"], ROOT)
    diff = run(["git", "diff", "--binary"], src, capture=True).stdout
    for name in (f"{app}.patch", f"{app}.safe.patch"):
        with open(os.path.join(out, name), "w", encoding="utf-8", newline="") as fh:
            fh.write(diff)
    return 0


def main() -> int:
    src, app = os.path.abspath(sys.argv[1]), sys.argv[2]
    out = os.path.join(ROOT, "build")
    os.makedirs(out, exist_ok=True)
    report_path = os.path.join(out, f"{app}.report.json")
    env = dict(os.environ, RUSTFLAGS="--cap-lints=warn", CARGO_TERM_COLOR="never")

    if app == "artcraft-launcher":
        run([sys.executable, "-I", os.path.join(HERE, "patch_launcher.py"), src], ROOT)
        steps = [[]]
    else:
        steps = None

    keep: list[str] = []
    for attempt in range(1, 7):
        if steps is None:
            reset(src)
            cmd = [sys.executable, "-I", os.path.join(HERE, "debrand.py"), src, "--app", app, "--report", report_path]
            rules = os.path.join(ROOT, "apps", app, "debrand.toml")
            if os.path.exists(rules):
                cmd += ["--rules", rules]
            for k in keep:
                cmd += ["--keep", k]
            run(cmd, ROOT)
        res = subprocess.run(["cargo", "check", "--locked", "-p", app], cwd=src, env=env, text=True, capture_output=True)
        sys.stdout.write(res.stdout[-20000:])
        sys.stderr.write(res.stderr[-20000:])
        if res.returncode == 0:
            break
        if steps is not None:
            sys.exit("launcher does not compile after switching its sources")
        broken = sorted({os.path.normpath(p) for p in re.findall(r"-->\s+([^\s:]+\.rs):\d+:\d+", res.stderr)} - set(keep))
        broken = [b for b in broken if not os.path.isabs(b) and not b.startswith("..")]
        if not broken:
            return links_only(src, app, out, f"cargo check fails for a reason the debranding can't step back from")
        print(f"{app}: attempt {attempt} broke {broken}; keeping their code, links only", flush=True)
        keep += broken
    else:
        return links_only(src, app, out, f"still not compiling after keeping {keep}")

    diff = run(["git", "diff", "--binary"], src, capture=True).stdout
    if steps is None:
        # fallback for the build jobs: the same app with only its links rewritten
        reset(src)
        run([sys.executable, "-I", os.path.join(HERE, "debrand.py"), src, "--app", app, "--links-only"], ROOT)
        safe = run(["git", "diff", "--binary"], src, capture=True).stdout
        with open(os.path.join(out, f"{app}.safe.patch"), "w", encoding="utf-8", newline="") as fh:
            fh.write(safe)
    else:
        with open(os.path.join(out, f"{app}.safe.patch"), "w", encoding="utf-8", newline="") as fh:
            fh.write(diff)
    with open(os.path.join(out, f"{app}.patch"), "w", encoding="utf-8", newline="") as fh:
        fh.write(diff)
    if os.path.exists(report_path):
        with open(report_path, encoding="utf-8") as fh:
            report = json.load(fh)
        report["kept_files"] = keep
        with open(report_path, "w", encoding="utf-8") as fh:
            json.dump(report, fh, indent=2)
    print(f"{app}: patch with {diff.count(chr(10))} lines, {len(keep)} files kept as they were")
    return 0


if __name__ == "__main__":
    sys.exit(main())
