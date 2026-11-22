#!/usr/bin/env python3
"""Geometry and view corners, including a small synthetic raster."""

import struct
import zlib

from caselib import Cases, mv, num, pt, setn


def encoded(text):
    return "_" + text.encode().hex()


def raster():
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))

    header = struct.pack(">2I5B", 12, 8, 8, 2, 0, 0, 0)
    pixels = (b"\0" + b"\x11\x77\x99" * 12) * 8
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(
        b"IDAT", zlib.compress(pixels)
    ) + chunk(b"IEND", b"")


asset = encoded("sample.png")
image_asset = f"$imageAsset({asset}, _{raster().hex()})"


def image(label, placement):
    return f"{label} = $image({asset}, 1, false, true, _, {placement})"


cases = Cases("corners")
# CommandsTest.cmdVertex, with movable endpoints and an editable index.
cases.case("geometry", [
    pt("A", 0, 0), pt("B", 1, 0), pt("C", 0, 1), num("n", 2),
    "p = Polygon(A, B, C)", "q = PolyLine(A, B, C)", "s = Segment(B, C)",
    "V = Corner(p)", "W = Corner(q)", "G = Corner(p, n)", "H = Corner(q, n)",
    "S = Corner(s, n)",
], [setn("n", 0), mv("B", 0, 1), setn("n", 1.5), setn("n", 2)])
cases.case("conic", [
    pt("A", -1, 0), pt("B", 1, 0), num("r", 3),
    "k = Ellipse(A, B, r)", "V = Corner(k)",
], [setn("r", 0.5), mv("B", 1, 2), setn("r", 3)])
cases.case("view", [
    num("v", 1), num("n", 1), "P = Corner(v, n)", "Q = Corner(1, 6)",
], [setn("n", 5), setn("v", 2), setn("n", 3), setn("v", 1), setn("n", 7), setn("n", 4)])
cases.case("text", [
    pt("A", 2, 3), num("n", 1), 't = Text("Corner", A)', "P = Corner(t, n)",
], [setn("n", 3), mv("A", -2, 4), setn("n", 5), setn("n", 1)])
cases.case("raster", [
    image_asset, pt("A", 2, 3), pt("B", 5, 3), num("n", 1),
    image("im", f"world(false, false, point({encoded('A')}), point({encoded('B')}), _)"),
    image("fixed", "screen(12, 18)"), "P = Corner(im, n)", "Q = Corner(fixed, 2)",
], [setn("n", 2), mv("B", 2, 3), setn("n", 3), mv("B", 5, 5),
    setn("n", 5), setn("n", 4)])
cases.case("inequality", [
    num("h", 2), "q(x,y) = x > 0 && y > 0 && x + y < h", "P = Corner(q)",
], [setn("h", -1), setn("h", 3)])
cases.case("inequality", [
    "q(x,y) = x > 0 && y > 0 && x + y < 2 && y > sin(x) - 10",
    "P = Corner(q)",
], [])

# CommandsTest.cmdVertex combines linear, quadratic and cubic borders.
cases.case("vertex", [
    num("h", 2),
    "q(x,y) = x > y && x > 0 && x^2 + y^2 < h && 4x > y^3 && 4y > x^3",
    "P = Vertex(q)",
], [setn("h", 0), setn("h", 3), setn("h", 2)])
cases.case("vertex", [
    num("h", 2), "P = Vertex(x > 0 && y > 0 && x + y < h)",
], [setn("h", -1), setn("h", 3)])
cases.case("text", [
    pt("A", -1, 2), num("n", 4), 't = Text("Corner", A)', "P = Vertex(t, n)",
], [setn("n", 0), mv("A", 2, 3), setn("n", 2)])
cases.case("checkbox", [
    "b = true", num("n", 1), "P = Vertex(b, n)",
], [setn("n", 3), setn("n", 5), setn("n", 2)])
cases.case("vertex", [
    "q(x,y) = x > 0 && y > 0 && x + y < 2 && y > sin(x) - 10",
    "P = Vertex(q)",
], [])

# CommandsTest.cmdDilate uses the origin and a supplied center.
cases.case("dilate", [
    image_asset, pt("A", 2, 3), pt("B", 5, 3), pt("D", 1, 5), pt("O", 1, 1),
    num("n", 0.5), "r = 1/n",
    image("im", f"world(false, false, point({encoded('A')}), point({encoded('B')}), point({encoded('D')}))"),
    "u = Dilate(im, r)", "v = Dilate(im, r, O)",
    "P = Corner(u, 1)", "Q = Corner(u, 3)", "R = Corner(v, 1)", "S = Corner(v, 4)",
], [mv("A", 1, 2), mv("O", -1, 2), setn("n", 0), setn("n", -1), setn("n", 0.5)])
cases.case("dilate", [
    image_asset, pt("A", 2, 3), num("n", 2),
    image("centered", f"world(true, false, point({encoded('A')}), _, _)"),
    image("fixed", "screen(12, 18)"),
    "u = Dilate(centered, n)", "v = Dilate(fixed, n, A)",
    "P = Corner(u, 1)", "Q = Corner(u, 3)", "R = Corner(v, 1)",
], [setn("n", 0), setn("n", -2), mv("A", -1, 3), setn("n", 2)])
cases.case("dilate", [
    image_asset, pt("A", 2, 3),
    image("im", f"world(false, false, point({encoded('A')}), _, _)"),
    "u = Dilate(im, 2, (1, 1))", "P = Corner(u, 2)", "Q = Corner(u, 4)",
], [mv("A", 1, 2)])

