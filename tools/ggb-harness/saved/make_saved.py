#!/usr/bin/env python3
"""Writes saved/saved.json: constructions saved after edits.

Each case gives raw GeoGebra input lines and optional steps to move points,
change a number, enable animation, set an angle style, rename an object, or
set presentation properties (`style`).
It can also set the angle unit or hide specific objects. The harness
(`harness.sh saved`) writes the construction file and GeoGebra's own values
at that moment. The importer checks intersection and tangent output order
after edits, angles under both units, and values of saved expressions.
Fixed seeds keep the input deterministic. New cases are appended to their topic.
"""

import json
import random
from pathlib import Path


def fmt(v):
    v = round(float(v), 6)
    return str(int(v)) if v == int(v) else ("%.6f" % v).rstrip("0")


def pt(label, x, y):
    return f"{label} = ({fmt(x)}, {fmt(y)})"


def mv(label, x, y):
    return {"action": "move", "label": label, "x": round(float(x), 6), "y": round(float(y), 6)}


cases = []
counters = {}
rng = random.Random()


def topic(name):
    rng.seed(f"saved-{name}", version=2)


def case(name, script, steps, unit=None, hidden=None, archive=False,
         images=None, after_images=None):
    counters[name] = counters.get(name, 0) + 1
    entry = {"id": f"saved-{name}-{counters[name]:03d}"}
    if unit:
        entry["angle_unit"] = unit
    if hidden:
        entry["hidden"] = hidden
    if archive:
        entry["archive"] = True
    entry["script"] = script
    if images:
        entry["images"] = images
    if after_images:
        entry["after_images"] = after_images
    entry["steps"] = steps
    cases.append(entry)


def r2(lo=-6.0, hi=6.0):
    return round(rng.uniform(lo, hi), 2)


case("curve-branch",
     ["a = 0", "c = Curve(t^2, t * (t^2 - 1) + a * t, t, -2, 2)",
      "P = Point(c)", "Q = Point(c)", "p = PathParameter(P)", "q = PathParameter(Q)"],
     [{"action": "curve_parameter", "label": "P", "value": 1},
      {"action": "curve_parameter", "label": "Q", "value": -1}])

# A line and a circle: the line is dragged (both points, or one) before saving,
# including through the direction that swaps the order of the two points.
topic("lc")
for n in range(24):
    ax, ay, r = r2(-3, 3), r2(-3, 3), round(rng.uniform(1.5, 4), 2)
    cx, cy = ax - 5, round(ay + rng.uniform(-0.5, 0.5), 2)
    dx, dy = ax + 5, round(ay + rng.uniform(-0.5, 0.5), 2)
    steps = []
    for _ in range(rng.randint(1, 4)):
        which = rng.choice(["C", "D", "A"])
        if which == "A":
            steps.append(mv("A", ax + r2(-1, 1), ay + r2(-1, 1)))
        elif which == "C":
            steps.append(mv("C", cx + r2(-3, 8), ay + r2(-2, 2)))
        else:
            steps.append(mv("D", dx + r2(-8, 3), ay + r2(-2, 2)))
    if n % 4 == 3:  # reverse the direction of the line
        steps += [mv("C", dx, dy), mv("D", cx, cy)]
    case("lc", [pt("A", ax, ay), f"r = {fmt(r)}", "k = Circle(A, r)",
                pt("C", cx, cy), pt("D", dx, dy), "f = Line(C, D)", "P = Intersect(f, k)"],
         steps)

# Two circles: centres and radii change before saving.
topic("cc")
for n in range(24):
    ax, ay = r2(-3, 3), r2(-3, 3)
    bx, by = round(ax + rng.uniform(2, 4), 2), round(ay + rng.uniform(-2, 2), 2)
    r, s = round(rng.uniform(1.5, 3), 2), round(rng.uniform(1.5, 3), 2)
    steps = []
    for _ in range(rng.randint(1, 4)):
        if rng.random() < 0.5:
            steps.append(mv("A", ax + r2(-2, 2), ay + r2(-2, 2)))
        else:
            steps.append(mv("B", bx + r2(-2, 2), by + r2(-2, 2)))
    if n % 4 == 3:  # swap the centres
        steps += [mv("A", bx, by), mv("B", ax, ay)]
    case("cc", [pt("A", ax, ay), f"r = {fmt(r)}", "k = Circle(A, r)",
                pt("B", bx, by), f"s = {fmt(s)}", "c = Circle(B, s)", "P = Intersect(k, c)"],
         steps)

