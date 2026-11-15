# Command matrix

`docs/command-matrix.csv` has one row per entry of GeoGebra's `Commands` enum
(`Commands.java`, with table ids from `CommandsConstants.java`), in enum order.
Do not edit it by hand; regenerate it:

    python3 -I tools/command-matrix/build_command_matrix.py \
        --source <geogebra checkout> --out docs/command-matrix.csv

`--check` exits non-zero when the file on disk differs from the generated one.
The script prints the counts it found (entries, aliases, rows per group,
status and priority) so they can be compared with the project plan.

## Columns

`command,alias_of,group,priority,status,notes`

- `alias_of`: empty for a real command; for an alias, the command it points to.
  Aliases are the `case` labels of `Commands.englishToInternal`; chains are
  followed to a real command. An alias inherits group, priority and status
  from its target (notes stay empty).
- `group`: the table constant without `TABLE_`, lower case (`geometry`, `3d`).
  The group `english` is the table of renamed commands that are not aliases;
  it has no topic, so those rows fall under the unclassified rule below.

## Status and priority rules

1. Groups `3d`, `spreadsheet`, `probability`, `cas`, `scripting`,
   `statistics`: status `out_of_scope`, priority empty.
2. Hand-listed exceptions the kernel already covers in part: status
   `partial`, `notes` lists the covered forms; an alias copies its target.
   The list lives in `PARTIAL` in the script and comes from
   `crates/geo-kernel/src/script.rs` and the group files in
   `crates/geo-kernel/src/algo/`. The priority is `P0` for the commands of
   the first kernel (`FIRST_KERNEL` in the script), otherwise the priority
   of the group. These win over rule 1.
3. Groups `geometry`, `conic`, `transformation`: status `todo`, priority `P1`.
4. Everything else: status `todo`, priority `P2`. This includes groups the
   script cannot place confidently (currently `english`); the script
   reports them.
