#!/usr/bin/env python3
"""Record horizontal derivatives and a typed infinite intercept."""

import json
from pathlib import Path

scripts = [
    (["g: y = 2", "f(x) = g(abs(x))", "T = Tangent(0,f)"], ["T"]),
    (["g = Line((0,2),(2,2))", "f(x) = g(abs(x))", "T = Tangent(0,f)"], ["T"]),
    (["n = 0", "g: y = 1/n", "a = g(0)"], ["a"]),
    (
        ["n = 0", "g: y = x/n", "a = g(-1)", "b = g(0)", "d = g(1)", "valid = IsDefined(g)"],
        ["a", "b", "d", "valid"],
    ),
]
cases = [
    {
        "id": f"statistics-linecallhorizontal-{i:03d}",
        "group": "statistics",
        "script": script,
        "observe": observe,
        "steps": [{"action": "initial"}],
    }
    for i, (script, observe) in enumerate(scripts, 1)
]
cases.append({
    "id": "statistics-linecallhorizontal-005",
    "group": "statistics",
    "script": ["n = 2", "c = 1/n", "g: x + y = c", "a = g(0)", "d = IsDefined(g)"],
    "observe": ["a", "d"],
    "steps": [
        {"action": "initial"},
        {"action": "set_number", "label": "n", "value": 0},
        {"action": "set_number", "label": "n", "value": 2},
    ],
})
Path(__file__).with_name("linecallhorizontal.json").write_text(
    json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8"
)
