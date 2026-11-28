#!/usr/bin/env python3
"""Scalar application of a line on either side of the vertical threshold."""

import json
from pathlib import Path


def set_width(width):
    return {"action": "set_number", "label": "width", "value": width}


case = {
    "id": "statistics-linecalllimits-001",
    "group": "statistics",
    "script": [
        "width = 0.00000002",
        "g = Line((0,0),(width,1))",
        "direct = g(2)",
        "f(x) = 1 + g(x)",
        "nested = f(2)",
    ],
    "observe": ["g", "direct", "nested"],
    "steps": [
        {"action": "initial"},
        set_width(1e-9),
        set_width(1e-8),
        set_width(0),
        set_width(-2e-8),
    ],
}
Path(__file__).with_name("linecalllimits.json").write_text(
    json.dumps({"cases": [case]}, indent=2) + "\n", encoding="utf-8"
)
