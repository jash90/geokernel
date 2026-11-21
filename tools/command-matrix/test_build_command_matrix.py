import unittest

import build_command_matrix as m

COMMANDS = """
package x;
public enum Commands implements CommandsConstants {
	// ===== Geometry =====
	Line(TABLE_GEOMETRY),

	/* a block comment, with a comma , inside */
	OrthogonalLine(TABLE_GEOMETRY), // trailing comment, with comma
	// Spreadsheet
	CellRange(
		TABLE_SPREADSHEET),

	/** English name */
	PerpendicularLine(TABLE_ENGLISH),
	Mystery(TABLE_ENGLISH),
	Stadium(TABLE_ENGLISH),
	Segment(TABLE_GEOMETRY);

	Commands(int table) { this.table = table; }
	public static Commands englishToInternal(Commands comm) {
		switch (comm) {
			case PerpendicularLine:
				return OrthogonalLine;
			default:
				break;
		}
		return comm;
	}
}
"""
CONSTANTS = """
public interface CommandsConstants {
	int TABLE_GEOMETRY = 0;
	int TABLE_SPREADSHEET = 12;
	int TABLE_ENGLISH = 20;
}
"""


class BuildMatrixTest(unittest.TestCase):
    def setUp(self):
        self.rows, self.unclassified, self.declared = m.build_rows(COMMANDS, CONSTANTS)
        self.by_name = {r[0]: r for r in self.rows}

    def test_entries_in_order_comments_and_multiline(self):
        self.assertEqual([r[0] for r in self.rows],
                         ["Line", "OrthogonalLine", "CellRange", "PerpendicularLine", "Mystery",
                          "Stadium", "Segment"])

    def test_normal_entry(self):
        self.assertEqual(self.by_name["Line"][1:5], ["", "geometry", "P0", "partial"])

    def test_multiline_entry_group(self):
        self.assertEqual(self.by_name["CellRange"][1:5], ["", "spreadsheet", "", "out_of_scope"])

    def test_alias_inherits_from_target(self):
        row = self.by_name["PerpendicularLine"]
        self.assertEqual(row[1:5], ["OrthogonalLine", "geometry", "P0", "partial"])
        self.assertEqual(row[5], "")

    def test_unclassified_group_is_todo_p2(self):
        self.assertEqual(self.by_name["Mystery"][1:5], ["", "english", "P2", "todo"])
        self.assertEqual(self.unclassified, {"english"})

    def test_declared_counts(self):
        self.assertEqual(self.declared["english"], 3)

    def test_listed_command_is_out_of_scope(self):
        self.assertEqual(self.by_name["Stadium"][1:7], ["", "english", "", "out_of_scope", "", ""])

    def test_forms_columns_and_done_status(self):
        forms = {"Line": ("geometry", ["point,point", "point,line"]),
                 "OrthogonalLine": ("geometry", ["point,line"]),
                 "Segment": ("geometry", ["point,point"]),
                 "Mystery": ("english", ["number"])}
        rows, _, _ = m.build_rows(COMMANDS, CONSTANTS, forms,
                                  {"Line": 1, "OrthogonalLine": 1, "Mystery": 1})
        by = {r[0]: r for r in rows}
        # partial with some forms done; done once all are, even for a hand-listed partial
        self.assertEqual(by["Line"][4:7], ["partial", "2", "1"])
        self.assertEqual(by["OrthogonalLine"][4:7], ["done", "1", "1"])
        self.assertEqual(by["Mystery"][4:7], ["done", "1", "1"])
        self.assertEqual(by["Segment"][4:7], ["partial", "1", "0"])
        # an alias copies the status, not the counts
        self.assertEqual(by["PerpendicularLine"][4:7], ["done", "", ""])
        self.assertEqual(by["CellRange"][4:7], ["out_of_scope", "", ""])
        self.assertEqual(m.check_scope(rows, forms), [])
        self.assertTrue(m.check_scope(rows, {"Line": ("geometry", [])}))

    def test_zero_forms_are_not_done(self):
        rows, _, _ = m.build_rows(COMMANDS, CONSTANTS, {"Mystery": ("english", [])}, {})
        self.assertEqual({r[0]: r for r in rows}["Mystery"][4:7], ["todo", "0", "0"])

    def test_forms_and_evidence_files(self):
        forms = m.read_forms('command,group,forms_2d,forms\nLine,geometry,2,"point,point | point,line"\n'
                             'AxisStepX,geogebra,1,()\nType,geogebra,0,\n')
        self.assertEqual(forms["Line"], ("geometry", ["point,point", "point,line"]))
        self.assertEqual(forms["AxisStepX"][1], ["()"])
        self.assertEqual(forms["Type"][1], [])
        with self.assertRaises(ValueError):
            m.read_forms('command,group,forms_2d,forms\nLine,geometry,3,"point,point"\n')
        row = '[[form]]\ncommand = "Line"\nform = "point,point"\ncases = ["x"]\n'
        self.assertEqual(m.read_evidence(row, forms), {"Line": 1})
        for bad in (row + row, row.replace("point,point", "line"),
                    '[[form]]\ncommand = "Line"\nform = "point,line"\n'):
            with self.assertRaises(ValueError):
                m.read_evidence(bad, forms)

    def test_csv_is_deterministic(self):
        self.assertEqual(m.render_csv(self.rows), m.render_csv(self.rows))
        self.assertTrue(m.render_csv(self.rows).startswith("command,alias_of,group,"))


if __name__ == "__main__":
    unittest.main()
