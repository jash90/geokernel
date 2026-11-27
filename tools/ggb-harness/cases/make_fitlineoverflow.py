"""Observe the finite normal of a regression line with an infinite intercept."""

import json
from pathlib import Path

cases = []
for index, (command, points) in enumerate(
    [
        ("FitLine", "{(-10^149,10^20),(10^149,2*10^20)}"),
        ("FitLineX", "{(-10^20,-10^149),(-2*10^20,10^149)}"),
    ],
    start=1,
):
    cases.append({
        "id": f"statistics-fitlineoverflow-{index:03d}",
        "group": "statistics",
        "script": [
            f"L = {points}",
            f"g = {command}(L)",
            "v = Direction(g)",
            "a = x(v)",
            "b = y(v)",
        ],
        "steps": [{"action": "initial"}],
        "observe": ["a", "b"],
    })

Path(__file__).with_name("fitlineoverflow.json").write_text(
    json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8"
)
