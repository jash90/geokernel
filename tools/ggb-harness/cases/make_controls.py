#!/usr/bin/env python3
"""Record the algebra values of linked and empty input boxes."""

from caselib import Cases, setn

cases = Cases("controls")
cases.case("inputbox", [
    "a = Slider(0, 8, 0.5)", "b = true", 'note = "x < y"', "A = (2, 3)",
    "field = $inputBox(a, _)", "flag = $inputBox(b, _)", "words = $inputBox(note, _)",
    "point = $inputBox(A, _)", "empty = $inputBox(, _)",
    "ang = 30°", "anglefield = $inputBox(ang, _)",
], [setn("a", 3.5)])
cases.case("reviewopen", [
    "a = Slider(0, 8, 0.5)", "field = $inputBox(a, _)",
], [setn("a", 3.5)])
cases.case("reviewblur", [
    "a = Slider(0, 8, 0.5)", "b = 2*a", "field = $inputBox(a, _)",
], [setn("a", 3.5)])
cases.case("reviewtab", [
    "a = 0", "b = 0", "c = 0",
    "first = $inputBox(a, _)", "second = $inputBox(b, _)", "third = $inputBox(c, _)",
], [setn("c", 3.5)])
cases.write()
