# t-pairs

Command-in, string-out pairs taken from the `t("input", "expected", ...)` calls
in GeoGebra's kernel tests (`source/shared/common-jre/src/test/java/org/geogebra/common/kernel`).

- Derived from GeoGebra at commit `112919f2d450ec7430a5cd3b65dba226fdc43b74`.
- Licence EUPL 1.2, copyright GeoGebra GmbH. The data is a derivative of
  GeoGebra's test sources and stays under the same licence.

## Files

- `t-pairs.jsonl`: one object per extracted call: `file` (relative to the GeoGebra
  repo root), `line`, `test` (enclosing test method or null), `input`, `expected`
  (array of strings; empty when the test expects no result).
- `skipped.jsonl`: calls that could not be extracted: `file`, `line`, `reason`,
  `source` (raw call text).
- `REPORT.md`: totals, skip reasons, the grep denominator, per-file counts.

All three are generated. Do not edit them by hand.

## Caveats for consumers

- The meaning of "expected" depends on the helper in each test class. In the
  default helper (`CommandTestSetup.t`) the strings are compared with the XML
  template output of each result. `ArithmeticTest` and `CommandsIn2DAppTest`
  define their own `t`, so check the `file` field before comparing.
- Calls that pass a `StringTemplate`, a matcher, a variable or a method call are
  skipped, never guessed. `Unicode.X` constants are resolved from GeoGebra's
  own `Unicode.java` (EUPL 1.2 header); see REPORT.md for the list.

## Regenerate

```
python3 -I tools/extract-t-pairs/extract_t_pairs.py \
    --source <path to a GeoGebra checkout at the commit above> --out golden/t-pairs
python3 -I -m unittest discover -s tools/extract-t-pairs
```

The output is deterministic. The script refuses to run on another commit unless
`--allow-other-commit` is given.
