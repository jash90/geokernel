# Command matrix

`docs/command-matrix.csv` has one row per entry of GeoGebra's `Commands` enum
(`Commands.java`, with table ids from `CommandsConstants.java`), in enum order.
Do not edit it by hand; regenerate it:

    python3 -I tools/command-matrix/build_command_matrix.py \
        --source <geogebra checkout> --out docs/command-matrix.csv

`--check` exits non-zero when the file on disk differs from the generated one.
The script prints the counts it found (entries, aliases, rows per group,
status and priority, forms) so they can be compared with the project plan.
CI has no upstream checkout; there `crates/geo-golden/tests/command_forms.rs`
fails when the forms columns or the `done` status disagree with the two files
below. The script's own tests: `python3 -m unittest test_build_command_matrix`
in this directory.

## Columns

`command,alias_of,group,priority,status,forms_total,forms_done,notes`

- `alias_of`: empty for a real command; for an alias, the command it points to.
  Aliases are the `case` labels of `Commands.englishToInternal`; chains are
  followed to a real command. An alias inherits group, priority and status
  from its target (notes stay empty).
- `group`: the table constant without `TABLE_`, lower case (`geometry`, `3d`).
  The group `english` is the table of renamed commands that are not aliases;
  it has no topic, so those rows fall under the unclassified rule below.
- `forms_total`: the number of 2D forms of the command in `forms.csv`.
- `forms_done`: the number of those forms with a row in `form-evidence.toml`.
  Both are empty for aliases (they count under their target) and for rows
  out of scope.

## Forms and their evidence

`forms.csv` (`command,group,forms_2d,forms`) lists every 2D form of every
in-scope command as an argument signature: `point,point`, `()` for no
argument, and `...` repeating the token before it. A token is the kind
upstream's command processor tests for (`number`, `line`, `conic`, ...);
`any`, `path` and `region` are wildcards. The forms come from an inventory of
the command processors at upstream commit 112919f, with every alternative of
an argument (`polygon|polyline`) written as a form of its own; 3D forms are
left out. The file must list exactly the in-scope commands of the matrix.

`form-evidence.toml` has one row per done form. It can cite golden case ids
that call the command in that form, a disposition in `DIFFERENCES.md`, or
both. A difference reference uses `section-slug: Area text`. Its first cell
names the command with the form's tokens in backticks, such as
`` `Mod(function, function)` `` or `` `AxisStepX()` ``.
`crates/geo-golden/tests/command_forms.rs` checks that each cited case exists,
has no known gap, and calls the command by its name or an alias with matching
arguments. The test reads each argument's kind from the case's first snapshot
or the literal argument. A wildcard form requires
a disposition, with the exception of transformation forms whose cases cover
all supported core geometry kinds. Cases can document supported inputs
alongside a disposition. To list candidate cases for new rows:

    cargo test -p geo-golden --test command_forms -- --ignored --nocapture

## Status and priority rules

0. The commands in `OUT_OF_SCOPE_COMMANDS` (`var`, `cov`, `Polyhedron`,
   `CornerThreeD`, `Evaluate`, `Stadium`, `TableToChart`) and `EXCLUDED_ALGEBRA`
   have status `out_of_scope` and an empty priority, regardless of their group.
   `EXCLUDED_ALGEBRA` records the reason for each exclusion in the matrix's
   `notes` column. `forms.csv` contains the signatures of in-scope commands.
1. Groups `3d`, `spreadsheet`, `probability`, `cas`, `scripting`,
   `statistics`: status `out_of_scope`, priority empty.
2. Hand-listed exceptions the kernel already covers in part: `notes` lists
   the covered forms (the status comes from rule 5); an alias copies its target.
   The list lives in `PARTIAL` in the script and comes from
   `crates/geo-kernel/src/script.rs` and the group files in
   `crates/geo-kernel/src/algo/`. The priority is `P0` for the commands of
   the first kernel (`FIRST_KERNEL` in the script), otherwise the priority
   of the group. These win over rule 1.
3. Groups `geometry`, `conic`, `transformation`: status `todo`, priority `P1`.
4. Everything else: status `todo`, priority `P2`. This includes groups the
   script cannot place confidently (currently `english`); the script
   reports them.
5. The forms decide the status of every row in scope, after rules 2-4 set
   the priority and notes: `done` when the command has at least one form and
   every form is done, `partial` when some are, `todo` when none is. A
   command with no 2D form is never `done`. A hand-listed partial whose
   forms are not done yet (for example forms with a wildcard argument) is
   `todo`; its `notes` still list what the kernel covers.
