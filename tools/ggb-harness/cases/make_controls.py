#!/usr/bin/env python3
"""Record the algebra values of linked and empty input boxes."""

from caselib import Cases, setn

cases = Cases("controls")
cases.case("inputbox", [
    "a = Slider(0, 8, 0.5)", "b = true", 'note = "x < y"', "A = (2, 3)",
    "field = $inputBox(a, _)", "flag = $inputBox(b, _)", "words = $inputBox(note, _)",
    "point = $inputBox(A, _)", "empty = $inputBox(, _)",
], [setn("a", 3.5)])
cases.write()
