"""Write the pointer inputs for the kernel-ready toolbar tools."""

import json
from pathlib import Path


def case(name, mode, script, clicks, answer=None):
    events = []
    for x, y in clicks:
        time = len(events) * 100
        events.append(dict(t=time, type="mouse_down", pos=[x, y]))
        events.append(dict(t=time + 60, type="mouse_up", pos=[x, y]))
    if answer is not None:
        time = len(events) * 100
        events.append(dict(t=time, type="text", text=answer))
        events.append(dict(t=time + 60, type="key", key="Enter"))
    return dict(name=name, tool_mode=mode, initial_script=script, events=events)


scenarios = [
    case("tool_relation_compares_points", 14, ["A = (1, 1)", "B = (2, 1)"],
         [[1, 0.99], [4, 3.99], [2, 0.99]]),
    case("tool_rigid_polygon_closes", 64, [],
         [[1, 0.99], [4, 0.99], [1, 3.99], [1, 0.99]]),
    case("tool_segment_fixed_creates_endpoint", 45, [], [[1, 0.99]], "4"),
    case("tool_angle_fixed_on_segment", 46,
         ["A = (2, 0)", "B = (0, 0)", "f = Segment(A, B)"], [[1, -0.01]], "90"),
    case("tool_extremum_picks_function", 75, ["f(x) = (x - 1)^2 + 1"],
         [[4, 3.99], [1, 0.99]]),
    case("tool_roots_picks_function", 76, ["f(x) = x^2 - 1"],
         [[4, 3.99], [0, -1.01]]),
    case("tool_point_on_object_places_in_polygon", 501,
         ["A = (0, 0)", "B = (4, 0)", "C = (0, 4)", "p = Polygon(A, B, C)"],
         [[0, -0.01], [1, 0.99]]),
    case("tool_attach_detach_redefines_point", 67,
         ["A = (0, 0)", "B = (4, 0)", "f = Segment(A, B)",
          "P = (2, 2)", "M = Midpoint(A, P)"],
         [[2, 1.99], [3, -0.01], [3, -0.01]]),
    case("tool_attach_detach_frees_path_point", 67,
         ["A = (0, 0)", "B = (4, 0)", "f = Segment(A, B)",
          "P = Point(f, 0.75)", "M = Midpoint(A, P)"],
         [[3, -0.01]]),
]

Path(__file__).with_name("tool_pointer.json").write_text(
    json.dumps({"scenarios": scenarios}, indent=2) + "\n", encoding="utf-8"
)