# CommandsTest.cmdApplyMatrix exercises both sizes of matrix.
cases.case("matrix", [
    image_asset, pt("A", 1, 2), pt("B", 3, 2), pt("D", 1, 4), num("n", 2),
    image("im", f"world(false, false, point({encoded('A')}), point({encoded('B')}), point({encoded('D')}))"),
    "m = {{n, 1}, {0, 3}}", "u = ApplyMatrix(m, im)",
    "P = Corner(u, 1)", "Q = Corner(u, 2)", "R = Corner(u, 3)", "S = Corner(u, 4)",
], [mv("A", 1, 1), setn("n", 0), mv("D", 1, 2), setn("n", -2)])
cases.case("matrix", [
    image_asset, pt("A", 1, 2), pt("B", 3, 2), pt("D", 1, 4), num("n", 0.25),
    image("im", f"world(false, false, point({encoded('A')}), point({encoded('B')}), point({encoded('D')}))"),
    "m = {{1, 0, 0}, {0, 1, 0}, {n, 0, 1}}", "u = ApplyMatrix(m, im)",
    "P = Corner(u, 1)", "Q = Corner(u, 2)", "R = Corner(u, 3)", "S = Corner(u, 4)",
], [setn("n", -1), mv("A", 1, 0), mv("A", 1, 2), setn("n", 0), mv("B", 4, 1),
    setn("n", 0.25)])
cases.case("matrix", [
    image_asset, image("fixed", "screen(12, 18)"), num("n", 2),
    "u = ApplyMatrix({{n, 0}, {0, 3}}, fixed)", "P = Corner(u, 1)",
    "v = ApplyMatrix({{1, 2, 3}, {0, 1, 0}}, fixed)", "Q = Corner(v, 4)",
], [setn("n", 0), setn("n", -1)])

cases.case("vertex", [
    num("h", 4), "q(x,y) = x > 0 && y > 0 && x^2*y < h && x < 4", "P = Vertex(q)",
], [setn("h", -1), setn("h", 4)])
for sign in (1, -1):
    cases.case("vertex", [
        num("a", sign), num("h", 4),
        "q(x,y) = x^2 + y^2 < h && a*(x+y) > 0", "P = Vertex(q)",
    ], [setn("a", -sign), setn("h", -1), setn("h", 9), setn("a", sign)])
cases.case("vertex", [
    num("h", 2), "q(x,y) = abs(x) < h && y > 0 && y < 3", "P = Vertex(q)",
], [setn("h", -1), setn("h", 3), setn("h", 2)])
cases.case("vertex", [
    num("h", 1), "q(x,y) = x > y^5-y && x < y^5+h", "P = Vertex(q)",
], [setn("h", -1), setn("h", 0), setn("h", 2)])

cases.case("dilate", [
    image_asset, pt("A", 0, 0), pt("B", 3, 2), pt("D", 1, 4), num("n", 1), "r = 1/n",
    image("im", f"world(false, false, point({encoded('A')}), point({encoded('B')}), point({encoded('D')}))"),
    "u = Dilate(im, r)", "v = Dilate(im, r, D)",
    "P = Corner(u, 1)", "Q = Corner(u, 2)", "R = Corner(v, 1)", "S = Corner(v, 4)",
], [setn("n", 0), mv("A", 1, 2), setn("n", -1)])
cases.case("matrix", [
    image_asset, pt("A", 0, 0), pt("B", 3, 2), pt("D", 1, 4), num("n", 1),
    image("im", f"world(false, false, point({encoded('A')}), point({encoded('B')}), point({encoded('D')}))"),
    "u = ApplyMatrix({{1, 0, 0}, {0, 1, 0}, {1, 0, n}}, im)",
    "P = Corner(u, 1)", "Q = Corner(u, 2)", "R = Corner(u, 3)", "S = Corner(u, 4)",
], [setn("n", 0), mv("A", 1, 2), setn("n", 1)])

cases.case("vertex", [
    "q(x,y) = 0.3x + 0.1y < 0.7 && 0.9x + 0.3y < 2.1 && y > 0 && x > -3",
    "P = Vertex(q)",
], [])

for command in ("Vertex", "Corner"):
    cases.case("growth", [
        num("h", -1), "q(x,y) = x > 0 && y > 0 && x + y < h", f"P = {command}(q)",
    ], [setn("h", 2), setn("h", -1), setn("h", 3)])
    cases.cases[-1]["track_boundary_growth"] = True

for assignment in (" =", ":"):
    cases.case("inequality", [
        f"a{assignment} y > x^2", "b = a(0, 1)", "c = a(2, 1)", "P = Vertex(a)",
    ], [])
cases.case("inequality", [
    "a: y >= x^2", "b = a(0, 0)", "P = Vertex(a)",
], [])

cases.write()
