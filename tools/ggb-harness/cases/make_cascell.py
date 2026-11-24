#!/usr/bin/env python3
"""Input cases for one-row evaluation in GeoGebra's CAS view."""

import json
from pathlib import Path


def case(topic, number, source, *script):
    return {
        "id": f"cascell-{topic}-{number:03}",
        "group": "cascell",
        "script": list(script),
        "steps": [{"action": "initial"}],
        "cascell": source,
    }


cases = [
    case("exact", 1, "sqrt(2) + sqrt(3)"),
    case("exact", 2, "2/3 * 5/7"),
    case("exact", 3, "9007199254740992 + 1"),
    case("exact", 4, "Expand((x + 1) * (x - 1))"),
    case("exact", 5, "Factor(x^2 - 1)"),
    case("exact", 6, "(x + 1) * (x - 1)"),
    case("exact", 7, "Solve(x^2 = 1)"),
    case("exact", 8, "NextPrime(100)"),
    case("exact", 9, "1 / 0"),
    case("exact", 10, "?"),
    case("exact", 11, " 2/3 * 5/7 = "),
    case("exact", 12, "f(x) = 1 + Simplify(g)", "g: 2x + 3y = 6"),
    case("display", 1, "Factor(x^3-x)"),
    case("display", 2, "Factor(x^4-x^2)"),
    case("display", 3, "Factor((x-2)*(x+1)*(x+3))"),
    case("display", 4, "g", "g: x + y = 0"),
    case("display", 5, "g", "g: x + y = 0.5"),
]
Path(__file__).with_name("cascell.json").write_text(
    json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8"
)
