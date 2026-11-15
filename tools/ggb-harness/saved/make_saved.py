#!/usr/bin/env python3
"""Writes saved/saved.json: constructions that are saved after their free
points were moved.

Each case gives raw GeoGebra input lines, `move` steps, and optionally the
angle unit of the kernel. The harness (`harness.sh saved`) runs the lines, moves
the points, and writes the construction file together with GeoGebra's own values
at that moment, so the importer can be checked against the state the file was
saved from: the order of the outputs of a command that makes several objects
(`Intersect` of a line and a circle, of two circles; `Tangent`) after the
points moved, and angles saved under each angle unit. The input is
deterministic (fixed seeds); new cases are appended to their topic.
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


def case(name, script, steps, unit=None):
    counters[name] = counters.get(name, 0) + 1
    entry = {"id": f"saved-{name}-{counters[name]:03d}"}
    if unit:
        entry["angle_unit"] = unit
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

out = Path(__file__).with_name("saved.json")
lines = ",\n".join("    " + json.dumps(c, separators=(", ", ": ")) for c in cases)
out.write_text('{\n  "cases": [\n' + lines + "\n  ]\n}\n")
print(f"{len(cases)} cases written to {out.name}")
