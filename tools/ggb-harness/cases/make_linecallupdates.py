#!/usr/bin/env python3
"""Record live explicit equations and repeated direct applications."""

import json
from pathlib import Path

initial = {"action": "initial"}
cases = [
    {
        "id": "statistics-linecallupdates-001",
        "group": "statistics",
        "script": ["n = 0", "g: y = (x+n)-n", "f(x) = g(x)", "a = f(1)"],
        "observe": ["g", "a"],
        "steps": [initial, {"action": "set_number", "label": "n", "value": 2**53}],
    },
    {
        "id": "statistics-linecallupdates-002",
        "group": "statistics",
        "script": ["g: y = x+x", "a = " + "g(" * 18 + "1" + ")" * 18],
        "observe": ["a"],
        "steps": [initial],
    },
]
Path(__file__).with_name("linecallupdates.json").write_text(
    json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8"
)