# Tangents from a point to a circle: the point goes round the circle.
topic("tangent")
for n in range(16):
    ax, ay, r = r2(-2, 2), r2(-2, 2), round(rng.uniform(1, 3), 2)
    px, py = round(ax + r + rng.uniform(1, 3), 2), r2(-2, 2)
    steps = []
    for _ in range(rng.randint(1, 3)):
        steps.append(mv("P", ax + rng.choice([-1, 1]) * (r + rng.uniform(0.5, 4)),
                        ay + rng.choice([-1, 1]) * (r + rng.uniform(0.5, 4))))
    if n % 4 == 3:
        steps.append(mv("A", ax + r2(-2, 2), ay + r2(-2, 2)))
    case("tangent", [pt("A", ax, ay), f"r = {fmt(r)}", "k = Circle(A, r)",
                     pt("P", px, py), "t = Tangent(P, k)"], steps)

# The very first update after creation: GeoGebra fixes its permutation of the
# two points then (AlgoIntersectLineConic / AlgoIntersectConics:
# `isPermutationNeeded`), so the first move reverses the line's direction (C
# past D), or swaps the two centres, and nothing else happens before saving.
topic("firstmove")
for n in range(12):
    ax, ay, r = r2(-3, 3), r2(-3, 3), round(rng.uniform(1.5, 4), 2)
    cx, cy = ax - 5, round(ay + rng.uniform(-0.5, 0.5), 2)
    dx, dy = ax + 5, round(ay + rng.uniform(-0.5, 0.5), 2)
    case("firstmove", [pt("A", ax, ay), f"r = {fmt(r)}", "k = Circle(A, r)",
                       pt("C", cx, cy), pt("D", dx, dy), "f = Line(C, D)",
                       "P = Intersect(f, k)"],
         [mv("C", dx + 1, dy), mv("D", cx - 1, cy)] if n % 2 else [mv("C", dx + 1, dy)])
for n in range(12):
    ax, ay = r2(-3, 3), r2(-3, 3)
    bx, by = round(ax + rng.uniform(2, 4), 2), round(ay + rng.uniform(-2, 2), 2)
    r, s = round(rng.uniform(1.5, 3), 2), round(rng.uniform(1.5, 3), 2)
    case("firstmove", [pt("A", ax, ay), f"r = {fmt(r)}", "k = Circle(A, r)",
                       pt("B", bx, by), f"s = {fmt(s)}", "c = Circle(B, s)",
                       "P = Intersect(k, c)"],
         [mv("A", bx + 0.5, by)] if n % 2 else [mv("A", bx + 0.5, by), mv("B", ax - 0.5, ay)])

# A labelled second output on a tangent point. `avoidDoubleTangentPoint` drops
# the doubled point only when it is not labelled; `Intersect(f, k)` labels both
# outputs, so GeoGebra keeps the second point (equal to the first) defined.
# The line, or the second circle, is moved onto tangency, keeping the order.
topic("doubletangent")
# Whole numbers, so that the discriminant is exactly zero at tangency.
for n in range(6):
    ax, ay, r = rng.randint(-3, 3), rng.randint(-3, 3), rng.randint(2, 4)
    cx, dx = ax - 5, ax + 5
    y0 = ay + rng.randint(-1, 1)
    top = ay + r if n % 2 == 0 else ay - r
    case("doubletangent", [pt("A", ax, ay), f"r = {fmt(r)}", "k = Circle(A, r)",
                           pt("C", cx, y0), pt("D", dx, y0), "f = Line(C, D)",
                           "P = Intersect(f, k)"],
         [mv("C", cx, top), mv("D", dx, top)])
for n in range(6):
    ax, ay = rng.randint(-3, 3), rng.randint(-3, 3)
    r, s = rng.randint(2, 3), rng.randint(2, 3)
    sign = 1 if n % 2 == 0 else -1
    # externally tangent: centres r + s apart on the x axis through A
    case("doubletangent", [pt("A", ax, ay), f"r = {fmt(r)}", "k = Circle(A, r)",
                           pt("B", ax + sign * (r + s - 1), ay), f"s = {fmt(s)}",
                           "c = Circle(B, s)", "P = Intersect(k, c)"],
         [mv("B", ax + sign * (r + s), ay)])

# Angles saved under each angle unit, after moving the points.
topic("angle")
for unit in ("degree", "radiant"):
    for n in range(8):
        a = (r2(-3, 3), r2(-3, 3))
        b = (round(a[0] + rng.uniform(1, 4), 2), r2(-3, 3))
        c = (r2(-3, 3), round(a[1] + rng.uniform(1, 4), 2))
        steps = [mv("B", b[0] + r2(-1, 1), b[1] + r2(-1, 1)),
                 mv("C", c[0] + r2(-1, 1), c[1] + r2(-1, 1))]
        case("angle", [pt("A", *a), pt("B", *b), pt("C", *c),
                       "a = Angle(B, A, C)", "b = Angle(C, A, B)"], steps, unit)

