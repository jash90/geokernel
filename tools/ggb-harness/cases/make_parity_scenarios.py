#!/usr/bin/env python3
"""Write fixed inputs for the interaction parity recordings."""

import json
from pathlib import Path


def pointer(t, kind, x, y):
    return {"t": t, "type": "mouse_" + kind, "pos": [x, y]}


def event(t, kind, **fields):
    return {"t": t, "type": kind, **fields}


def case(name, mode, script, *events):
    return dict(name=name, tool_mode=mode, initial_script=script, events=list(events))


directory = Path(__file__).resolve().parent
fixture = directory.parents[2] / "crates/geo-mcp/tests/fixtures/pointer_scenarios.json"
legacy = {s["name"]: s for s in json.loads(fixture.read_text())["scenarios"]}

# Copy the input fields exactly, including their original event times.
cases = [
    {key: legacy[name][key] for key in ("name", "tool_mode", "initial_script", "events")}
    for name in (
        "free_equation_line_not_draggable",
        "area_tool_on_polygon",
        "enter_on_empty_input_row",
        "tool_text_places_literal",
        "tool_input_box_places_empty_widget",
        "press_on_polygon_edge_does_nothing",
        "segment_tool_twice_stays_selected",
        "delete_tool_stroke_erases_crossed_object",
        "drag_point_on_line_slides",
        "zoom_in_tool_rectangle_fills_canvas",
    )
]

