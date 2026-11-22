#!/usr/bin/env python3
"""Writes saved/saved.json: constructions that are saved after their free
points were moved.

Each case gives raw GeoGebra input lines and `move` or `set_number` steps.
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


def case(name, script, steps, unit=None, hidden=None):
    counters[name] = counters.get(name, 0) + 1
    entry = {"id": f"saved-{name}-{counters[name]:03d}"}
    if unit:
        entry["angle_unit"] = unit
    if hidden:
        entry["hidden"] = hidden
    entry["script"] = script
    entry["steps"] = steps
    cases.append(entry)


def r2(lo=-6.0, hi=6.0):
    return round(rng.uniform(lo, hi), 2)


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
    "bad = x(A) + NextPrime(v_{0x})", "lost = bad + 1",
    "multi(x, y) = x + y", "control = Checkbox()",
    "button = Button()", "field = InputBox(v_{0x})",
]
hidden = ["C", "s"]
case("expressions", expressions, [], hidden=hidden)
move = mv("A", 2, 1)
case("expressions", expressions, [move], hidden=hidden)
case("expressions", expressions,
     [move, {"action": "set_number", "label": "v_{0x}", "value": 5}], hidden=hidden)

out = Path(__file__).with_name("saved.json")
lines = ",\n".join("    " + json.dumps(c, separators=(", ", ": ")) for c in cases)
out.write_text('{\n  "cases": [\n' + lines + "\n  ]\n}\n")
print(f"{len(cases)} cases written to {out.name}")
