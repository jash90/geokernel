"""Helpers for the make_<group>.py scripts that write cases/<group>.json.

The inputs only describe what to run. Snapshots come exclusively from running
GeoGebra through the harness (`harness.sh golden`). The output is
deterministic: a fixed seed and a fixed order, so case ids stay stable as long
as existing topics are only appended to.
"""

import json
import random
from pathlib import Path


def fmt(v):
    """Plain decimal literal for a script line (no exponent notation)."""
    v = round(float(v), 6)
    if v == int(v):
        return str(int(v))
    return ("%.6f" % v).rstrip("0")


def pt(label, x, y):
    return f"{label} = ({fmt(x)}, {fmt(y)})"


def num(label, v):
    return f"{label} = {fmt(v)}"


def mv(label, x, y):
    return {"action": "move", "label": label, "x": float(round(x, 6)), "y": float(round(y, 6))}


def setn(label, v):
    return {"action": "set_number", "label": label, "value": float(v)}


class Cases:
    """The cases of one group, written to cases/<group>.json."""

    def __init__(self, group):
        self.group = group
        self.rng = random.Random()
        self.cases = []
        self.counters = {}

    def case(self, topic, script, steps):
        self.counters[topic] = self.counters.get(topic, 0) + 1
        self.cases.append({
            "id": f"{self.group}-{topic}-{self.counters[topic]:03d}",
            "group": self.group,
            "script": script,
            "steps": [{"action": "initial"}] + steps,
        })

    def topic(self, name):
        """Starts a topic: re-seeds so each topic's values do not depend on others.

        New cases go at the end of their topic so existing ids keep their meaning.
        """
        self.rng.seed(f"{self.group}-{name}", version=2)

    def r2(self, lo=-8.0, hi=8.0):
        return round(self.rng.uniform(lo, hi), 2)

    def rp(self, lo=-8.0, hi=8.0):
        return (self.r2(lo, hi), self.r2(lo, hi))

    def write(self, directory=None):
        out = Path(__file__).with_name(f"{self.group}.json")
        if directory is not None:
            out = out.parent / directory / out.name
            out.parent.mkdir(exist_ok=True)
        lines = ",\n".join("    " + json.dumps(c, separators=(", ", ": ")) for c in self.cases)
        out.write_text('{\n  "cases": [\n' + lines + "\n  ]\n}\n")
        print(f"{len(self.cases)} cases written to {out.name}")
