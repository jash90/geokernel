#!/usr/bin/env python3
"""Finite raw-data selections for the shared spreadsheet analysis model."""

import json
from pathlib import Path


def edit(cell, source):
    return {"action": "edit", "cell": cell, "input": source}


def analyze(anchor, end):
    return {"action": "analyze_one_variable", "cell": anchor, "target": end}


def case(index, *actions):
    return {
        "id": f"spreadsheet-onevariable-{index:03d}",
        "group": "spreadsheet",
        "script": [],
        "steps": [{"action": "initial"}],
        "spreadsheet": list(actions),
    }


cases = [
    case(
        1,
        edit("A1", "2"),
        edit("B2", "-3"),
        edit("B3", "2"),
        edit("B4", "2"),
        edit("B5", "=A1 + 3"),
        edit("B6", "8"),
        analyze("B2", "B6"),
        edit("B3", "-1"),
        edit("A1", "7"),
        {"action": "undo"},
        {"action": "undo"},
        {"action": "redo"},
        {"action": "redo"},
    ),
    case(
        2,
        edit("B2", "-3"),
        edit("C2", "2"),
        edit("D2", "2"),
        edit("E2", "5"),
        edit("F2", "8"),
        analyze("F2", "B2"),
    ),
    case(3, edit("C3", "4"), analyze("C3", "C3")),
]

Path(__file__).with_name("spreadsheet_one_variable.json").write_text(
    json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8"
)
