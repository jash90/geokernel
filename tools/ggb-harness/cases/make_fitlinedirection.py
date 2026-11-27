"""Record raw fit-line directions through edits that leave a line undefined."""

import json
from pathlib import Path


def direction_case(index, command, transpose):
    def point(x, y):
        return (y, x) if transpose else (x, y)

    script = [
        f"{label} = {point(x, y)}"
        for label, x, y in [("A", 2, 1), ("B", 3, 3), ("C", 2, 5)]
    ]
    script += [f"g = {command}({{A,B,C}})", "v = Direction(g)", "a = x(v)", "b = y(v)"]
    moves = [point(x, 3) for x in (2, 2.0001, 3)]
    return {
        "id": f"statistics-fitlinedirection-{index:03d}",
        "group": "statistics",
        "script": script,
        "steps": [{"action": "initial"}] + [
            {"action": "move", "label": "B", "x": x, "y": y} for x, y in moves
        ],
        "observe": ["g", "v", "a", "b"],
    }


cases = [direction_case(1, "FitLine", False), direction_case(2, "FitLineX", True)]
cases.append({
    "id": "statistics-fitlinedirection-003",
    "group": "statistics",
    "script": [
        "n = 2", "t = 2^n", "L = {(t,t),(2*t,2*t)}",
        "g = FitLine(L)", "h = FitLineX(L)",
        "v = Direction(g)", "w = Direction(h)",
    ],
    "steps": [{"action": "initial"}] + [
        {"action": "set_number", "label": "n", "value": n} for n in (342, 343, 2)
    ],
    "observe": ["g", "h", "v", "w"],
})

Path(__file__).with_name("fitlinedirection.json").write_text(
    json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8"
)
