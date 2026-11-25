#!/usr/bin/env python3
"""Spreadsheet commands and headless cell edits."""

import json
from pathlib import Path


def edit(cell, source):
    return {"action": "edit", "cell": cell, "input": source}


def command(source, *labels):
    return {"action": "command", "input": source, "observe": list(labels)}


def case(topic, *actions):
    return {
        "id": f"spreadsheet-{topic}-001",
        "group": "spreadsheet",
        "script": [],
        "steps": [{"action": "initial"}],
        "spreadsheet": list(actions),
    }


cases = [
    case("range", command("r = CellRange(A1, A1)", "r"), edit("A1", "2"),
         edit("A2", "3"), edit("B1", "5"), command("s = A1:B2", "r", "s"),
         command('t = CellRange(C1, C2, "numeric")', "t"),
         edit("D1", "=Sum($A$1:B2)")),
    case("address", edit("AA7", "3"),
         command("r = Row(AA7)", "r"), command("c = Column(AA7)", "c"),
         command("t = ColumnName(AA7)", "t"), command("u = Row((1, 1))", "u"),
         command("v = Column((1, 1))", "v"), command("w = ColumnName((1, 1))", "w")),
    case("lookup", edit("A1", "3"), edit("B1", "4"), command("n = 1", "n"),
         command("v = Cell(n, 1)", "v"), command("n = 2", "v"),
         edit("B1", "7"), command("w = Cell(42, 4)", "v", "w")),
    case("fillrow", command("FillRow(4, {1, 2, 3, 4, 5})")),
    case("fillcolumn", command("FillColumn(13, {1, 2, 3, 4, 5})")),
    case("fillcells", command("FillCells(A1, {1,2,3,4,5})"),
         command("FillCells(A1, {{1,2},{3,4}})"), command("FillCells(A3:C4, {5,6})"),
         command("FillCells(F1:F2, A1+1)")),
    case("edit", edit("A1", "2"), edit("B1", "=A1 / 3"), edit("B2", "Hello"),
         edit("A1", "8"), edit("C1", "=0/0"), edit("B2", "")),
]

cases.extend([
    case("latecell", edit("A1", "3"), command("n = 5", "n"),
         command("v = Cell(n, 1)", "v"), edit("E1", "9"), edit("E1", "12")),
    case("files", edit("A1", "1.250"), edit("B1", "=$A$1 * 2"), edit("A2", "Total"),
         edit("B2", "=0/0"), command("FillCells(A3:B3, {3,4})")),
    case("matrix", command('FillCells(B2:D4, {{1,2},{"label",4}})')),
    case("textlookup", edit("B2", "value"), command("t = Cell(2, 2)", "t"),
         edit("B2", "other")),
    case("drag", edit("A1", "2"), edit("A2", "3"), edit("A3", "5"),
         edit("B1", "=$A$1 + A1"),
         {"action": "drag", "cell": "B1", "target": "B3"}),
    case("undo", edit("A1", "2"), edit("A2", "=A1 + 3"),
         edit("A1", "7"), {"action": "undo"}, {"action": "redo"}),
    case("locus", command("P = (1, 1)"), command("Q = (4, 4)"),
         command("s = Segment(P, Q)"), command("U = Point(s)"),
         command("V = (x(U), y(U) + 1)"), command("l = Locus(V, U)"),
         command("FillCells(E1:F3, l)")),
])

cases.extend([
    case("algebrareload", edit("A1", "1"), edit("B1", "=A1+1"),
         command("SetValue(A1, 2)"), {"action": "reload"}),
    case("directdrag", edit("A1", "1"), edit("A2", "2"), edit("A3", "3"),
         edit("B1", "=A1"), {"action": "drag", "cell": "B1", "target": "B3"},
         edit("A2", "8"), edit("A3", "9")),
    case("cleardependents", edit("A1", "2"), edit("B1", "=A1+1"), edit("A1", "")),
    case("invalidedit", edit("A1", "2"), edit("B1", "=A1+1"), edit("A1", "=1+")),
    case("nestedlatecell", command("n = 5", "n"), command("v = Cell(n,1)+1", "v"),
         edit("E1", "9")),
    case("unrelatedcellkind", edit("A1", "2"), edit("B1", "3"), command("n = 1", "n"),
         command("v = Cell(n,1)", "v"), edit("B1", "text")),
    case("repeatedlookup", command("FillCells(A1:ALL1, {2})"), command("n = 1", "n"),
         command("v = " + "+".join(["Cell(n,1)"] * 100), "v")),
    case("fillslider", command("B1 = Slider(0,10)"), command("FillRow(1,{7,8})")),
    case("inlinefillrange", command("FillCells(CellRange(A1,B2),{4})")),
    case("typedemptyrange", command('r = CellRange(A1,A2,"text")', "r"),
         command("v = Element(r,1)", "v")),
    case("geometrydisplay", {"action": "degree_mode"}, edit("A1", "=Segment((0,0),(3,4))"),
         edit("B1", "=90°")),
    case("viewsettings", {"action": "load", "input":
         '<geogebra format="5.0"><spreadsheetView><spreadsheetColumn id="0" width="200"/>'
         '<layout showGrid="false"/></spreadsheetView><construction/></geogebra>'}),
])

Path(__file__).with_name("spreadsheet.json").write_text(
    json.dumps({"cases": cases}, indent=2) + "\n", encoding="utf-8"
)
