"""Observe the two normal components when overflow makes the intercept NaN."""

import json
from pathlib import Path

case = {
    "id": "statistics-fitlinenan-001",
    "group": "statistics",
    "script": [
        "n = 511", "t = 2^n", "L = {(t,t),(-t,-t)}",
        "g = FitLine(L)", "h = FitLineX(L)", "v = Direction(g)", "w = Direction(h)",
        "a = x(v)", "b = y(v)", "p = x(w)", "q = y(w)",
    ],
    "steps": [{"action": "initial"}] + [
        {"action": "set_number", "label": "n", "value": n} for n in (512, 513, 511)
    ],
    "observe": ["g", "h", "a", "b", "p", "q"],
}

Path(__file__).with_name("fitlinenan.json").write_text(
    json.dumps({"cases": [case]}, indent=2) + "\n", encoding="utf-8"
)
