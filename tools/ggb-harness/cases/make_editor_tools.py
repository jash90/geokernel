#!/usr/bin/env python3
"""Pointer inputs for the Classic editor-tool probe."""

import json
import math
import struct
import zlib
from pathlib import Path


def click(x, y):
    return [("mouse_down", x, y + 0.01), ("mouse_up", x, y + 0.01)]


def case(name, mode, script, events, **observations):
    event_list = []
    for index, (kind, *args) in enumerate(events):
        event = {"t": index * 60, "type": kind}
        if kind == "select_tool":
            event["mode"] = args[0]
        else:
            event["pos"] = args
        event_list.append(event)
    return {
        "name": "tool_" + name,
        "tool_mode": mode,
        "initial_script": script,
        "events": event_list,
        **observations,
    }


point_script = ["A = (2, 1)", "B = (4, 1)", "C = (6, 1)"]
cases = [
    case("label_visibility", 28, point_script,
         click(2, 1) + click(4, 1) + [("select_tool", 0)], labels=["A", "B", "C"]),
    case("copy_visual_style", 35, point_script + ["$drawingStyle(_41, 2, false, 7, rgb285aa0)"],
         click(2, 1) + click(4, 1) + click(6, 1) + click(2, 1),
         labels=["B", "C"], style=True),
    case("move_rotate", 39, point_script[:2],
         click(2, 1) + [("mouse_down", 4, 1.01), ("mouse_move", 3, 2.01), ("mouse_move", 2, 3.01),
                         ("mouse_up", 2, 3.01)], points=["B"]),
    case("image_fixture", 26, point_script[:1], click(2, 1), image=True),
    case("function_inspector", 68, ["f(x) = x^2"], click(1, 1), inspector=True),
    case("freehand_shape", 73, [],
         [("mouse_down", 1, 1.01), ("mouse_move", 1.1, 0.91), ("mouse_move", 1.2, 0.81),
          ("mouse_up", 1.2, 0.81)], points=["A", "B"]),
    case("freehand_function", 74, [],
         [("mouse_down", 1, 1.01), ("mouse_move", 2, 2.01), ("mouse_move", 3, 3.01),
          ("mouse_up", 3, 3.01)], function=True),
]
cases += [
    case("imported_line_thickness", 0,
         ["A = (2, 1)", "B = (4, 1)", "l = Line(A, B)", "m = Line(A, (3, 2))"], [],
         thicknesses={"m": 10}, labels=["l", "m"], style=True, regression=True),
    case("label_then_cas_undo", 28, point_script[:1], click(2, 1),
         cas_then_undo="1 + 2", regression=True),
]
cases += [
    case("inspector_negative_endpoint", 68, ["f(x) = 0"], click(-7.88, 0), inspector=True, regression=True),
    case("rotate_parent_of_center", 39, point_script[:2] + ["M = Midpoint(A, B)"],
         click(3, 1) + [("mouse_down", 2, 1.01), ("mouse_move", 3, 2.01),
                        ("mouse_up", 3, 2.01)], points=["A", "M"], regression=True),
    case("copy_style_reload", 35, point_script[:2] + ["$drawingStyle(_41, 2, false, 7, rgb285aa0)"],
         click(2, 1) + click(4, 1), labels=["B"], style=True, reload=True, regression=True),
    case("image_cancel", 26, point_script[:1], click(2, 1), image=True, cancel_image=True, regression=True),
    case("freehand_vertical", 74, [],
         [("mouse_down", 1, 1.01), ("mouse_move", 1, 2.01), ("mouse_move", 1, 3.01),
          ("mouse_up", 1, 3.01)], function_defined=True, regression=True),
]
hyperbola = [(2 + math.cosh(i / 8), math.sinh(i / 8)) for i in range(-15, 16)]
cases += [
    case("freehand_hyperbola", 73, [],
         [("mouse_down", *hyperbola[0])] + [("mouse_move", *p) for p in hyperbola[1:]]
         + [("mouse_up", *hyperbola[-1])], conic_type=True, regression=True),
    case("text_label_visibility", 28, ["A = (2, 1)", 't = Text("Canvas text", A)'],
         click(2, 1) + [("select_tool", 0)], text_hit="t", labels=["t"], regression=True),
]
cases += [
    case("styled_curve_zoom", 0, ["c: x^2 + 4y^2 = 16"], [],
         thicknesses={"c": 10}, labels=["c"], style=True, zoom=True, regression=True),
]
directory = Path(__file__).with_name("editor_tools")
directory.mkdir(exist_ok=True)
directory.joinpath("inputs.json").write_text(json.dumps({"cases": cases}, indent=2) + "\n")


def chunk(kind, payload):
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))


directory.joinpath("fixture.png").write_bytes(
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(b"\x00\x00\x00\x00\xff"))
    + chunk(b"IEND", b"")
)
