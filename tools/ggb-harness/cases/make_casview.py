#!/usr/bin/env python3
"""Interaction cases for the headless CAS view."""

import json
from pathlib import Path


def evaluate(row, source):
    return {"action": "evaluate", "row": row, "input": source}


def edit(row, source):
    return {"action": "edit", "row": row, "input": source}


def delete(row):
    return {"action": "delete", "row": row}


def case(number, *actions, script=()):
    return {
        "id": f"casview-rows-{number:03}",
        "group": "casview",
        "script": list(script),
        "steps": [{"action": "initial"}],
        "casview": actions,
    }


cases = [
    case(1, evaluate(1, "2/3 * 5/7"), evaluate(2, "Expand((x + 1) * (x - 1))")),
    case(2, evaluate(1, "2"), evaluate(2, "$1 + 3"), evaluate(3, "$2 + 1"), edit(1, "7")),
    case(3, evaluate(1, "2"), evaluate(2, "NextPrime(100)"), evaluate(3, "$2 + 1"),
         delete(1), delete(1)),
    case(4, evaluate(1, "1 / 0")),
    case(5, evaluate(1, "f(x) = 1 + Simplify(g)"), script=("g: 2x + 3y = 6",)),
    case(6, evaluate(1, "{" + ", ".join(str(i) for i in range(1, 101)) + "}")),
]
Path(__file__).with_name("casview.json").write_text(
    json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8"
)