expressions = [
    "A = (1, 2)", "B = (4, -1)", "v_{0x} = 2",
    "C = (x(A), y(B) + 2)", "s = Segment(A, C)",
    "φ = x(C) + π", "a = sin(30° + v_{0x}°)",
    "f(x) = If[-4 < x, v_{0x} ⋅ x² + y(C), x - 1]",
    "t(x) = If[f(x) > 0 ∧ x > 0, f(x)]",
    "g(x) = If[x ≤ -2 ∨ x ≥ 2, t(x) + 1, If[x ≠ 0, f(x), a]]",
    "n = If[x(A) ≥ 0, g(x(A)), φ]",
    "Q = (n, f(y(A)))", "valid = x(A) ≥ 0",
    "j_{xy}(x) = f(x) + 1", "d = j_{xy}(y(A))",
    "ang = (v_{0x} + 30)°",
    "bad = x(A) + Limit(x^2, v_{0x})", "lost = bad + 1",
    "multi(x, y) = x + y", "control = Checkbox()",
    "button = Button()", "field = InputBox(v_{0x})",
]
hidden = ["C", "s"]
case("expressions", expressions, [], hidden=hidden)
move = mv("A", 2, 1)
case("expressions", expressions, [move], hidden=hidden)
case("expressions", expressions,
     [move, {"action": "set_number", "label": "v_{0x}", "value": 5}], hidden=hidden)

paths = [
    "A = (1, 2)", "B = (9, 6)", "s = Segment(A, B)", "l = Line(A, B)",
    "f(x) = x² + y(A) - 3", "c = Circle(A, 3)", "S = Point(s)", "L = Point(l)",
    "F = Point(f)", "C = Point(c)", "D = Point(s, 0.25)", "u = Vector(S, L)",
    "d = Segment(F, C)", "p = PerpendicularLine(S, l)", "q = PerpendicularLine(L, l)",
    "ts = PathParameter(S)", "tl = PathParameter(L)", "tf = PathParameter(F)",
    "tc = PathParameter(C)",
]
dragged = [mv("S", 5, 5), mv("L", -3, 1), mv("F", 2.25, 3), mv("C", 2, -4),
           {"action": "animate", "label": "S"}]
case("paths", paths, dragged)
case("paths", paths, dragged + [mv("B", 7, 3), mv("A", -1, 1), mv("F", -2.5, 2)])

case("review", ["n = 400", "ang = (n + 30)°", "acute = (n - 10)°",
                "reflex = (n + 30)°", "raw = (n + 30)°", "turn = ang / 1°",
                "A = (sqrt(2), 1)", "B = (3, 4)", "C = (2, 5)",
                "a = 2π", "d = 1/3", "P = If(n > 0, A, B)",
                "q = {A, d, 5, a}", "flat(x) = 4", "tri = Polygon(A, B, C)",
                "len = 5", "el = Ellipse(A, B, len)",
                "e = x(A) + Limit(x^2, n)", "lost = e + 1", "Euler = ℯ + 1"],
     [{"action": "angle_style", "label": "ang", "value": 0},
      {"action": "angle_style", "label": "acute", "value": 1},
      {"action": "angle_style", "label": "reflex", "value": 2},
      {"action": "rename", "label": "P", "name": "chosen"}])

case("coordinates", ["a = 2", "b = 3", "P = $point(a, b)",
                     "Q = $point(1/2, 3)", "v = $vector(a, b)",
                     "w = $vector(1/2, 3)"], [])

inline = ["A = (1, 2)", "u = (2, -1)", "a = 3", "v = Vector(A, (4, 1))",
          "s = Segment(A, A + (2, 0))", "w = Vector(2u)",
          "d = UnitVector(2u)", "c = Circle(A, 2a)", "t = Tangent(a, x^2 + a)",
          "e1 = Segment(A, (x(u), y(u)))", "e2 = Segment(A, A + u)",
          # CommandsTest.cmdCircle
          "k = Circle((1, 1), 42)", "j = Circle((1, 1), (2, 1/2))"]
case("inline", inline, [])
case("inline", inline, [mv("A", 4, 1), {"action": "set_number", "label": "a", "value": 2}])

controls = ["A = (2, 1)", "B = (4, 3)", "a = Slider(0, 8, 0.5)",
            "s = Segment(A, B)", "check = Checkbox(\"Show\", {A, s})",
            "run = Button(\"Apply\")", "field = InputBox(a)", "empty = InputBox()",
            "pos = a > 2"]
edits = [{"action": "set_number", "label": "a", "value": 3},
         {"action": "condition", "label": "B", "expression": "a > 2"},
         {"action": "click_script", "label": "run", "text": "SetValue(a, 7)\nSetValue(a, 6)"}]
case("controls", controls, edits)
case("controls", controls, edits + [{"action": "set_boolean", "label": "check", "value": False}])

case("elements", ["A = (0, 0)", "B = (3, 0)", "C = (4, 2)",
                  "p = PolyLine({A, B, C})", "P = Point(p)", "Q = (x(P), y(P)^2)",
                  "trace = Locus(Q, P)", "f = Curve(cos(t), sin(t), t, 0, 2)",
                  "n = Length(f, 0, 1)"], [])

