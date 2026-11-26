#!/usr/bin/env python3
"""Check the gap families listed in DIFFERENCES.md against the replay ledgers."""

import re
import sys
import tomllib
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TABLES = {
    "Golden corpus gap families": ("geo-golden/known-gaps.toml", ("gap",)),
    "Verified t-pairs gap families": ("geo-golden/t-pairs-gaps.toml", ("gap",)),
    "Import gap families": (
        "geo-import/known-import-gaps.toml",
        ("gap", "reclassified", "saved_tangency"),
    ),
}


def main():
    sections = {
        title: body
        for title, body in re.findall(
            r"^## ([^\n]+)\n(.*?)(?=^## |\Z)",
            (ROOT / "DIFFERENCES.md").read_text(encoding="utf-8"),
            re.MULTILINE | re.DOTALL,
        )
    }
    failed = False
    for title, (path, keys) in TABLES.items():
        with (ROOT / "crates" / path).open("rb") as source:
            ledger = tomllib.load(source)
        entries = [entry for key in keys for entry in ledger.get(key, [])]
        expected = Counter(
            entry["id"] + ("@" + entry["label"] if "label" in entry else "")
            for entry in entries
        )
        found = Counter()
        for line in sections.get(title, "").splitlines():
            # The first cell lists exact IDs. A family can share one reason.
            match = re.fullmatch(r"\| (.+?) \| (.+) \|", line)
            if match and match[1].startswith("`") and match[2].strip():
                found.update(re.findall(r"`([^`]+)`", match[1]))
        if missing := expected - found:
            print(f"{title}: undocumented: {', '.join(sorted(missing))}", file=sys.stderr)
            failed = True
        if extra := found - expected:
            print(f"{title}: stale or repeated: {', '.join(sorted(extra))}", file=sys.stderr)
            failed = True
        if expected == found:
            print(f"{title}: {len(entries)} entries documented")
    return int(failed)


if __name__ == "__main__":
    sys.exit(main())
