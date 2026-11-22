#!/usr/bin/env python3
"""Build docs/command-matrix.csv from GeoGebra's `Commands` enum.

Usage (run from the repository root):
    python3 -I tools/command-matrix/build_command_matrix.py \
        --source <geogebra checkout> --out docs/command-matrix.csv [--check]

Only Commands.java and CommandsConstants.java are read upstream. The form
counts come from tools/command-matrix/forms.csv and the done forms from
tools/command-matrix/form-evidence.toml. The classification rules are
documented in tools/command-matrix/README.md.
"""
import argparse
import csv
import io
import re
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent

COMMANDS_DIR = Path("source/shared/common/src/main/java/org/geogebra/common/kernel/commands")
COLUMNS = ["command", "alias_of", "group", "priority", "status", "forms_total", "forms_done",
           "notes"]
FORMS_COLUMNS = ["command", "group", "forms_2d", "forms"]

OUT_OF_SCOPE_GROUPS = {"3d", "spreadsheet", "probability", "cas", "scripting", "statistics"}
# Single commands outside the 2D scope although their table is in scope:
# a statistics pair (var, cov), 3D objects (Polyhedron, CornerThreeD), a
# command of the CAS view (Evaluate) and whiteboard-only objects (Stadium,
# TableToChart).
OUT_OF_SCOPE_COMMANDS = {"var", "cov", "Polyhedron", "CornerThreeD", "Evaluate", "Stadium",
                         "TableToChart"}
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
    "Locus": "Tracing point with a movable path point or slider, slope function with initial point, or slope field with initial point",
    "Line": "Line(point, point); Line(point, line, segment or ray); Line(point, vector)",
    "Segment": "Segment(point, point); Segment(point, length); Segment(point, point, polygon) creates no output",
    "Midpoint": "Midpoint(point, point); Midpoint(segment); Midpoint(circle or conic)",
    "Distance": "Distance(point, point); point and path in either order (line, segment, ray, conic, arc, sector, polygon, polyline, function or point list); two line carriers",
    "OrthogonalLine": "OrthogonalLine(point, line, segment or ray); OrthogonalLine(point, vector); OrthogonalLine(point, conic or conic part)",
    "Circle": "Circle(centre, point); Circle(centre, radius); Circle(centre, segment); "
              "Circle(point, point, point)",
    "Intersect": "line/line; line/circle; circle/circle; line/conic; optional index 1 or 2",
    "LineBisector": "LineBisector(point, point); LineBisector(segment)",
    "AngularBisector": "AngularBisector(point, point, point); AngularBisector(line, line)",
    "Tangent": "Tangent(point, circle or conic); Tangent(line or segment, circle or conic)",
    "Polar": "Polar(point, circle or conic)",
    "Incircle": "Incircle(point, point, point)",
    "Center": "Center(circle or conic); Center(segment)",
    "Diameter": "Diameter(line or segment, circle or conic)",
    "Length": "Point, vector, segment, list, quoted text, locus sample count, conic part, function or Cartesian curve between bounds",
    "Radius": "Radius(conic or conic part); defined for circle and single-point types",
    "Circumference": "Conic, conic part, polygon or locus",
    "Perimeter": "Polygon, conic, conic part or locus",
    "Area": "Area(conic, arc, sector or polygon); Area of three or more points",
    "Point": "Coordinate list, point and vector, or a line, segment, ray, circle, conic, polygon, polyline, arc or sector with an optional path parameter",
    "PathParameter": "PathParameter(point on a path)",
    "CircleArc": "CircularArc(centre, point, point)",
    "CircleSector": "CircularSector(centre, point, point)",
    "CircumcircleArc": "CircumcircularArc(point, point, point)",
    "CircumcircleSector": "CircumcircularSector(point, point, point)",
    "Semicircle": "Semicircle(point, point)",
    "Arc": "Conic with point endpoints or numeric parameters",
    "Sector": "Conic with point endpoints or numeric parameters",
    "Slope": "Slope(line, segment, ray or function)",
    "AffineRatio": "AffineRatio(point, point, point)",
    "CrossRatio": "CrossRatio(point, point, point, point)",
    "Cross": (
        "point or vector pairs and flat list pairs of equal length 2 or 3 "
        "(vectorforms-listcross-001 and vectorforms-listcross-002)"
    ),
    "Angle": (
        "Angle(angle); Angle(number); Angle(point); Angle(vector); Angle(conic); Angle(polygon); "
        "Angle(point, point); Angle(point, vector); Angle(vector, point); Angle(vector, vector); "
        "Angle(line, line); Angle(point, point, point); Angle(point, point, number)"
    ),
    "Mirror": "point, line, segment, ray, vector, polygon, polyline, circle, conic, arc or sector in a point or a line; point, line, segment, circle, arc or sector in a circle (inversion)",
    "Rotate": "point, line, segment, ray, vector, polygon, polyline, circle, conic, arc or sector by an angle, about the origin or a point",
    "Dilate": "point, line, segment, ray, vector, polygon, polyline, circle, conic, arc or sector by a factor, from the origin or a point",
    "Shear": "point, line, segment, ray, vector, polygon, circle, conic, arc or sector with a line and a ratio",
    "Stretch": "point, line, segment, ray, vector, polygon, circle, conic, arc or sector with a line and a ratio, or by a vector",
    "Vector": "Vector(point, point); Vector(point); a free (x, y) under a lower-case label",
    "Ray": "Ray(point, point); Ray(point, vector)",
    "Translate": "point, line, segment, circle, conic, ray, vector, polygon, polyline, arc or sector by a vector",
    "Direction": "Direction(line, segment or ray)",
    "UnitVector": "UnitVector(vector, line, segment or ray)",
    "OrthogonalVector": "OrthogonalVector(vector, line, segment or ray)",
    "UnitOrthogonalVector": "UnitOrthogonalVector(vector, line, segment or ray)",
    "AreCollinear": "AreCollinear(point, point, point)",
    "AreParallel": "AreParallel of two lines, segments or rays",
    "ArePerpendicular": "ArePerpendicular of two lines, segments or rays",
    "AreConcurrent": "AreConcurrent of three lines, segments or rays",
    "AreConcyclic": "AreConcyclic(point, point, point, point)",
    "AreEqual": "Numeric equality by object kind, including cyclic vertex equality and polynomial equality through degree four",
    "AreCongruent": "Numeric congruence by object kind, including cyclic or reversed polygon congruence",
    "IsTangent": "IsTangent(line, segment or ray, conic or conic part)",
    "ClosestPoint": "Path and point in either order (line, segment, ray, conic, arc, sector, polygon, polyline, function or point list); two line carriers except line with segment",
    "Trilinear": "Trilinear(point, point, point, number, number, number)",
    "Polygon": "Live list of points, separate points, or two points and a regular polygon vertex count",
    "PolyLine": "Live list with optional Boolean or separate points with an optional false flag",
    "Centroid": "Centroid(polygon)",
    "Vertex": "Conic, polygon, polyline or a kernel graphics corner, including indexed polygon, polyline and segment forms",
    "InteriorAngles": "InteriorAngles(polygon)",
    "If": "If(condition, then); If(condition, then, else); If(condition, value, condition, value, ..., else)",
    "Element": "Element(list, index)",
    "Sum": "Sum(list); Sum(list, number of elements); numbers, points or vectors",
    "Product": "Product(list); Product(list, number of elements)",
    "Mean": "Mean(list)",
    "Sequence": "Sequence(end); Sequence(start, end); Sequence(start, end, step); "
                "Sequence(expression, variable, start, end); Sequence(expression, variable, start, end, step)",
    "Reverse": "Reverse(list)",
    "First": "First(list); First(list, number of elements)",
    "Last": "Last(list); Last(list, number of elements)",
    "Sort": "Sort(list of numbers, booleans or points)",
    "Append": "Append(list, object); Append(object, list)",
    "Join": "Join(list, list, ...); Join(list of lists)",
    "Mod": "Mod(number, number)",
    "Div": "Div(number, number)",
    "GCD": "GCD(number, number)",
    "LCM": "LCM(number, number)",
    "Max": "Max(number, number); Max(list)",
    "Min": "Min(number, number); Min(list)",
    "Conic": "Conic(point, point, point, point, point)",
    "Ellipse": "Ellipse(focus, focus, point); Ellipse(focus, focus, semi-major axis length)",
    "Hyperbola": "Hyperbola(focus, focus, point); Hyperbola(focus, focus, semi-major axis length)",
    "Parabola": "Parabola(focus, directrix line)",
    "Focus": "Focus(conic)",
    "Axes": "Axes(conic)",
    "FirstAxis": "FirstAxis(conic)",
    "SecondAxis": "SecondAxis(conic)",
    "Directrix": "Directrix(conic)",
    "FirstAxisLength": "FirstAxisLength(conic), as a number",
    "SecondAxisLength": "SecondAxisLength(conic), as a number",
    "Eccentricity": "Eccentricity(conic)",
    "Excentricity": "Excentricity(conic)",
    "Parameter": "Parameter(parabola)",
    "RigidPolygon": "Copy of a polygon with optional offsets or rigid polygon from three or more points",
    "TriangleCenter": "TriangleCenter(point, point, point, index)",
    "PointIn": "PointIn(polygon, conic or conic part as a single region argument)",
    "IntersectPath": "Polygon and line, segment, ray or conic in either order; two polygons",
    "Difference": "Difference(polygon, polygon); Difference(polygon, polygon, boolean)",
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
    if command in OUT_OF_SCOPE_COMMANDS:
        return "", "out_of_scope", ""
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


