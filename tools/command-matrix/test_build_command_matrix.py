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
	Mystery(TABLE_ENGLISH);

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
                         ["Line", "OrthogonalLine", "CellRange", "PerpendicularLine", "Mystery"])

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
        self.assertEqual(self.declared["english"], 2)

    def test_csv_is_deterministic(self):
        self.assertEqual(m.render_csv(self.rows), m.render_csv(self.rows))
        self.assertTrue(m.render_csv(self.rows).startswith("command,alias_of,group,"))


if __name__ == "__main__":
    unittest.main()