texts = [
    'marker = "Anchor label"',
    "P = (1.235, -2.246)", "a = 4.1256", "theta = 30.1256°",
    'caption = "Fixed panel #1 & notes"',
    'note = "Survey note\nsecond line"',
    'readout = "a = " + a + "; P = " + P + "; θ = " + theta',
    r'formula = "\frac{1}{2}\\\frac{3}{4}"',
    "turned = VerticalText(formula)",
    'hiddenText = "Hidden label"',
    'atPoint = Text("Placed by command", P, true, false, 0, 1)',
]
text_placement = [
    {"action": "text_screen", "label": "caption", "x": 51, "y": 73},
    {"action": "text_world", "label": "note", "x": -3, "y": 2},
    {"action": "text_anchor", "label": "marker", "point": "P"},
    {"action": "text_world", "label": "readout", "x": -1, "y": 3},
    {"action": "text_font", "label": "formula", "latex": True, "serif": True,
     "size": 1.4, "style": 3},
]
case("texts", texts, text_placement, hidden=["hiddenText"], archive=True)
case("texts", texts, text_placement + [mv("P", 3.456, 2.468),
     {"action": "set_number", "label": "a", "value": 8.5678}],
     hidden=["hiddenText"], archive=True)

text_equations = [
    "a = 2", "l: y = a x + 1", "p: y = a x^2 - x + 1",
    "A = (0, 1)", "B = (1, 3)", "e = Line(A, B)", "g = Line(A, B)", "r = Line(A, B)",
    "lText = Text(l)", "pTex = FormulaText(p)", "eText = Text(e)",
    "gText = Text(g)", "rText = Text(r)",
]
equation_styles = [
    {"action": "line_style", "label": "e", "form": "EXPLICIT"},
    {"action": "line_style", "label": "g", "form": "GENERAL"},
    {"action": "line_style", "label": "r", "form": "PARAMETRIC", "parameter": "u"},
]
case("text-equations", text_equations, equation_styles)
case("text-equations", text_equations, equation_styles + [
    {"action": "set_number", "label": "a", "value": -3},
])

conic_styles = [
    ("pe", "pe: y = x^2 - 4x + 7", "EXPLICIT"),
    ("pv", "pv: y = x^2 - 4x + 7", "VERTEX"),
    ("pc", "pc: x = 2y^2 + 8y + 11", "CONICFORM"),
    ("ps", "ps: y^2 = 4x", "SPECIFIC"),
    ("s", "s = Circle(C, 3)", "SPECIFIC"),
    ("cp", "cp = Circle(O, 3)", "PARAMETRIC"),
    ("ee", "ee: 4x^2 + 9y^2 = 36", "SPECIFIC"),
    ("h", "h: 4x^2 - 9y^2 = 36", "SPECIFIC"),
    ("di", "di: x^2 - y^2 = 0", "SPECIFIC"),
    ("dp", "dp: x^2 - y^2 = 0", "PARAMETRIC"),
    ("pp", "pp: (x - 1)^2 = 4", "PARAMETRIC"),
    ("pl", "pl: (x - 1)^2 = 0", "PARAMETRIC"),
    ("sp", "sp: x^2 + y^2 = 0", "PARAMETRIC"),
]
case("text-conic-styles",
     ["C = (1, -2)", "O = (0, 0)"] + [source for _, source, _ in conic_styles]
     + [f"{label}Text = Text({label})" for label, _, _ in conic_styles]
     + [f"{label}TeX = FormulaText({label})" for label, _, _ in conic_styles],
     [{"action": "conic_style", "label": label, "form": form}
      for label, _, form in conic_styles])

text_template = ["a = 3", 't = "value: " + a']
latex = {"action": "text_font", "label": "t", "latex": True, "size": 1, "style": 0}
case("text-template", text_template, [latex])
case("text-template", text_template,
     [latex, {"action": "set_number", "label": "a", "value": 3.141592653589793}])

case("slider-bounds", ["n = Slider(0, 10)", "m = Slider(0, 10)", "k = Slider(0, 10)", "a = 2", "q = 2n"],
     [{"action": "set_number", "label": "n", "value": 9},
      {"action": "slider_bounds", "label": "n", "max": "a + 3"},
      {"action": "slider_bounds", "label": "m", "min": "a - 1"},
      {"action": "slider_bounds", "label": "k", "min": "-3", "max": "Limit(x^2, a)"}])

image_points = ["A = (-2, -1)", "B = (1, 0)", "D = (-1, 2)"]
anchor_image = {"label": "anchoredImage", "file": "generated/tile.png",
                "corners": ["A", "B", "D"], "alpha": 0.5, "crop": [2, 1, 8, 6]}