def build_rows(commands_java, constants_java, forms=None, done=None):
    """Rows of the matrix. `forms` maps an in-scope command to (group, list of
    its 2D forms) and `done` maps a command to its number of done forms."""
    forms = forms or {}
    done = done or {}
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
        priority, status, notes = classify(group, name, "")
        total = got = ""
        if status != "out_of_scope":
            n_forms = len(forms.get(name, ("", []))[1])
            n_done = done.get(name, 0)
            # The counts alone decide: a hand-listed partial with no done
            # form is todo (its notes still list what the kernel covers).
            if n_forms and n_done == n_forms:
                status = "done"
            elif n_done:
                status = "partial"
            else:
                status = "todo"
            total, got = str(n_forms), str(n_done)
        rows[name] = (group, priority, status, total, got, notes)
    out = []
    for name, _ in entries:
        target = aliases.get(name, "")
        real = target or name
        group, priority, status, total, got, notes = rows[real]
        if target:
            total = got = notes = ""
        out.append([name, target, group, priority, status, total, got, notes])
    declared = {}
    for _, args in entries:
        key = tables.get(args, args.lower() or "unknown")
        declared[key] = declared.get(key, 0) + 1
    return out, unclassified, declared


def read_forms(text):
    """forms.csv -> {command: (group, [form, ...])}; checks the counts."""
    reader = csv.reader(io.StringIO(text))
    header = next(reader)
    if header != FORMS_COLUMNS:
        raise ValueError("forms.csv: unexpected header %r" % header)
    out = {}
    for command, group, count, field in reader:
        forms = field.split(" | ") if field else []
        if int(count) != len(forms) or len(set(forms)) != len(forms):
            raise ValueError("forms.csv: %s: count %s for %r" % (command, count, field))
        if command in out:
            raise ValueError("forms.csv: %s is listed twice" % command)
        out[command] = (group, forms)
    return out


