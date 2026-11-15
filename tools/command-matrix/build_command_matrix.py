#!/usr/bin/env python3
"""Build docs/command-matrix.csv from GeoGebra's `Commands` enum.

Usage (run from the repository root):
    python3 -I tools/command-matrix/build_command_matrix.py \
        --source <geogebra checkout> --out docs/command-matrix.csv [--check]

Only Commands.java and CommandsConstants.java are read. The classification
rules are documented in tools/command-matrix/README.md.
"""
import argparse
import csv
import io
import re
import sys
from pathlib import Path

COMMANDS_DIR = Path("source/shared/common/src/main/java/org/geogebra/common/kernel/commands")
COLUMNS = ["command", "alias_of", "group", "priority", "status", "notes"]

OUT_OF_SCOPE_GROUPS = {"3d", "spreadsheet", "probability", "cas", "scripting", "statistics"}
P1_GROUPS = {"geometry", "conic", "transformation"}
# Groups the rule knows how to place. Anything else becomes todo/P2 and is reported.
KNOWN_GROUPS = OUT_OF_SCOPE_GROUPS | P1_GROUPS | {
    "algebra", "text", "logical", "function", "list", "vector", "charts", "discrete",
    "geogebra", "optimization", "financial",
}

# The only hand-listed exceptions: commands the kernel covers in part
# (derived from crates/geo-kernel/src/script.rs and the group files in
# crates/geo-kernel/src/algo/). The note lists the supported forms.
PARTIAL = {
    "Line": "Line(point, point); Line(point, line or segment); Line(point, vector)",
    "Segment": "Segment(point, point)",
    "Midpoint": "Midpoint(point, point); Midpoint(segment); Midpoint(circle)",
    "Distance": "Distance(point, point); Distance(point, line or segment); Distance(line, line)",
    "OrthogonalLine": "OrthogonalLine(point, line or segment); OrthogonalLine(point, vector)",
    "Circle": "Circle(centre, point); Circle(centre, radius); Circle(centre, segment); "
              "Circle(point, point, point)",
    "Intersect": "line/line; line/circle; circle/circle; optional index 1 or 2",
    "LineBisector": "LineBisector(point, point); LineBisector(segment)",
    "AngularBisector": "AngularBisector(point, point, point); AngularBisector(line, line)",
    "Tangent": "Tangent(point, circle); Tangent(line or segment, circle)",
    "Polar": "Polar(point, circle)",
    "Incircle": "Incircle(point, point, point)",
    "Center": "Center(circle); Center(segment)",
    "Diameter": "Diameter(line or segment, circle)",
    "Length": "Length(segment); Length(vector)",
    "Radius": "Radius(circle)",
    "Circumference": "Circumference(circle)",
    "Perimeter": "Perimeter(circle)",
    "Area": "Area(circle); Area of three or four points",
    "Slope": "Slope(line or segment)",
    "AffineRatio": "AffineRatio(point, point, point)",
    "CrossRatio": "CrossRatio(point, point, point, point)",
    "Angle": "Angle(point, point, point)",
    "Mirror": "point, line, segment or circle in a point or a line; point in a circle",
    "Rotate": "point, line, segment or circle by an angle, about the origin or a point",
    "Dilate": "point, line, segment or circle by a factor, from the origin or a point",
    "Shear": "point, line or segment with a line and a ratio",
    "Stretch": "point, line or segment with a line and a ratio",
    "Vector": "Vector(point, point); Vector(point); a free (x, y) under a lower-case label",
    "Ray": "Ray(point, point); Ray(point, vector)",
    "Translate": "point, line, segment, circle, ray or vector by a vector",
    "Direction": "Direction(line, segment or ray)",
    "UnitVector": "UnitVector(vector, line, segment or ray)",
    "OrthogonalVector": "OrthogonalVector(vector, line, segment or ray)",
    "UnitOrthogonalVector": "UnitOrthogonalVector(vector, line, segment or ray)",
}

# Commands the first version of the kernel already had: priority P0.
FIRST_KERNEL = {"Line", "Segment", "Midpoint", "Distance", "OrthogonalLine", "Circle", "Intersect"}


def strip_comments(text):
    """Remove // and /* */ comments, keeping string literals intact."""
    out, i, n = [], 0, len(text)
    while i < n:
        two = text[i:i + 2]
        if two == "//":
            while i < n and text[i] != "\n":
                i += 1
        elif two == "/*":
            end = text.find("*/", i + 2)
            i = n if end < 0 else end + 2
        elif text[i] == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            out.append(text[i:j + 1])
            i = j + 1
        else:
            out.append(text[i])
            i += 1
    return "".join(out)


def split_enum_entries(text):
    """Return (name, args) for each constant of the `Commands` enum, in order."""
    text = strip_comments(text)
    m = re.search(r"\benum\s+Commands\b[^{]*\{", text)
    if not m:
        raise ValueError("enum Commands not found")
    body, depth, start, chunks = text[m.end():], 0, 0, []
    end = len(body)
    for i, ch in enumerate(body):
        if ch in "({":
            depth += 1
        elif ch in ")}":
            depth -= 1
        elif depth == 0 and ch in ",;":
            chunks.append(body[start:i])
            start = i + 1
            if ch == ";":
                end = i
                break
    del end
    entries = []
    for chunk in chunks:
        chunk = chunk.strip()
        if not chunk:
            continue
        em = re.fullmatch(r"(\w+)\s*(?:\((.*)\))?", chunk, re.S)
        if not em:
            raise ValueError("cannot parse enum entry: %r" % chunk)
        entries.append((em.group(1), (em.group(2) or "").strip()))
    return entries


