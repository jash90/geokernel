#!/usr/bin/env python3
"""Distinguish a literal constant tangent from a zero sampled slope."""

import json
from pathlib import Path

initial = {"action": "initial"}
cases = [
    {
        "id": "statistics-linecalltangent-001",
        "group": "statistics",
        "script": ["n = 0", "g: y = n*x+2", "f(x) = g(abs(x))", "T = Tangent(0,f)", "a = f(1)"],
        "observe": ["T", "a"],
        "steps": [
            initial,
            {"action": "set_number", "label": "n", "value": 1},
            {"action": "set_number", "label": "n", "value": 0},
        ],
    },
]
for i, expression in enumerate(["0*x+2", "x-x+2"], 2):
    cases.append({
        "id": f"statistics-linecalltangent-{i:03d}",
        "group": "statistics",
        "script": [f"g: y = {expression}", "f(x) = g(abs(x))", "T = Tangent(0,f)"],
        "observe": ["T"],
        "steps": [initial],
    })
Path(__file__).with_name("linecalltangent.json").write_text(
    json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8"
)
