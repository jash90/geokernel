#!/usr/bin/env python3
"""Record original equation evaluation, signed zero and tangent slopes."""

import json
from pathlib import Path


def case(number, script, observe):
    return {
        "id": f"statistics-linecallexpansion-{number:03d}",
        "group": "statistics",
        "script": script,
        "observe": observe,
        "steps": [{"action": "initial"}],
    }


cases = [
    case(1, ["g: y = (x + 1)/3", "f(x) = g(x)", "a = 1/(f(5)-2)", "b = 1/(g(5)-2)"], ["a", "b"]),
    case(2, ["g = Line((2,-1),(0,0))", "f(x) = g(x)", "a = 1/f(0)"], ["a"]),
    case(3, ["g = Line((0,1),(2,5))", "f(x) = 1 + g(x)", "T = Tangent(3,f)"], ["T"]),
]
Path(__file__).with_name("linecallexpansion.json").write_text(
    json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8"
)