def read_evidence(text, forms):
    """form-evidence.toml -> {command: number of done forms}. Only the
    references are checked here; crates/geo-golden/tests/command_forms.rs
    checks that the cases and DIFFERENCES.md rows back them."""
    done, seen = {}, set()
    for row in tomllib.loads(text).get("form", []):
        command, form = row["command"], row["form"]
        if command not in forms or form not in forms[command][1]:
            raise ValueError("form-evidence.toml: unknown form %s(%s)" % (command, form))
        if (command, form) in seen:
            raise ValueError("form-evidence.toml: %s(%s) is listed twice" % (command, form))
        if not row.get("cases") and not row.get("difference"):
            raise ValueError("form-evidence.toml: %s(%s) needs cases or a difference"
                             % (command, form))
        seen.add((command, form))
        done[command] = done.get(command, 0) + 1
    return done


def check_scope(rows, forms):
    """forms.csv must list exactly the in-scope commands, with their groups."""
    errors = []
    in_scope = {r[0]: r[2] for r in rows if not r[1] and r[4] != "out_of_scope"}
    for name in sorted(set(in_scope) - set(forms)):
        errors.append("forms.csv has no row for the in-scope command %s" % name)
    for name in sorted(set(forms) - set(in_scope)):
        errors.append("forms.csv lists %s, which is not an in-scope command" % name)
    for name in sorted(set(forms) & set(in_scope)):
        if forms[name][0] != in_scope[name]:
            errors.append("forms.csv: %s is in group %s, the matrix says %s"
                          % (name, forms[name][0], in_scope[name]))
    return errors


def render_csv(rows):
    buf = io.StringIO(newline="")
    writer = csv.writer(buf, lineterminator="\n")
    writer.writerow(COLUMNS)
    writer.writerows(rows)
    return buf.getvalue()


def summary(rows, unclassified, declared):
    lines = ["total entries: %d" % len(rows),
             "aliases: %d" % sum(1 for r in rows if r[1])]
    real = [r for r in rows if not r[1]]
    counted = [r for r in real if r[5]]
    lines.append("in-scope commands: %d, forms_total: %d, forms_done: %d" % (
        len(counted), sum(int(r[5]) for r in counted), sum(int(r[6]) for r in counted)))
    status = {}
    for r in real:
        status[r[4]] = status.get(r[4], 0) + 1
    lines.append("by status, real commands only: " + ", ".join(
        "%s=%d" % kv for kv in sorted(status.items())))
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
    ap.add_argument("--forms", default=str(HERE / "forms.csv"),
                    help="the 2D forms of every in-scope command")
    ap.add_argument("--evidence", default=str(HERE / "form-evidence.toml"),
                    help="the done forms and their evidence")
    ap.add_argument("--check", action="store_true",
                    help="exit 1 if the CSV on disk differs from the generated one")
    args = ap.parse_args(argv)
    base = Path(args.source) / COMMANDS_DIR
    commands_java = (base / "Commands.java").read_text(encoding="utf-8")
    constants_java = (base / "CommandsConstants.java").read_text(encoding="utf-8")
    forms = read_forms(Path(args.forms).read_text(encoding="utf-8"))
    done = read_evidence(Path(args.evidence).read_text(encoding="utf-8"), forms)
    rows, unclassified, declared = build_rows(commands_java, constants_java, forms, done)
    errors = check_scope(rows, forms)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
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