def parse_aliases(text):
    """Map alias -> target from the switch in englishToInternal()."""
    text = strip_comments(text)
    m = re.search(r"englishToInternal\s*\([^)]*\)\s*\{", text)
    if not m:
        return {}
    start, depth, end = m.end(), 1, None
    for i in range(start, len(text)):
        depth += {"{": 1, "}": -1}.get(text[i], 0)
        if depth == 0:
            end = i
            break
    body = text[start:end]
    aliases, pending = {}, []
    for label in re.finditer(r"case\s+(\w+)\s*:|return\s+(\w+)\s*;", body):
        if label.group(1):
            pending.append(label.group(1))
        else:
            for name in pending:
                aliases[name] = label.group(2)
            pending = []
    return aliases


def parse_table_constants(text):
    """Map TABLE_X constant name to its group name (lower case, TABLE_ stripped)."""
    names = re.findall(r"\bint\s+(TABLE_\w+)\s*=", strip_comments(text))
    return {n: n[len("TABLE_"):].lower() for n in names}


def resolve_aliases(aliases, names):
    """Follow alias chains so a target is always a real command."""
    resolved = {}
    for alias, target in aliases.items():
        seen = {alias}
        while target in aliases and target not in seen:
            seen.add(target)
            target = aliases[target]
        if alias in names and target in names and target != alias:
            resolved[alias] = target
    return resolved


def classify(group, command, alias_of):
    """Return (priority, status, notes) for a real command; aliases copy their target."""
    if command in PARTIAL and not alias_of:
        if command in FIRST_KERNEL:
            priority = "P0"
        else:
            priority = "P1" if group in P1_GROUPS else "P2"
        return priority, "partial", PARTIAL[command]
    if group in OUT_OF_SCOPE_GROUPS:
        return "", "out_of_scope", ""
    if group in P1_GROUPS:
        return "P1", "todo", ""
    return "P2", "todo", ""


def build_rows(commands_java, constants_java):
    tables = parse_table_constants(constants_java)
    entries = split_enum_entries(commands_java)
    names = {name for name, _ in entries}
    aliases = resolve_aliases(parse_aliases(commands_java), names)
    group_of = {}
    for name, args in entries:
        group_of[name] = tables.get(args, args.lower() or "unknown")
    rows, unclassified = {}, set()
    for name, _ in entries:
        target = aliases.get(name, "")
        real = target or name
        group = group_of[real]
        if group not in KNOWN_GROUPS:
            unclassified.add(group)
        if target:
            continue
        rows[name] = (group,) + classify(group, name, "")
    out = []
    for name, _ in entries:
        target = aliases.get(name, "")
        real = target or name
        group, priority, status, notes = rows[real]
        note = notes if not target else ""
        out.append([name, target, group, priority, status, note])
    declared = {}
    for _, args in entries:
        key = tables.get(args, args.lower() or "unknown")
        declared[key] = declared.get(key, 0) + 1
    return out, unclassified, declared


def render_csv(rows):
    buf = io.StringIO(newline="")
    writer = csv.writer(buf, lineterminator="\n")
    writer.writerow(COLUMNS)
    writer.writerows(rows)
    return buf.getvalue()


def summary(rows, unclassified, declared):
    lines = ["total entries: %d" % len(rows),
             "aliases: %d" % sum(1 for r in rows if r[1])]
    for title, idx in (("group", 2), ("status", 4), ("priority", 3)):
        counts = {}
        for r in rows:
            counts[r[idx] or "(none)"] = counts.get(r[idx] or "(none)", 0) + 1
        lines.append("by %s: %s" % (title, ", ".join(
            "%s=%d" % kv for kv in sorted(counts.items()))))
    own = {}
    for r in rows:
        if not r[1]:
            own[r[2]] = own.get(r[2], 0) + 1
    lines.append("by group, real commands only: " + ", ".join(
        "%s=%d" % kv for kv in sorted(own.items())))
    lines.append("by declared table constant (aliases counted where declared): " + ", ".join(
        "%s=%d" % kv for kv in sorted(declared.items())))
    lines.append("plan figures: 553 entries, 53 aliases, geometry 58, conic 20, transformation 6")
    lines.append("groups not classified (todo/P2): " + (", ".join(sorted(unclassified)) or "none"))
    return "\n".join(lines)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--source", required=True, help="GeoGebra checkout root")
    ap.add_argument("--out", required=True, help="CSV file to write")
    ap.add_argument("--check", action="store_true",
                    help="exit 1 if the CSV on disk differs from the generated one")
    args = ap.parse_args(argv)
    base = Path(args.source) / COMMANDS_DIR
    commands_java = (base / "Commands.java").read_text(encoding="utf-8")
    constants_java = (base / "CommandsConstants.java").read_text(encoding="utf-8")
    rows, unclassified, declared = build_rows(commands_java, constants_java)
    text = render_csv(rows)
    print(summary(rows, unclassified, declared))
    out = Path(args.out)
    if args.check:
        current = out.read_text(encoding="utf-8") if out.exists() else None
        if current != text:
            print("%s is out of date; regenerate it" % out, file=sys.stderr)
            return 1
        print("%s is up to date" % out)
        return 0
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8", newline="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