image_edits = [
    {"action": "move", "label": "A", "x": -3, "y": 0},
    {"action": "move", "label": "D", "x": -2, "y": 3},
]
case("images", image_points, [], archive=True, images=[
    {"label": "fixedImage", "file": "generated/tile.png", "screen": [41, 63]},
    {"label": "worldImage", "file": "generated/tile.png", "corners": [[2, 1], [5, 1]]},
    anchor_image,
    {"label": "backImage", "file": "generated/tile.jpg", "corners": [[-5, 1], None, [-4, 3]],
     "background": True},
    {"label": "animatedImage", "file": "generated/animated.png", "format": "gif",
     "corners": [[6, 0]], "centered": True},
])
case("images", image_points, image_edits, archive=True, images=[anchor_image])

case("object-scripts", ["A = (1, 2)", "check = false", "a = 2"],
     [{"action": "condition", "label": "A", "expression": "check"},
      {"action": "click_script", "label": "A", "text": "SetValue(a, 7)"},
      {"action": "object_script", "label": "A", "event": "UPDATE",
       "language": "GGBSCRIPT", "text": "SetValue(a, 8)"},
      {"action": "object_script", "label": "A", "event": "DRAG_END",
       "language": "JAVASCRIPT", "text": 'ggbApplet.setValue("a", 9);'},
      {"action": "object_script", "label": "A", "event": "EDITOR_KEY_TYPED",
       "language": "JAVASCRIPT", "text": 'ggbApplet.setValue("a", 10);'}],
     hidden=["check"])

corners = ["P = Vertex(art, n)", "Q = Corner(art, 2)",
           "R = Vertex(art, 3)", "S = Vertex(art, 4)",
           "T = Vertex(markerImage, 3)", "U = Vertex(fixedImage, 1)"]
for steps in ([], image_edits, image_edits + [
    mv("B", -3, 0), {"action": "set_number", "label": "n", "value": 5}
]):
    case("vertex-image", image_points + ["n = 1.5"], steps, archive=True,
         images=[
             {"label": "art", "file": "generated/tile.png", "corners": ["A", "B", "D"]},
             {"label": "markerImage", "file": "generated/tile.png", "corners": ["A"]},
             {"label": "fixedImage", "file": "generated/tile.png", "screen": [40, 60]},
         ],
         after_images=corners)

curve_intersections = ["A = (-2, 1)", "B = (2, 1)", "g = Line(A, B)",
                       "q = Curve(t, t^2, t, -2, 2)", "r = Curve(s, 1, s, -2, 2)",
                       "P = Intersect(g, q)", "Q = Intersect(q, r, -1.5, 0)"]
case("curve-intersections", curve_intersections, [])
case("curve-intersections", curve_intersections, [mv("A", -2, 0), mv("B", 2, 0)])

clip_paths = ["O = (0, 0)", "k = Circle(O, 2)", "A = (-4, 0)", "B = (4, 0)",
              "C = (4, 1)", "D = (-4, 1)", "p = PolyLine(A, B, C, D)",
              "s = Segment(A, B)", "q = IntersectPath(p, k)", "t = IntersectPath(k, s)"]
case("conic-clip", clip_paths, [])
case("conic-clip", clip_paths, [mv("O", 0, 2)])

case("implicit", ["a = 2", "p: x^3/a + y^3 = 1", "q: x^4 + y^4 = 2"], [])


def st(label, **properties):
    return {"action": "style", "label": label, **properties}


# Presentation properties of every exported kind, set through the reference
# application's own setters, for the export round trip. Every fixture has the
# hidden numbers `r`, `g`, `b` for dynamic colours, the boolean `show` for
# conditions and the text `note` for dynamic captions. `styled` gives an
# object one value of each property of the named groups; the values change
# from object to object.
STYLE_HELPERS = ["r = 0.25", "g = 0.5", "b = 0.75", "show = true", 'note = "Dynamic"']
PATTERNS = ["HATCH", "CROSSHATCHED", "CHESSBOARD", "DOTTED", "HONEYCOMB", "BRICK", "WEAVING"]
styled_objects = 0


