"""Write the pointer inputs for the kernel-ready toolbar tools."""

import json
from pathlib import Path


def case(name, mode, script, clicks, answer=None, observe=None):
    events = []
    for x, y in clicks:
        time = len(events) * 100
        events.append(dict(t=time, type="mouse_down", pos=[x, y]))
        events.append(dict(t=time + 60, type="mouse_up", pos=[x, y]))
    if answer is not None:
        time = len(events) * 100
        events.append(dict(t=time, type="text", text=answer))
        events.append(dict(t=time + 60, type="key", key="Enter"))
    scenario = dict(name=name, tool_mode=mode, initial_script=script, events=events)
    if observe is not None:
        scenario["observe"] = observe
    return scenario


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
    case("tool_point_on_x_axis", 501, [], [[2, -0.01]]),
    case("tool_point_on_y_axis", 501,
         ["A = (-6, -3)", "B = (-2, -3)", "C = (-4, 1)", "f = Line(A, B)",
          "D = (4, -2)", "E = (6, -2)", "k = Circle(D, E)", "G = (-7, 3)",
          "H = (-3, 4)", "u = Vector(G, H)"], [[0, 1.99]]),
    case("tool_point_on_vector", 501,
         ["A = (0, 0)", "B = (4, 2)", "u = Vector(A, B)"], [[2, 0.99]]),
    case("tool_segment_fixed_starts_on_function", 45, ["f(x) = x^2"], [[1, 0.99]], "4"),
    case("tool_angle_fixed_segment_after_point", 46,
         ["A = (2, 0)", "B = (0, 0)", "C = (1, 2)", "f = Segment(A, B)"],
         [[1, 1.99], [1, -0.01]], "90"),
    case("tool_angle_fixed_negative", 46,
         ["A = (2, 0)", "B = (0, 0)", "f = Segment(A, B)"], [[1, -0.01]], "-90°"),
    case("tool_segment_fixed_expression", 45, [], [[1, 0.99]], "1+2"),
    case("tool_attach_detach_frees_fixed_endpoint", 67,
         ["A = (0, 0)", "s = Segment(A, 3)", "M = Midpoint(A, B)"], [[3, -0.01]]),
    case("tool_relation_compares_text_and_point", 14,
         ['T = Text("hello", (1, 1))', "A = (2, 2)"], [[1.2, 1.15], [2, 1.99]]),
    case("tool_roots_picks_circle", 76, ["O = (0, 0)", "k = Circle(O, 2)"], [[0, 1.99]]),
    case("tool_extremum_picks_circle", 75, ["O = (0, 0)", "k = Circle(O, 2)"], [[0, 1.99]]),
    case("tool_point_on_large_polygon", 501,
         ["L = Sequence((k, k^2), k, 1, 4097)", "p = Polygon(L)"], [[1, 0.99]], observe=["A", "B"]),
    case("tool_point_on_large_polyline", 501,
         ["L = Sequence((k, k^2), k, 1, 4097)", "p = Polyline(L)"], [[1, 0.99]], observe=["A", "B"]),
    case("tool_rigid_copy_large_polygon", 64,
         ["L = Sequence((k, k^2), k, 1, 4097)", "p = Polygon(L)"], [[1, 0.99]], observe=["A", "B"]),
    case("tool_relation_compares_large_polygon", 14,
         ["L = Sequence((k, k^2), k, 1, 4097)", "p = Polygon(L)", "A = (2, 4)"],
         [[1, 0.99], [2, 3.99]], observe=["A"]),
]

(Path(__file__).parent / "pointer" / "tool_pointer.json").write_text(
    json.dumps({"scenarios": scenarios}, indent=2) + "\n", encoding="utf-8"
)
