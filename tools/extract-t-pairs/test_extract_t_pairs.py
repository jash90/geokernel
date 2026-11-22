"""Unit tests for extract_t_pairs.py. Run: python3 -m unittest discover
-s tools/extract-t-pairs"""
import importlib.util
import os
import unittest
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
SPEC = importlib.util.spec_from_file_location(
    "extract_t_pairs", os.path.join(HERE, "extract_t_pairs.py"))
ext = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ext)

CONSTANTS = {"DEGREE_CHAR": ("char", "°", 1),
             "DEGREE_STRING": ("String", "°", 2)}


def run(java, constants=CONSTANTS):
    other = Counter()
    pairs, skipped, found, decls = ext.extract_file(
        java, "X.java", constants, Counter(), other)
    return pairs, skipped, found, other


def wrap(body):
    return "class A {\n  void foo() {\n" + body + "\n  }\n}\n"


class ExtractTest(unittest.TestCase):
    def test_simple_pair_and_method_name(self):
        pairs, skipped, found, _ = run(wrap('t("a", "b");'))
        self.assertEqual(found, 1)
        self.assertEqual(skipped, [])
        self.assertEqual(pairs[0]["input"], "a")
        self.assertEqual(pairs[0]["expected"], ["b"])
        self.assertEqual(pairs[0]["test"], "foo")
        self.assertEqual(pairs[0]["line"], 3)

    def test_escapes(self):
        pairs, _, _, _ = run(wrap(
            r't("say \"hi\" \\ x", "a\nb\tc é \101");'))
        self.assertEqual(pairs[0]["input"], 'say "hi" \\ x')
        self.assertEqual(pairs[0]["expected"], ["a\nb\tc é A"])

    def test_surrogate_pair_and_lone_surrogate(self):
        pairs, skipped, _, _ = run(wrap(
            r't("a", "😀");' + "\n" + r't("a", "\uD83D");'))
        self.assertEqual(pairs[0]["expected"], ["\U0001F600"])
        self.assertIn("lone surrogate", skipped[0]["reason"])

    def test_comma_and_paren_inside_string(self):
        pairs, _, _, _ = run(wrap('t("f(a, b)", "{1, 2)", ")", "(");'))
        self.assertEqual(pairs[0]["input"], "f(a, b)")
        self.assertEqual(pairs[0]["expected"], ["{1, 2)", ")", "("])

    def test_multiline_call(self):
        pairs, _, _, _ = run(wrap('t(\n  "a",\n  "b",\n  "c"\n);'))
        self.assertEqual(pairs[0]["expected"], ["b", "c"])
        self.assertEqual(pairs[0]["line"], 3)

    def test_concatenation(self):
        pairs, _, _, _ = run(wrap('t("a" + "b",\n "c" +\n "d");'))
        self.assertEqual(pairs[0]["input"], "ab")
        self.assertEqual(pairs[0]["expected"], ["cd"])

    def test_comments_are_ignored(self):
        java = wrap('// t("x", "y");\n/* t("x", "y"); */\n'
                    '/* multi\n t("x", "y")\n*/ t("real", "one");')
        pairs, skipped, found, _ = run(java)
        self.assertEqual(found, 1)
        self.assertEqual([p["input"] for p in pairs], ["real"])
        self.assertEqual(pairs[0]["line"], 7)

    def test_comment_markers_inside_string(self):
        pairs, _, _, _ = run(wrap('t("a // b", "/* c */");'))
        self.assertEqual(pairs[0]["input"], "a // b")
        self.assertEqual(pairs[0]["expected"], ["/* c */"])

    def test_non_literal_argument_is_skipped(self):
        pairs, skipped, found, _ = run(wrap('t("a", someVar);\nt("b", f(1));'))
        self.assertEqual(pairs, [])
        self.assertEqual(found, 2)
        self.assertTrue(all(s["reason"].startswith("non-literal")
                            for s in skipped))

    def test_template_argument_is_skipped(self):
        _, skipped, _, _ = run(wrap(
            't("a", StringTemplate.editTemplate, "b");'))
        self.assertTrue(skipped[0]["reason"].startswith("template argument"))

    def test_constant_resolution(self):
        pairs, _, _, _ = run(wrap('t("90" + Unicode.DEGREE_STRING, "x");'))
        self.assertEqual(pairs[0]["input"], "90°")

    def test_unknown_constant_is_skipped(self):
        pairs, skipped, _, _ = run(wrap('t("a" + Unicode.NOPE, "x");'))
        self.assertEqual(pairs, [])
        self.assertIn("unresolved constant: Unicode.NOPE",
                      skipped[0]["reason"])

    def test_char_plus_char_is_skipped(self):
        _, skipped, _, _ = run(wrap(
            't(Unicode.DEGREE_CHAR + Unicode.DEGREE_CHAR, "x");'))
        self.assertIn("char arithmetic", skipped[0]["reason"])

    def test_char_plus_string_is_concatenation(self):
        pairs, _, _, _ = run(wrap('t(Unicode.DEGREE_CHAR + "a", "x");'))
        self.assertEqual(pairs[0]["input"], "°a")

    def test_other_helpers_not_matched_as_t(self):
        pairs, _, found, other = run(wrap(
            'tRound("a", "b");\ntpm("c", "d");\nx.t("e", "f");\n'
            'at("g", "h");'))
        self.assertEqual((pairs, found), ([], 0))
        self.assertEqual(other, Counter({"tRound": 1, "tpm": 1}))

    def test_declaration_is_not_a_call(self):
        java = ("class A {\n void t(String a, String b) { x(); }\n"
                " void foo() { t(\"a\", \"b\"); }\n}\n")
        pairs, _, found, _ = run(java)
        self.assertEqual(found, 1)
        self.assertEqual(pairs[0]["test"], "foo")

    def test_one_param_helper_calls_are_skipped(self):
        java = ("class A {\n void t(String def) { x(); }\n"
                " void foo() { t(\"a\"); }\n}\n")
        pairs, skipped, _, _ = run(java)
        self.assertEqual(pairs, [])
        self.assertIn("no expected value", skipped[0]["reason"])

    def test_no_expected_value_is_empty_list(self):
        pairs, _, _, _ = run(wrap('t("a=1");'))
        self.assertEqual(pairs[0]["expected"], [])