def styled(label, groups, **extra):
    global styled_objects
    styled_objects += 1
    n = styled_objects
    words = groups.split()
    props = {}
    if "common" in words or "control" in words:
        props.update(
            color=[(40 * n) % 256, (70 * n + 30) % 256, (110 * n + 60) % 256],
            layer=1 + n % 9, fixed=True, auxiliary=True, selection_allowed=False,
            tooltip_mode=1 + n % 3,
            dynamic_color=["r", "g", "b"] + (["1 - r"] if n % 2 else []),
            color_space=1 + n % 2,
            # In both graphics views, or in the second one only.
            views=2 + n % 2, ordering=0.5 + n, dynamic_caption="note",
            breakpoint=True, algebra_label=False, fixed_size=True)
    if "common" in words:
        props.update(label_mode=[1, 2, 3, 9][n % 4], caption=f"Caption {n}",
                     label_visible=False, label_offset=[5 + n, -7 - n])
    if "line" in words:
        props.update(line=[[10, 15, 20, 30][n % 4], 2 + n % 11],
                     line_opacity=40 + (17 * n) % 200, line_hidden=2 * (n % 2))
    if "fill" in words:
        # Every pattern keeps its angle and distance; only the symbol fill
        # keeps a symbol.
        symbol = n % 2 == 0
        props.update(alpha=round(0.15 + 0.07 * (n % 10), 2),
                     fill=["SYMBOLS" if symbol else PATTERNS[n % 7], 15 * (4 + n % 5), 11 + n % 20],
                     inverse_fill=True)
        if symbol:
            props["fill_symbol"] = "\u2605"
    if "point" in words:
        props.update(point=[1 + n % 4 + 5, 1 + n % 9], show_trimmed=True)
    if "trace" in words:
        props["trace"] = True
    props.update(extra)
    steps = [st(label, **{k: v for k, v in props.items() if v is not None})]
    if "common" in words or "control" in words:
        steps.append({"action": "condition", "label": label, "expression": "show"})
    return steps


def style_case(script, steps, hidden=(), **options):
    case("styles", STYLE_HELPERS + script, [step for group in steps for step in group],
         hidden=["r", "g", "b", "show", "note"] + list(hidden), **options)


# Points and straight objects. A second object of each kind is hidden.
style_case([
    "A = (1, 2)", "B = (4, 1)", "C = (0, 4)", "l = Line(A, B)", "P = Point(l)",
    "l2 = PerpendicularLine(C, l)", "s = Segment(A, C)", "s2 = Segment(B, C)",
    "h = Ray(B, C)", "h2 = Ray(C, B)", "v = Vector(A, B)", "v2 = Vector(B, A)",
    "k = Circle(A, 2)", "Q = Intersect(l, k, 1)", "M = Midpoint(A, B)",
], [
    styled("A", "common point trace"),
    styled("P", "common point trace"),
    styled("Q", "common point trace"),
    [st("M", point=[3, 10], trace=True, label_mode=2)],
    styled("l", "common line trace"),
    styled("s", "common line trace", decoration=2, ends=["ARROW", "CIRCLE_OUTLINE"]),
    styled("h", "common line trace"),
    styled("v", "common line trace", head_style=1),
    [st("l2", line=[0, 13], line_opacity=255), st("s2", decoration=5)],
], hidden=["B", "l2", "s2", "h2", "v2"])

# Circles, conics and their parts.
style_case([
    "M = (0, 0)", "A = (4, 0)", "B = (0, 4)", "C = (-4, 0)", "k = Circle(M, A)",
    "k2 = Circle(M, 2)", "len = 9", "e = Ellipse(A, C, len)", "e2 = Ellipse(A, B, len)",
    "a = CircularArc(M, A, B)", "a2 = CircumcircularArc(A, B, C)",
    "d = CircularSector(M, A, B)", "d2 = CircumcircularSector(A, B, C)",
    "s = Semicircle(A, C)",
], [
    styled("k", "common line fill trace"),
    styled("k2", "line fill"),
    styled("e", "common line fill trace"),
    styled("e2", "line fill"),
    styled("a", "common line fill trace"),
    styled("a2", "line fill"),
    styled("d", "common line fill trace"),
    styled("d2", "line fill"),
    [st("s", alpha=0.1, line=[0, 9])],
], hidden=["len", "k2", "e2", "a2", "d2"])

# Numbers, sliders, angles and booleans. A visible number or angle cannot be
# fixed, so a hidden one carries that flag.
style_case([
    "A = (0, 0)", "B = (4, 0)", "C = (1, 3)", "a = Slider(-3, 3, 0.25)",
    "a2 = Slider(1, 5, 0.5)", "n = 4", "speed = 2", "w = Angle(A, B, C)",
    "w2 = Angle(C, B, A)", "t = (10 n)\u00b0", "check = Checkbox()", "flag = true",
    "lock = false", "pos = a > 2",
], [
    styled("a", "common line", fixed=None,
           slider={"position": [120, 80], "width": 150, "horizontal": False,
                   "fixed": True, "show_algebra": False},
           animation={"speed": "speed", "mode": 1}, knob_size=7.5),
    # The speed is arithmetic over the construction and the slider is playing.
    [st("a2", slider={"absolute": False, "position": [2, -3], "width": 4.5},
        animation={"step": "0.125", "speed": "speed / 4", "mode": 3, "playing": True},
        label_mode=9)],
    styled("n", "common"),
    styled("w", "common line fill", fixed=None, arc_size=45,
           emphasize_right_angle=False, decoration=2),
    [{"action": "angle_style", "label": "w", "value": 2},
     st("w2", arc_size=20, decoration=4, alpha=0.4, fixed=True),
     {"action": "angle_style", "label": "w2", "value": 1}],
    styled("check", "common", fixed=None, checkbox_fixed=True),
    styled("flag", "common", fixed=None),
    # A boolean with a condition cannot be fixed.
    [st("lock", fixed=True)],
], hidden=["speed", "n", "w2", "flag", "lock", "pos"])