cases += [
    case("controller_drag_time_gate", 0, ["A = (1, 1)"],
         pointer(0, "down", 1, 0.99),
         {**pointer(80, "move", 2, 0.99), "since_press_ms": 80},
         {**pointer(150, "move", 3, 0.99), "since_press_ms": 150},
         pointer(230, "up", 3, 0.99)),
    case("hit_radius_mouse_and_touch", 0, ["A = (1, 1)"],
         event(0, "hit_test", pos=[1.4, 0.99], source="mouse"),
         event(150, "hit_test", pos=[1.4, 0.99], source="touch")),
    case("dependent_line_snap", 0, ["A = (1, 1)", "B = (4, 2)", "f = Line(A, B)"],
         pointer(0, "down", 2.5, 1.49), pointer(150, "move", 3.44, 2.41),
         pointer(210, "up", 3.44, 2.41)),
    case("grid_steps_during_zoom", 0, [],
         event(0, "wheel", pos=[0, -0.01], deltaY=-100),
         event(130, "wheel", pos=[0, -0.01], deltaY=-100),
         event(260, "wheel", pos=[0, -0.01], deltaY=-100),
         event(390, "wheel", pos=[0, -0.01], deltaY=-100)),
    case("pan_off_axis", 0, [],
         pointer(0, "down", 2, 1.99), pointer(150, "move", 3, 2.01),
         pointer(200, "move", 4, 2.03), pointer(260, "up", 4, 2.03)),
    case("pan_near_axis_inside_lock_cone", 40, [],
         pointer(0, "down", 2, 1.99), pointer(150, "move", 2.24, 2.07),
         pointer(200, "move", 2.8, 2.13), pointer(260, "up", 2.8, 2.13)),
    case("segment_preview_endpoint", 15, [],
         pointer(0, "down", 1, 0.99), pointer(60, "up", 1, 0.99),
         pointer(150, "move", 3.04, 2.05),
         pointer(220, "down", 3.04, 2.05), pointer(280, "up", 3.04, 2.05)),
    case("zoom_rectangle_near_view_aspect", 41, [],
         pointer(0, "down", -4, 3.99), pointer(150, "move", 2, -0.81),
         pointer(220, "up", 2, -0.81)),
    case("segment_preview_hits_existing_point", 15, ["B = (3, 2)"],
         pointer(0, "down", 1, 0.99), pointer(60, "up", 1, 0.99),
         pointer(150, "move", 3.04, 2.05)),
    case("resize_after_view_reload", 0, ["A = (1, 1)"],
         event(0, "wheel", pos=[2, 0.99], deltaY=-100),
         event(150, "reload_view"),
         event(250, "resize_canvas", width=1092, height=847)),
    case("rapid_second_press_on_point", 0, ["A = (1, 1)"],
         pointer(0, "down", 1, 0.99), pointer(40, "up", 1, 0.99),
         {**pointer(100, "down", 1, 0.99), "since_release_ms": 60},
         pointer(140, "up", 1, 0.99),
         {**pointer(700, "down", 1, 0.99), "since_release_ms": 560},
         pointer(740, "up", 1, 0.99)),
    case("point_visibility_with_automatic_labeling", 1, [],
         event(0, "algebra_visibility", visible=True),
         pointer(150, "down", 2, 0.99), pointer(220, "up", 2, 0.99),
         event(300, "algebra_visibility", visible=False),
         pointer(450, "down", 4, 2.99), pointer(520, "up", 4, 2.99)),
    case("context_menu_routes_object_or_canvas", 0, ["A = (1, 1)"],
         {**pointer(0, "down", 4, 2.99), "right_click": True},
         {**pointer(60, "up", 4, 2.99), "right_click": True},
         {**pointer(200, "down", 1, 0.99), "right_click": True},
         {**pointer(260, "up", 1, 0.99), "right_click": True}),
    case("select_point_for_algebra_highlight", 0, ["A = (1, 1)"],
         pointer(0, "down", 1, 0.99), pointer(60, "up", 1, 0.99)),
    case("drag_release_outside_canvas", 0, ["A = (8, 1)"],
         pointer(0, "down", 8, 0.99), pointer(150, "move", 8.5, 1.19),
         event(200, "mouse_exit"), pointer(250, "move", 9.2, 1.39),
         pointer(310, "up", 9.2, 1.39)),
    case("circle_radius_dialog_cancel", 34, [],
         pointer(0, "down", 2, 0.99), pointer(60, "up", 2, 0.99),
         event(200, "key", key="Escape")),
    case("wheel_checkpoint_at_next_press", 0, [],
         event(0, "wheel", pos=[2, 0.99], deltaY=-100),
         pointer(200, "down", 4, 2.99), pointer(260, "up", 4, 2.99)),
    case("wheel_checkpoint_at_exit", 0, [],
         event(0, "wheel", pos=[2, 0.99], deltaY=-100),
         event(260, "mouse_exit")),
    case("zoom_tool_checkpoint_on_leave", 41, [],
         pointer(0, "down", 2, 0.99), pointer(60, "up", 2, 0.99),
         event(260, "select_tool", mode=0)),
    case("algebra_description_separators", 0,
         ["A = (1, 1)", "a = 3", "f: y = 1", "g(x) = x^2"],
         event(0, "algebra_descriptions")),
    case("pinch_sub_threshold_step", 0, [],
         event(0, "pinch_start", fingers=[[346, 373], [546, 373]]),
         event(120, "pinch_move", fingers=[[345, 373], [547, 373]]),
         event(160, "pinch_move", fingers=[[296, 373], [596, 373]]),
         event(220, "pinch_end")),
    case("two_fingers_scale_x_axis", 0, [],
         event(0, "touch_gesture_start", fingers=[[346, 374], [546, 374]]),
         event(120, "touch_gesture_move", fingers=[[296, 374], [596, 374]])),
    case("two_fingers_scale_circle", 0, ["A = (0, 1)", "B = (0, 3)", "c = Circle(A, B)"],
         event(0, "touch_gesture_start", fingers=[[346, 324], [546, 324]]),
         event(120, "touch_gesture_move", fingers=[[296, 324], [596, 324]])),
    case("two_fingers_move_line", 0, ["A = (-3, 2)", "B = (3, 2)", "f = Line(A, B)"],
         event(0, "touch_gesture_start", fingers=[[346, 274], [546, 274]]),
         event(120, "touch_gesture_move", fingers=[[346, 244], [546, 244]])),
]

area = next(c for c in cases if c["name"] == "area_tool_on_polygon")
cases += [
    case("area_polygon_occupied_anchor_label", area["tool_mode"],
         [*area["initial_script"], "Pointp = (0, 0)"], *area["events"]),
    case("area_polygon_inline_vertices", area["tool_mode"],
         ["p = Polygon((1,1),(5,1),(1,4))"], *area["events"]),
    case("area_polygon_mixed_vertices", area["tool_mode"],
         ["A = (1,1)", "p = Polygon(A,(5,1),(1,4))"], *area["events"]),
    dict(case("area_polygon_grows_after_placement", area["tool_mode"],
              ["n = 3", "p = Polygon(Sequence((k, k^2), k, 1, n))"],
              pointer(0, "down", 2, 4.51), pointer(60, "up", 2, 4.51),
              event(200, "command", input="SetValue(n, 4097)")),
         observe=["n", "Pointp", "Textp"]),
]

(directory / "pointer" / "parity-scenarios.json").write_text(
    json.dumps({"scenarios": cases}, indent=2) + "\n", encoding="utf-8"
)