def scripts(java):
    lines = {p["line"] for p in run(java)[0]}
    return ext.extract_scripts(java, "X.java", CONSTANTS, lines)


class ScriptTest(unittest.TestCase):
    def test_steps_in_source_order(self):
        java = ("class A {\n  @Test\n  void foo() {\n"
                '    add("a=1");\n    GeoElement g = add("b=2");\n'
                '    tRound("c", "3.14");\n    t("a+b", "3");\n'
                '    assertEquals(1, 1);\n  }\n}\n')
        [rec] = scripts(java)
        self.assertEqual(rec["test"], "foo")
        self.assertTrue(rec["complete"])
        self.assertEqual([(s["call"], s["input"]) for s in rec["steps"]],
                         [("add", "a=1"), ("add", "b=2"),
                          ("tRound", "c"), ("t", "a+b")])
        self.assertEqual(rec["steps"][2]["expected"], ["3.14"])
        self.assertEqual(rec["steps"][3]["line"], 7)

    def test_other_statement_makes_method_incomplete(self):
        java = ("class A {\n  @Test\n  void foo() {\n"
                '    app.setRounding("5");\n    t("a", "b");\n'
                '    for (int i = 0; i < 2; i++) { t("x", "y"); }\n  }\n}\n')
        [rec] = scripts(java)
        self.assertFalse(rec["complete"])
        self.assertEqual([s["call"] for s in rec["steps"]],
                         ["other", "t", "other"])
        self.assertEqual(rec["steps"][0]["source"], 'app.setRounding("5");')

    def test_settings_and_failing_inputs(self):
        java = ("class A {\n  @Test\n  void foo() {\n"
                "    app.setRandomSeed(7);\n"
                "    AlgebraTestHelper.enableCAS(getApp(), false);\n"
                '    shouldFail("x+", "error", app);\n    t("a", "b");\n'
                "  }\n}\n")
        [rec] = scripts(java)
        self.assertTrue(rec["complete"])
        self.assertEqual([(s["call"], s.get("value")) for s in rec["steps"]],
                         [("seed", 7), ("cas", False), ("t", None)])

    def test_helper_method_is_not_complete(self):
        java = ('class A {\n  private void check() {\n    t("a", "b");\n'
                "  }\n}\n")
        [rec] = scripts(java)
        self.assertEqual(rec["test"], "check")
        self.assertFalse(rec["complete"])

    def test_lambda_and_block_statements(self):
        java = ("class A {\n  @Test\n  void foo() {\n"
                "    assertThrows(X.class, () -> { t(\"q\", \"r\"); });\n"
                '    if (true) { x(); } else { y(); }\n    t("a", "b");\n'
                "  }\n}\n")
        [rec] = scripts(java)
        self.assertEqual([s["call"] for s in rec["steps"]], ["other", "t"])


if __name__ == "__main__":
    unittest.main()