# Functions, curves and loci.
style_case([
    "f(x) = x^2 - 2", "f2(x) = sin(x)", "j(x, y) = x + y", "j2(x, y) = x y",
    "c = Curve(t^2, t (t^2 - 1), t, -2, 2)", "c2 = Curve(t, t^3, t, -1, 1)",
    "A = (0, 0)", "B = (3, 0)", "C = (4, 2)", "p = PolyLine({A, B, C})",
    "P = Point(p)", "Q = (x(P), y(P)^2)", "path = Locus(Q, P)",
    "R = (y(P), x(P))", "path2 = Locus(R, P)",
], [
    styled("f", "common line trace"),
    styled("j", "common line fill"),
    styled("c", "common line trace"),
    styled("path", "common line trace"),
    [st("f2", line=[30, 1]), st("c2", line=[20, 12], line_opacity=30)],
], hidden=["f2", "j2", "c2", "path2"])

# Polygons, broken lines, lists and charts. A chart cannot be fixed.
style_case([
    "A = (0, 0)", "B = (4, 0)", "C = (1, 3)", "tri = Polygon(A, B, C)",
    "sq = Polygon(A, B, 4)", "open = PolyLine(A, B, C)", "open2 = PolyLine(C, A, B)",
    "nums = {1, 2, 3}", "pts = {A, B, (2, 2)}", "chart = PieChart({1, 2, 3})",
    "chart2 = PieChart({2, 5})",
], [
    styled("tri", "common line fill trace"),
    styled("sq", "line fill"),
    styled("open", "common line trace", ends=["SQUARE", "DIAMOND_OUTLINE"]),
    styled("nums", "common"),
    styled("pts", "common line fill point trace", fill=["SYMBOLS", 75, 12],
           fill_symbol="\u2665"),
    styled("chart", "common line fill", fixed=None),
    [st("open2", line=[15, 1])],
], hidden=["sq", "open2", "nums", "chart2"])

# Texts. Their own rounding leaves the whole numbers of `tally` and `total`
# as they are, so the kernel prints the same text.
style_case([
    "n = 3.14159265", "P = (2, 1)", 'title = "Styled title"',
    'value = "n = " + n', 'boxed = "Framed"', 'plain = "Plain"',
    "count = 4", 'tally = "count: " + count', 'total = "total: " + 31 count',
    'hiddenText = "Hidden"',
], [
    [{"action": "text_screen", "label": "title", "x": 40, "y": 60},
     {"action": "text_font", "label": "title", "latex": False, "serif": True,
      "size": 2, "style": 1}],
    styled("title", "control", background=[255, 240, 200, 255], auxiliary=False),
    [{"action": "text_world", "label": "value", "x": -2, "y": 3},
     {"action": "text_font", "label": "value", "latex": False, "serif": False,
      "size": 0.7, "style": 2},
     st("value", color=[160, 0, 0], background=[10, 20, 30, 128]),
     {"action": "text_anchor", "label": "boxed", "point": "P"},
     {"action": "text_font", "label": "boxed", "latex": True, "serif": True,
      "size": 1.4, "style": 3},
     st("boxed", layer=9, label_offset=[6, -4]),
     st("plain", color=[0, 0, 0]),
     st("tally", decimals=4),
     st("total", figures=3)],
], hidden=["n", "count", "hiddenText"])

# Buttons and input boxes.
style_case([
    "a = Slider(0, 8, 0.5)", "run = Button(\"Apply\")", "run2 = Button(\"Reset\")",
    "field = InputBox(a)", "empty = InputBox()",
], [
    styled("run", "control", background=[0, 90, 180, 255], caption="Go",
           label_visible=False),
    styled("field", "control", background=[230, 255, 230, 255], caption="Value",
           auxiliary=False, label_visible=False),
    [st("empty", tooltip_mode=2)],
], hidden=["run2", "empty"])

# Images.
style_case(image_points, [
    styled("art", "control", color=None, dynamic_color=None, color_space=None,
           auxiliary=False, caption="Logo"),
    [st("pinned", layer=1)],
], hidden=["pinned"], archive=True, images=[
    {"label": "art", "file": "generated/tile.png", "corners": ["A", "B", "D"], "alpha": 0.5},
    {"label": "pinned", "file": "generated/tile.png", "screen": [41, 63]},
])

# Styled objects that are undefined when the file is saved: the circle is
# dragged off the line, and the intersection with what depends on it goes.
style_case([
    "A = (0, 0)", "B = (4, 0)", "C = (2, 0.5)", "l = Line(A, B)", "k = Circle(C, 1)",
    "Q = Intersect(l, k, 1)", "s = Segment(A, Q)", "M = Midpoint(A, Q)",
], [
    styled("Q", "common point trace"),
    styled("s", "common line trace", decoration=3),
    styled("M", "common point trace"),
    [mv("C", 2, 5)],
])

# Implicit curves.
style_case([
    "p: x^3 + y^3 = 1", "q: x^4 + y^4 = 2",
], [
    styled("p", "common line trace"),
    [st("q", line=[20, 6], line_opacity=90)],
], hidden=["q"])

# Sliders whose value lies outside the interval the reference application
# gives a number that becomes visible (-5 to 5, an angle 0 to 360 degrees).
style_case([
    "n = Slider(0, 100, 1)", "m = Slider(-200, -100, 10)", "P = (n / 10, m / 100)",
], [
    [{"action": "set_number", "label": "n", "value": 30},
     {"action": "set_number", "label": "m", "value": -170},
     st("n", slider={"width": 150}, label_visible=False)],
])

# Dynamic colour channels and an animation step and speed whose inputs are
# undefined when the file is saved: the intersection `Q` is gone after the
# move and `sqrt(a)` has no value once `a` is negative.
style_case([
    "A = (0, 0)", "B = (4, 0)", "C = (2, 0.5)", "l = Line(A, B)", "k = Circle(C, 1)",
    "Q = Intersect(l, k, 1)", "a = 1", "P = (1, 2)", "n = Slider(0, 10, 1)",
], [
    [st("P", dynamic_color=["x(Q) / 10", "sqrt(a)", "b"], color_space=0),
     st("n", animation={"step": "sqrt(a)", "speed": "x(Q) + 1", "mode": 1})],
    [{"action": "set_number", "label": "a", "value": -1}, mv("C", 2, 5)],
], hidden=["a"])

# Pen strokes with presentation properties. The second stroke is hidden.
style_case([
    "s = PenStroke((0, 0), (1, 2), (3, 1), (4, 3))", "s2 = PenStroke((1, 1), (2, 2))",
], [
    styled("s", "common line trace", draw_arrows=True),
    [st("s2", line=[20, 9], line_opacity=60)],
], hidden=["s2"])

# Pen strokes: the file keeps the smoothed points with their segment codes,
# each number in five significant digits; a stroke without points is saved
# without them.
topic("strokes")
case("strokes", [
    "s = PenStroke((-3, 3), (4, 3), (2, 5))",
    "t = PenStroke((0, 0), (1, 1), (?, ?), (2, 0), (3, 1), (4, 0))",
    "u = PenStroke((1, 3))",
    "v = PenStroke()",
    "w = PenStroke((?, ?))",
    "q = PenStroke(" + ", ".join(f"({fmt(0.03 * i)}, {fmt(0.02 * i * i % 1.7)})" for i in range(14)) + ")",
], [])

# SlowPlot hides the source graph and plays a new slider from 0 to 1, again
# and again or once; the file keeps that state.
topic("slowplot")
case("slowplot", [
    "f(x) = sin(x)", "g = SlowPlot(f)", "A = (0, 0)", "B = (2, 1)", "l = Line(A, B)",
    "h = SlowPlot(l, false)", "k = SlowPlot(x^2, 0)",
], [])

topic("keptvertices")
case("keptvertices", [
    "n = 1", "U = (0, 0)", "A = If(n > 0, U)", "B = (2, 0)",
    "nv = 5", "p = Polygon(A, B, nv)",
], [{"action": "set_number", "label": label, "value": value}
    for label, value in [("n", 0), ("nv", 3), ("nv", 5)]])

topic("keptvalues")
case("keptvalues", [
    "a = 1", "l = If(a > 0, {{a, 2}}, {{a, 2}, {3, 4}})", "P = Point(l)",
], [{"action": "set_number", "label": "a", "value": value} for value in (0, 1)])
case("keptvalues", [
    "A = (0, 0)", "B = (3, 0)", "r = Distance(A, B)", "k = Circle(A, r)",
    "C = (-5, 1)", "D = (5, 1)", "g = Line(C, D)", "X = Intersect(g, k, 1)",
    "P = Intersect(k, k, 1)", "Q = Intersect(k, k, 2)",
], [mv("C", -5, 2)])

case("keptvalues", [
    "A = (0, 0)", "B = (3, 0)", "r = Distance(A, B)", "k = Circle(A, r)",
    "C = (-5, 1)", "D = (5, 1)", "g = Line(C, D)", "X = Intersect(g, k, 1)",
    "Q = Intersect(k, k, 2)", "P = Intersect(k, k, 1)",
], [mv("C", -5, 2)])

out = Path(__file__).with_name("saved.json")
lines = ",\n".join("    " + json.dumps(c, separators=(", ", ": ")) for c in cases)
out.write_text('{\n  "cases": [\n' + lines + "\n  ]\n}\n")
print(f"{len(cases)} cases written to {out.name}")
