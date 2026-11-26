//! `docs/kinds.csv`: one row per object kind, kernel kinds and the 2D kinds
//! of upstream alike, with what the project does with each.
//!
//! The rows: every `Kind` variant of `geo-kernel` (read from its source, as
//! the enum is `#[non_exhaustive]`) has exactly one row, a row names only
//! kernel kinds that exist, and every 2D object class of upstream has a row.
//!
//! The cells are `yes`, `partial` or `no`. Checked against the code, for
//! many objects of the kind (up to [`PER_KIND`] golden cases each, so the
//! shapes a kind takes in golden data are covered):
//!
//! * `kernel_golden`, exact: `yes` when a golden case recorded by the
//!   harness has an object of the kind (the golden test then compares it
//!   with the kernel), otherwise `no`;
//! * `svg`, exact: adding a line that creates only the object changes the
//!   image of the SVG writer (`geo_kernel::svg::render`): `yes` for every
//!   sample, `partial` for some, `no` for none. A function whose graph has
//!   no piece inside the view (golden cases of extreme numbers have such
//!   functions) is not a sample, as there is nothing to draw, nor is a pen
//!   stroke without points;
//! * `pointer_hit`, exact: `geo_app_core::hit::hit` hits the object somewhere
//!   on a 400 x 300 view, graded over the samples as for `svg`. A separate
//!   long locus witness tests the desktop sample cap;
//! * `algebra_text`, exact: per sample, the text is proper when
//!   `geo_app_core::describe::is_known` knows the value and
//!   `describe::object_text` is not just its debug form, a fallback when it is
//!   the debug form, missing when the value is not known; `yes` when every
//!   sample is proper, `no` when none is known, otherwise `partial`;
//! * `app_draw`, `no` versus the rest: the window's drawing code
//!   (`crates/geo-app/src/draw.rs`, `crates/geo-app/src/curves/mod.rs`, with
//!   comments and string literals removed) names the value variant. A source
//!   scan, as the window crate is too heavy to link here; whether the cell is
//!   `yes` or `partial` is declared;
//! * `import`, `no` versus the rest: a file of `golden/ggb` imports to an
//!   object of the kind; `yes` or `partial` is declared.
//!
//! * `export`, exact, derived by `tests/export.rs`
//!   (`the_export_column_follows_the_journal_rule_and_the_recorded_proof`,
//!   part of every `cargo test`): `yes` for editor objects serialized from
//!   their own stored data and references, whatever the journal says, whose
//!   modeled presentation properties the reload oracle has proven. Each
//!   `yes` row qualifies the definition and element forms the writer supports.
//!   `partial` records a specific missing property or dependency on journal
//!   syntax in the corpus. `no` means no defined example has a writer.
//!   The Java oracle checks a defined example of each supported kind.
//!
//! Tool coverage is declared in the table.
//! Axis uses graphics-view settings and Formula uses the LaTeX subset of
//! Text. Their witnesses are checked separately from the kernel kind rows.
//! List graphics witnesses distinguish anonymous drawables, named drawables
//! and numeric lists that have no geometry.
//!
//! The samples a checked cell is graded over come from a fixed allow-list
//! of golden cases ([`SAMPLE_CASES`]), never from whichever cases happen
//! to sort first, and hidden `\0` helper objects are never samples: adding
//! golden cases cannot shift the pool, so two branches that each add cases
//! cannot flip a cell of `docs/kinds.csv` when they merge.

use geo_app_core::describe;
use geo_app_core::hit::{self, Reach};
use geo_app_core::interact::ViewTransform;
use geo_golden::table::Table;
use geo_golden::{gaps, model, repo_root};
use geo_kernel::svg::{self, View};
use geo_kernel::{script, Construction, Id, Kind, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const COLUMNS: [&str; 12] = [
    "kind",
    "upstream",
    "kernel_kind",
    "kernel_golden",
    "svg",
    "app_draw",
    "pointer_hit",
    "tool",
    "algebra_text",
    "import",
    "export",
    "notes",
];
const CELLS: [&str; 8] = [
    "kernel_golden",
    "svg",
    "app_draw",
    "pointer_hit",
    "tool",
    "algebra_text",
    "import",
    "export",
];

/// The 2D object classes of upstream (`GeoClass` at commit 112919f, without
/// 3D, CAS, media, scripting actions and whiteboard-only objects).
const UPSTREAM_2D: [&str; 27] = [
    "ANGLE",
    "AXIS",
    "BOOLEAN",
    "BUTTON",
    "TEXTFIELD",
    "CONIC",
    "CONICPART",
    "FUNCTION",
    "IMAGE",
    "LINE",
    "LIST",
    "LOCUS",
    "NUMERIC",
    "POINT",
    "POLYGON",
    "RAY",
    "SEGMENT",
    "TEXT",
    "FORMULA",
    "VECTOR",
    "CURVE_CARTESIAN",
    "IMPLICIT_POLY",
    "FUNCTION_NVAR",
    "POLYLINE",
    "PENSTROKE",
    "TURTLE",
    "PIECHART",
];

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The variants of `pub enum Kind` in the kernel's source.
fn kernel_kinds() -> Vec<String> {
    let src = read("crates/geo-kernel/src/kernel.rs");
    let start = src
        .find("pub enum Kind {")
        .expect("pub enum Kind in kernel.rs");
    let kinds: Vec<String> = src[start + "pub enum Kind {".len()..]
        .lines()
        .map(str::trim)
        .take_while(|l| *l != "}")
        .filter(|l| !l.starts_with("//") && !l.starts_with('#') && !l.is_empty())
        .map(|l| l.trim_end_matches(',').to_string())
        .collect();
    for k in &kinds {
        assert!(
            k.chars().all(char::is_alphanumeric),
            "enum Kind: cannot read `{k}` (only unit variants are expected)"
        );
    }
    kinds
}

fn table() -> Vec<BTreeMap<String, String>> {
    let t =
        Table::parse(&read("docs/kinds.csv"), "docs/kinds.csv").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(t.header, COLUMNS, "docs/kinds.csv header");
    t.records()
        .into_iter()
        .map(|r| {
            r.into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
        .collect()
}

fn kind_name(c: &Construction, id: Id) -> String {
    format!("{:?}", c.kind(id))
}

/// Golden cases sampled per kind and check.
const PER_KIND: usize = 30;

/// The golden cases the per-kind samples come from (see [`samples`]): a
/// fixed allow-list, so that adding cases cannot shift which objects are
/// sampled and flip `docs/kinds.csv` when two branches merge. It holds
/// every case that contributed a sample when the list was made, and
/// nothing hidden: an object whose label starts with `\0` is a helper the
/// user never sees, not a sample. When a kind needs new samples (a new
/// kind, or a cell that no sample checks any more), regenerate the list
/// with `REGEN_SAMPLES=1 cargo test -p geo-golden --test kinds -- --nocapture`,
/// paste it here and update `docs/kinds.csv` with what the test says; a
/// listed case that has disappeared from the golden data fails the test.
const SAMPLE_CASES: &[&str] = &[
    "algebraedge-base-001",
    "algebraedge-filter-001",
    "algebraedge-filter-002",
    "algebraedge-filter-003",
    "algebraedge-integer-001",
    "algebraedge-integer-002",
    "algebraedge-maximum-001",
    "algebraedge-maximum-002",
    "algebraedge-maximum-003",
    "algebraedge-maximum-004",
    "algebraedge-maximum-005",
    "algebraedge-maximum-006",
    "algebraedge-minimum-001",
    "algebraedge-minimum-002",
    "algebraedge-minimum-003",
    "algebraedge-minimum-004",
    "algebraedge-minimum-005",
    "algebraedge-minimum-006",
    "algebralogic-base-001",
    "algebralogic-base-002",
    "algebralogic-base-003",
    "algebralogic-base-004",
    "algebralogic-base-005",
    "algebralogic-base-006",
    "algebralogic-base-011",
    "algebralogic-base-012",
    "algebralogic-extrema-001",
    "algebralogic-extrema-002",
    "algebralogic-extrema-003",
    "algebralogic-extrema-004",
    "algebralogic-extrema-005",
    "algebralogic-extrema-006",
    "algebralogic-extrema-007",
    "algebralogic-extrema-008",
    "algebralogic-extrema-009",
    "algebralogic-extrema-010",
    "algebralogic-extrema-011",
    "algebralogic-extrema-012",
    "algebralogic-extrema-013",
    "algebralogic-integer-001",
    "algebralogic-integer-002",
    "algebralogic-integer-003",
    "algebralogic-integer-004",
    "algebralogic-integer-005",
    "algebralogic-integer-006",
    "algebralogic-integer-007",
    "algebralogic-integer-009",
    "algebralogic-integer-010",
    "algebralogic-integer-011",
    "algebralogic-integer-012",
    "algebralogic-integer-013",
    "algebralogic-integer-014",
    "algebralogic-integer-015",
    "algebralogic-integer-016",
    "algebralogic-integer-019",
    "algebralogic-integer-021",
    "algebralogic-integer-022",
    "algebralogic-integer-023",
    "algebralogic-integer-024",
    "algebralogic-integer-025",
    "algebralogic-integer-026",
    "algebralogic-integer-027",
    "algebralogic-integer-028",
    "algebralogic-logic-003",
    "algebralogic-logic-004",
    "algebralogic-logic-006",
    "algebralogic-logic-008",
    "algebralogic-logic-009",
    "algebralogic-logic-010",
    "algebralogic-logic-013",
    "algebralogic-logic-014",
    "algebralogic-logic-015",
    "algebralogic-logic-019",
    "algebralogic-logic-020",
    "algebralogic-logic-021",
    "algebralogic-logic-023",
    "algebralogic-logic-024",
    "algebralogic-polynomial-001",
    "algebralogic-polynomial-002",
    "algebralogic-polynomial-003",
    "algebralogic-products-007",
    "algebralogic-relation-001",
    "algebralogic-relation-002",
    "algebralogic-relation-003",
    "algebralogic-relation-004",
    "algebralogic-relation-005",
    "algebralogic-relation-006",
    "algebralogic-sides-001",
    "algebralogic-sides-002",
    "algebralogic-sides-003",
    "algebralogic-sides-004",
    "algebralogic-sides-005",
    "algebralogic-sides-006",
    "algebrareviewshared-circle-001",
    "algebrareviewshared-sides-001",
    "angle-conic-001",
    "angle-conic-002",
    "angle-conic-003",
    "angle-directions-001",
    "angle-directions-002",
    "angle-directions-003",
    "angle-directions-004",
    "angle-directions-005",
    "angle-directions-006",
    "angle-directions-007",
    "angle-directions-008",
    "angle-directions-009",
    "angle-directions-010",
    "angle-fixed-001",
    "angle-fixed-002",
    "angle-lines-001",
    "angle-lines-002",
    "angle-lines-003",
    "angle-lines-004",
    "angle-lines-005",
    "angle-lines-006",
    "angle-lines-007",
    "angle-lines-008",
    "angle-lines-009",
    "angle-lines-010",
    "angle-lines-011",
    "angle-lines-012",
    "angle-numeric-001",
    "angle-numeric-002",
    "angle-points-001",
    "angle-points-003",
    "angle-polygon-001",
    "angle-polygon-002",
    "angle-polygon-003",
    "arcs-chain-001",
    "arcs-chain-002",
    "arcs-circular-001",
    "arcs-circular-002",
    "arcs-circular-003",
    "arcs-circular-004",
    "arcs-circular-005",
    "arcs-circular-006",
    "arcs-circular-007",
    "arcs-circular-008",
    "arcs-circular-009",
    "arcs-circular-010",
    "arcs-circular-011",
    "arcs-circum-001",
    "arcs-circum-002",
    "arcs-circum-003",
    "arcs-circum-004",
    "arcs-circum-005",
    "arcs-circum-006",
    "arcs-circum-007",
    "arcs-circum-008",
    "arcs-circum-009",
    "arcs-circum-010",
    "arcs-circum-011",
    "arcs-circum-012",
    "arcs-circum-013",
    "arcs-conicparams-001",
    "arcs-conicparams-002",
    "arcs-conicparams-003",
    "arcs-conicparams-004",
    "arcs-conicparams-005",
    "arcs-conicparams-006",
    "arcs-conicparams-007",
    "arcs-conicpoints-001",
    "arcs-conicpoints-002",
    "arcs-conicpoints-003",
    "arcs-conicpoints-004",
    "arcs-conicpoints-005",
    "arcs-inline-001",
    "arcs-keptpart-001",
    "arcs-keptpart-002",
    "arcs-keptpart-003",
    "arcs-keptpart-004",
    "arcs-keptpart-005",
    "arcs-keptpart-007",
    "arcs-points-002",
    "arcs-points-006",
    "arcs-points-007",
    "arcs-points-008",
    "arcs-points-009",
    "arcs-points-010",
    "arcs-undefined-002",
    "arcs-undefined-003",
    "arcs-undefined-005",
    "arcs-undefined-006",
    "arcs-undefined-007",
    "axes-conics-001",
    "axes-functions-001",
    "axes-lines-001",
    "calculus-between-001",
    "calculus-between-002",
    "calculus-between-003",
    "calculus-between-004",
    "calculus-between-005",
    "calculus-between-006",
    "calculus-between-007",
    "calculus-curvederivative-001",
    "calculus-curvederivative-002",
    "calculus-curvederivative-003",
    "calculus-curvederivative-004",
    "calculus-curvederivative-005",
    "calculus-curvederivative-006",
    "calculus-curvederivative-007",
    "calculus-curvederivative-008",
    "calculus-curvederivative-009",
    "calculus-curvederivative-010",
    "calculus-curvederivative-011",
    "calculus-curvederivative-013",
    "calculus-curvederivative-014",
    "calculus-curvederivative-015",
    "calculus-curvederivative-016",
    "calculus-curvederivative-017",
    "calculus-curvederivative-018",
    "calculus-curvederivative-019",
    "calculus-curvederivative-020",
    "calculus-curvederivative-021",
    "calculus-curvederivative-022",
    "calculus-curvederivative-023",
    "calculus-curvederivative-024",
    "calculus-curvederivative-025",
    "calculus-rootpower-002",
    "canvas-drawing-001",
    "canvas-listcircles-001",
    "canvas-listconicmembers-001",
    "canvas-listlines-001",
    "canvas-listnamed-001",
    "canvas-listrays-001",
    "canvas-listsegments-001",
    "canvas-parts-001",
    "canvas-reviewlimits-001",
    "canvas-reviewlocus-001",
    "canvas-reviewpen-001",
    "canvas-reviewrectangle-001",
    "canvas-reviewselection-001",
    "canvas-reviewstretch-001",
    "canvas-reviewstrokes-001",
    "cas-coefficients-005",
    "cas-conic-001",
    "cas-conic-002",
    "cas-conic-003",
    "cas-conic-004",
    "cas-conic-005",
    "cas-conic-006",
    "cas-conic-007",
    "cas-conic-008",
    "cas-conic-011",
    "cas-conic-013",
    "cas-conic-053",
    "cas-conic-055",
    "cas-edge-017",
    "cas-expand-005",
    "cas-factor-009",
    "cas-factor-010",
    "cas-factored-001",
    "cas-factored-002",
    "cas-factored-003",
    "cas-factored-004",
    "cas-factored-005",
    "cas-factored-006",
    "cas-factored-007",
    "cas-factored-008",
    "cas-factored-009",
    "cas-factored-010",
    "cas-factored-011",
    "cas-factored-012",
    "cas-factored-013",
    "cas-factored-014",
    "cas-factored-015",
    "cas-factored-016",
    "cas-hint-007",
    "cas-hint-008",
    "cas-nestedrewrite-004",
    "cas-nestedrewrite-005",
    "cas-nestedrewrite-006",
    "cas-order-091",
    "cas-rejected-001",
    "cas-rejected-002",
    "cas-rejected-003",
    "cas-rejected-004",
    "cas-simplify-004",
    "cas-text-001",
    "cas-text-002",
    "cas-text-003",
    "cas-text-004",
    "cas-text-005",
    "cas-text-006",
    "cas-text-007",
    "cas-text-008",
    "cas-text-009",
    "cas-text-010",
    "cas-text-011",
    "cas-text-012",
    "cas-text-013",
    "cas-text-014",
    "cas-text-015",
    "cas-text-016",
    "cas-text-017",
    "cas-text-018",
    "cas-text-019",
    "charts-frequencypolygon-001",
    "charts-frequencypolygon-002",
    "charts-frequencypolygon-003",
    "charts-frequencypolygon-004",
    "charts-frequencypolygon-005",
    "charts-frequencypolygon-006",
    "charts-frequencypolygon-007",
    "charts-frequencypolygon-100",
    "charts-piechart-001",
    "charts-piechart-002",
    "charts-piechart-004",
    "charts-piechart-006",
    "charts-piechart-007",
    "charts-piechart-008",
    "charts-piechart-009",
    "charts-piechart-010",
    "commontangents-conics-001",
    "conic-diameter-004",
    "conic-diameter-008",
    "conic-midseg-001",
    "conic-midseg-002",
    "conic-midseg-003",
    "conic-midseg-004",
    "conic-midseg-005",
    "conic-midseg-006",
    "conic-segrad-001",
    "conic-segrad-002",
    "conic-segrad-003",
    "conic-segrad-004",
    "conic-segrad-005",
    "conic-segrad-006",
    "conic-segrad-007",
    "conic-segrad-008",
    "conic-segrad-009",
    "conicforms-center-001",
    "conicforms-center-002",
    "conicpaths-locus-001",
    "conicpaths-locus-002",
    "conicpaths-locus-003",
    "conicpaths-locus-004",
    "conicpaths-locus-005",
    "conicreview-circlepole-001",
    "conics-arcreaders-001",
    "conics-parabola-009",
    "conics-parabola-010",
    "conics-tangent-003",
    "conics-tangent-006",
    "conics-tangent-009",
    "conics-tangent-015",
    "conics-transform-001",
    "conics-transform-009",
    "conics-transform-017",
    "conics-transform-025",
    "controls-inputbox-001",
    "controls-reviewblur-001",
    "controls-reviewdrag-001",
    "controls-reviewopen-001",
    "controls-reviewsliderclick-001",
    "controls-reviewtab-001",
    "controls-reviewtext-001",
    "corners-checkbox-001",
    "corners-checkbox-002",
    "corners-checkbox-004",
    "corners-definedimage-001",
    "corners-dilate-001",
    "corners-dilate-002",
    "corners-dilate-003",
    "corners-dilate-004",
    "corners-geometry-001",
    "corners-growth-001",
    "corners-growth-002",
    "corners-helperstate-001",
    "corners-helperstate-002",
    "corners-helperstate-003",
    "corners-helperstate-004",
    "corners-helperstate-005",
    "corners-helperstate-006",
    "corners-helperstate-007",
    "corners-inequality-001",
    "corners-inequality-002",
    "corners-inequality-003",
    "corners-inequality-004",
    "corners-inequality-005",
    "corners-inequality-006",
    "corners-inequality-007",
    "corners-inequality-008",
    "corners-matrix-001",
    "corners-matrix-002",
    "corners-matrix-003",
    "corners-matrix-004",
    "corners-matrix-005",
    "corners-raster-001",
    "corners-reviewthree-002",
    "curvepairs-coincident-001",
    "curvepairs-coincident-002",
    "discrete-delaunay-001",
    "discrete-delaunay-006",
    "discrete-delaunay-007",
    "discrete-delaunay-008",
    "discrete-delaunay-009",
    "discrete-delaunay-010",
    "discrete-delaunay-011",
    "discrete-delaunay-012",
    "discrete-delaunay-013",
    "discrete-delaunay-014",
    "discrete-delaunay-015",
    "discrete-delaunay-016",
    "discrete-delaunay-017",
    "discrete-delaunaylist-001",
    "discrete-hull-001",
    "discrete-hull-003",
    "discrete-hull-004",
    "discrete-hull-005",
    "discrete-hull-006",
    "discrete-hull-007",
    "discrete-hullreview-001",
    "discrete-hullreview-002",
    "discrete-tour-006",
    "discrete-tree-008",
    "discrete-voronoi-006",
    "envelope-circle-001",
    "envelope-circle-002",
    "envelope-circle-003",
    "envelope-circle-004",
    "envelope-circle-005",
    "envelope-circle-006",
    "envelope-classic-002",
    "envelope-classic-003",
    "envelope-classic-004",
    "envelope-classic-005",
    "envelope-parabola-001",
    "export-conics-001",
    "export-curve-001",
    "export-geometry-001",
    "export-paths-001",
    "export-transforms-001",
    "expr-apply-015",
    "expr-cmd-011",
    "expr-fncmd-006",
    "expr-fnundef-009",
    "expr-label-001",
    "expr-label-002",
    "funcanalysis-curveedge-005",
    "funcanalysis-curveupstream-003",
    "funcanalysis-forms-016",
    "funcrest-conditional-004",
    "funcrest-derivative-002",
    "funcrest-splinelong-001",
    "funcrest-splinelong-002",
    "funcrest-splinelong-004",
    "funcrest-splinelong-005",
    "funcrest-splinelong-007",
    "funcrest-splinelong-008",
    "funcrest-splinelong-009",
    "funcrest-svd-025",
    "funcrest-vertexform-008",
    "geomclip-carrier-001",
    "geomclip-carrier-002",
    "geomclip-carrier-003",
    "geomclip-carrier-004",
    "geomclip-carrier-005",
    "geomclip-conic-001",
    "geomclip-conic-002",
    "geomclip-conic-003",
    "geomclip-conic-004",
    "geomclip-difference-003",
    "geomclip-difference-004",
    "geomclip-difference-005",
    "geomclip-difference-007",
    "geomclip-difference-008",
    "geomclip-polygon-001",
    "geomclip-polygon-002",
    "geomclosure-comparison-002",
    "geomclosure-locusmeasure-001",
    "geomclosure-locusmeasure-002",
    "geomclosure-ode-001",
    "geomclosure-polylineflag-001",
    "geomclosure-projection-001",
    "geomclosure-region-001",
    "geomclosure-regionmoves-001",
    "geomclosure-rigid-001",
    "geomclosure-shapelist-001",
    "geomclosure-shapelist-002",
    "geomclosure-slopefield-001",
    "geomextras-polygonlist-001",
    "geomextras-polylineflag-001",
    "geomextras-polylinelist-001",
    "geomextras-polylinelist-002",
    "geomforms-closest-008",
    "geomforms-ray-001",
    "geomforms-ray-002",
    "geomforms-ray-003",
    "geomforms-tangent-001",
    "geomforms-tangent-002",
    "geomformsreview-odenamed-001",
    "georestclip-conicpart-002",
    "georestclip-conicpart-005",
    "georestclip-conicpart-008",
    "georestclip-conicpart-011",
    "georestclip-conicpart-014",
    "georestclip-conicpart-017",
    "georestclip-conicpart-020",
    "georestclip-conicpart-023",
    "georestclip-conicpart-026",
    "georestclip-conicpart-029",
    "georestclip-conicpart-032",
    "georestclip-conicpart-035",
    "georestclip-longpath-001",
    "georestclip-paths-003",
    "georestclip-polyline-001",
    "georestclip-polyline-002",
    "georestclip-polyline-003",
    "georestclip-polyline-004",
    "georestclip-region-003",
    "georestintersect-affine-002",
    "georestintersect-affine-003",
    "georestsmall-ifgeometry-002",
    "implicit-asymptotes-001",
    "implicit-asymptotes-002",
    "implicit-asymptotes-003",
    "implicit-asymptotes-004",
    "implicit-asymptotes-005",
    "implicit-asymptotes-006",
    "implicit-barycentric-001",
    "implicit-barycentric-002",
    "implicit-cleanup-002",
    "implicit-conic-001",
    "implicit-conic-002",
    "implicit-conic-003",
    "implicit-conic-004",
    "implicit-conic-005",
    "implicit-conic-006",
    "implicit-conic-007",
    "implicit-conic-008",
    "implicit-conic-009",
    "implicit-conic-010",
    "implicit-line-008",
    "implicit-line-009",
    "implicit-polyline-001",
    "implicit-polyline-002",
    "implicit-polyline-003",
    "intersectfunctions-line-011",
    "intersections-boundedconicreview-013",
    "intersections-boundedconicreview-014",
    "intersections-boundedconicreview-015",
    "intersections-boundedconicreview-016",
    "intersections-boundedconicreview-017",
    "intersections-boundedconicreview-018",
    "intersections-boundedconicreview-019",
    "intersections-boundedconicreview-020",
    "intersections-boundedconicreview-021",
    "intersections-boundedconicreview-022",
    "intersections-boundedconicreview-023",
    "intersections-boundedconicreview-024",
    "lists-locusfirst-001",
    "lists-locusfirst-002",
    "lists-locusfirst-003",
    "lists-locusfirst-004",
    "lists-locusfirst-005",
    "lists-locusfirst-006",
    "locus-arc-001",
    "locus-arc-002",
    "matrixdilate-applypoly-003",
    "stroke-breaks-001",
    "stroke-breaks-002",
    "stroke-breaks-003",
    "stroke-breaks-004",
    "stroke-breaks-005",
    "stroke-breaks-006",
    "stroke-frozen-001",
    "stroke-frozen-003",
    "stroke-literals-001",
    "stroke-literals-002",
    "stroke-literals-003",
    "stroke-literals-004",
    "stroke-literals-005",
    "stroke-literals-006",
    "stroke-literals-007",
    "stroke-literals-008",
    "stroke-numbers-001",
    "stroke-numbers-002",
    "stroke-numbers-003",
    "stroke-numbers-004",
    "stroke-numbers-005",
    "stroke-points-002",
    "stroke-points-003",
    "stroke-points-004",
    "stroke-points-005",
    "stroke-points-006",
    "stroke-points-007",
    "transformsfix-area-001",
    "transformsfix-area-002",
    "transformsfix-area-003",
    "transformsfix-area-004",
    "transformsfix-area-005",
];

/// Whether the sample list should be printed instead of checked.
fn regenerating() -> bool {
    std::env::var_os("REGEN_SAMPLES").is_some()
}

/// Objects of each kind, found by running the golden scripts line by line;
/// at most one sample of a kind per case.
#[derive(Default)]
struct Samples {
    /// Kinds recorded in harness snapshots.
    recorded: BTreeSet<String>,
    /// Per kind: scripts and the label of a defined object of the kind.
    any: BTreeMap<String, Vec<(Vec<String>, String)>>,
    /// Per kind: scripts up to a line that creates only one object, of
    /// the kind and defined.
    single: BTreeMap<String, Vec<Vec<String>>>,
    /// The cases that contributed a sample (the regeneration output).
    used: BTreeSet<String>,
}

fn samples() -> Samples {
    let cases = model::load_all(&repo_root()).unwrap_or_else(|e| panic!("golden data: {e}"));
    let gaps = gaps::load().unwrap_or_else(|e| panic!("{e}"));
    let mut s = Samples::default();
    if !regenerating() {
        // Keep allow-list entries for cases that acquire a gap. They can
        // contribute samples again once the gap is closed.
        let known: BTreeSet<&str> = cases
            .iter()
            .filter(|c| c.generator == "ggb-harness")
            .map(|c| c.id.as_str())
            .collect();
        for id in SAMPLE_CASES {
            assert!(
                known.contains(id),
                "{id} is listed in SAMPLE_CASES but is no golden case (regenerate the list)"
            );
        }
    }
    let mut cases: Vec<_> = cases
        .into_iter()
        .filter(|c| c.generator == "ggb-harness" && !gaps.contains_key(&c.id))
        .collect();
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    let listed = |id: &str| regenerating() || SAMPLE_CASES.contains(&id);
    for case in &cases {
        for e in case.snapshots[0].values() {
            s.recorded.insert(format!("{:?}", e.kind));
        }
    }
    for case in &cases {
        if !listed(&case.id) {
            continue;
        }
        let mut c = Construction::new();
        let (mut any_here, mut single_here) = (BTreeSet::new(), BTreeSet::new());
        for (i, line) in case.script.iter().enumerate() {
            let before: BTreeSet<Id> = c.ids().collect();
            if script::exec(&mut c, line).is_err() {
                break;
            }
            let new: Vec<Id> = c.ids().filter(|id| !before.contains(id)).collect();
            for &id in &new {
                if matches!(c.value(id), Value::Undefined) {
                    continue;
                }
                // a hidden helper object is no sample
                if c.label(id).starts_with('\0') {
                    continue;
                }
                let k = kind_name(&c, id);
                let any = s.any.entry(k.clone()).or_default();
                if any.len() < PER_KIND && any_here.insert(k.clone()) {
                    s.used.insert(case.id.clone());
                    any.push((case.script[..=i].to_vec(), c.label(id).to_string()));
                }
                let single = s.single.entry(k.clone()).or_default();
                if new.len() == 1 && single.len() < PER_KIND && single_here.insert(k) {
                    s.used.insert(case.id.clone());
                    single.push(case.script[..=i].to_vec());
                }
            }
        }
    }
    let locus = cases
        .iter()
        .find(|case| case.id == "canvas-reviewlocus-001")
        .expect("the recorded closing-round locus");
    s.any
        .entry("Locus".into())
        .or_default()
        .push((locus.script.clone(), "l".into()));
    let rasters = include_bytes!("../../../golden/ggb/saved/saved-images-001.ggb");
    let imported = geo_import::import(rasters).unwrap();
    let c = script::load(&imported.script.join("\n")).unwrap();
    s.any.insert(
        "Image".into(),
        c.images()
            .map(|(id, _)| (imported.script.clone(), c.label(id).into()))
            .collect(),
    );
    // Control kinds also have drawable instances in saved worksheets. Most
    // numeric outputs and dependent booleans in the command corpus are
    // algebra-only, so include one of each canvas control explicitly.
    for (kind, source, label) in [
        (
            "Number",
            include_bytes!("../../../golden/ggb/saved/saved-controls-001.xml").as_slice(),
            "a",
        ),
        (
            "Boolean",
            include_bytes!("../../../golden/ggb/saved/saved-controls-001.xml").as_slice(),
            "check",
        ),
    ] {
        let imported = geo_import::import(source).unwrap();
        s.any
            .entry(kind.into())
            .or_default()
            .push((imported.script, label.into()));
    }
    if regenerating() {
        println!(
            "SAMPLE_CASES ({} cases):\n{}",
            s.used.len(),
            s.used
                .iter()
                .map(|id| format!("    \"{id}\","))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    s.single.retain(|_, v| !v.is_empty());
    s
}

/// `yes` when every sample passes, `no` when none does, else `partial`.
fn grade(passed: impl IntoIterator<Item = bool>) -> Option<&'static str> {
    let (mut yes, mut n) = (0, 0);
    for p in passed {
        n += 1;
        yes += usize::from(p);
    }
    match (yes, n) {
        (_, 0) => None,
        (0, _) => Some("no"),
        (y, n) if y == n => Some("yes"),
        _ => Some("partial"),
    }
}

/// Rust source with comments, string literals and character literals
/// removed, so a scan for a name sees only code.
fn code_only(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push(' ');
        } else if c == 'r'
            && matches!(next, Some('"' | '#'))
            && !b[..i]
                .last()
                .is_some_and(|p| p.is_alphanumeric() || *p == '_')
        {
            // raw string r"...", r#"..."#
            let mut j = i + 1;
            let mut hashes = 0;
            while b.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if b.get(j) != Some(&'"') {
                out.push(c);
                i += 1;
                continue;
            }
            j += 1;
            loop {
                if j >= b.len() {
                    break;
                }
                if b[j] == '"' && (0..hashes).all(|h| b.get(j + 1 + h) == Some(&'#')) {
                    j += 1 + hashes;
                    break;
                }
                j += 1;
            }
            out.push_str("\"\"");
            i = j;
        } else if c == '"' {
            i += 1;
            while i < b.len() && b[i] != '"' {
                i += if b[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push_str("\"\"");
        } else if c == '\'' {
            // a character literal ('x', '\n', '\'') or a lifetime ('a)
            if next == Some('\\') {
                i += 2;
                while i < b.len() && b[i] != '\'' {
                    i += 1;
                }
                i += 1;
                out.push_str("' '");
            } else if b.get(i + 2) == Some(&'\'') {
                i += 3;
                out.push_str("' '");
            } else {
                out.push(c);
                i += 1;
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn build(lines: &[String]) -> Construction {
    let mut c = Construction::new();
    for l in lines {
        script::exec(&mut c, l).unwrap_or_else(|e| panic!("`{l}`: {e}"));
    }
    c
}

/// Whether the SVG writer draws the last line's object.
fn svg_draws(lines: &[String]) -> bool {
    let with = build(lines);
    let without = build(&lines[..lines.len() - 1]);
    let view = View::fit(&with);
    svg::render(&with, view, 400, 300) != svg::render(&without, view, 400, 300)
}

/// Whether the last line of a script made something there is to draw: a
/// function counts only when its graph has a piece inside the view, two
/// neighbouring x of 401 probes with finite values in it (probed
/// independently of the plotter; an isolated defined point is not drawn),
/// and a pen stroke only when it has a point.
fn has_something_to_draw(lines: &[String]) -> bool {
    let c = build(lines);
    let before: BTreeSet<Id> = build(&lines[..lines.len() - 1]).ids().collect();
    let view = View::fit(&c);
    let inside = |id: Id, i: u32| {
        let x = view.xmin + (view.xmax - view.xmin) * f64::from(i) / 400.0;
        c.function_value(id, x)
            .is_some_and(|y| y.is_finite() && (view.ymin..=view.ymax).contains(&y))
    };
    c.ids().filter(|id| !before.contains(id)).all(|id| {
        stroke_has_points(&c, id)
            && (c.kind(id) != Kind::Text || c.text(id).is_some_and(|text| !text.is_empty()))
            && (c.kind(id) != Kind::Angle || c.angle_geometry(id).is_some())
            && (!matches!(c.value(id), Value::Function)
                || (0..400).any(|i| inside(id, i) && inside(id, i + 1)))
    })
}

/// Whether `id` is no pen stroke, or one with a point to draw.
fn stroke_has_points(c: &Construction, id: Id) -> bool {
    c.stroke(id)
        .is_none_or(|stroke| stroke.defined_points().next().is_some())
}

/// The construction of a script and the id of `label` in it.
fn object(lines: &[String], label: &str) -> (Construction, Id) {
    let c = build(lines);
    let id = c.id(label).unwrap_or_else(|| panic!("no object `{label}`"));
    (c, id)
}

/// Whether a pointer hits the object. Most shapes are probed on a 400 x 300
/// view. Text and angle markers are probed at their geometry because a very
/// small marker can fall between pixels of the grid.
fn pointer_hits(c: &Construction, id: Id) -> bool {
    let tr = ViewTransform::new(View::fit(c), 400.0, 300.0);
    let reach = Reach::square(3.0);
    let point = if let Some(angle) = c.angle_geometry(id) {
        let radius = 0.5 * angle.radius(tr.xscale());
        let theta = angle.start + 0.5 * angle.sweep;
        let (x, y) = tr.world_to_screen(angle.center.0, angle.center.1);
        Some((x + radius * theta.cos(), y - radius * theta.sin()))
    } else if c.kind(id) == Kind::Text {
        let [x0, y0, x1, y1] = c
            .text_pixel_rect(id, |x, y| tr.world_to_screen(x, y))
            .unwrap_or([0.0; 4]);
        Some((f64::from((x0 + x1) * 0.5), f64::from((y0 + y1) * 0.5)))
    } else {
        None
    };
    if let Some(p) = point {
        return hit::hit(c, id, &tr, p, 3.0, &reach).is_some();
    }
    (0..134).any(|i| {
        (0..100).any(|j| {
            let p = (3.0 * i as f64, 3.0 * j as f64);
            hit::hit(c, id, &tr, p, 3.0, &reach).is_some()
        })
    })
}

/// The text the window's drawing code must contain for a kind it draws.
fn draw_needle(kind: &str) -> String {
    match kind {
        // lines are drawn through `Value::unit_line`
        "Line" => "unit_line".into(),
        "Angle" => "angle_geometry".into(),
        "Boolean" => "Value::Bool".into(),
        k => format!("Value::{k}"),
    }
}

fn construction_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let p = e.unwrap().path();
        if p.is_dir() {
            construction_files(&p, out);
        } else if p
            .extension()
            .is_some_and(|x| x == "xml" || x == "ggb" && !p.with_extension("xml").exists())
        {
            out.push(p);
        }
    }
}

/// Kinds of the objects the importer makes from the files of `golden/ggb`.
fn imported_kinds() -> BTreeSet<String> {
    let mut files = Vec::new();
    construction_files(&repo_root().join("golden/ggb"), &mut files);
    assert!(!files.is_empty(), "no files in golden/ggb");
    let mut kinds = BTreeSet::new();
    for f in files {
        let Ok(imported) = geo_import::import(&std::fs::read(&f).unwrap()) else {
            continue;
        };
        let mut c = Construction::new();
        for line in &imported.script {
            if script::exec(&mut c, line).is_err() {
                break;
            }
        }
        kinds.extend(c.ids().map(|id| kind_name(&c, id)));
    }
    kinds
}

#[test]
fn every_kernel_kind_and_upstream_kind_has_a_row() {
    let rows = table();
    let kinds = kernel_kinds();
    assert!(kinds.len() >= 15, "parsed only {kinds:?} from enum Kind");
    let mut errors = Vec::new();
    for k in &kinds {
        let n = rows.iter().filter(|r| &r["kernel_kind"] == k).count();
        if n != 1 {
            errors.push(format!("Kind::{k} has {n} rows"));
        }
    }
    let mut names = BTreeSet::new();
    for r in &rows {
        let kk = &r["kernel_kind"];
        if !kk.is_empty() && !kinds.contains(kk) {
            errors.push(format!(
                "row `{}` names Kind::{kk}, which does not exist",
                r["kind"]
            ));
        }
        if !names.insert(r["kind"].clone()) {
            errors.push(format!("row `{}` is listed twice", r["kind"]));
        }
        if !UPSTREAM_2D.contains(&r["upstream"].as_str()) {
            errors.push(format!(
                "row `{}`: `{}` is no 2D upstream class",
                r["kind"], r["upstream"]
            ));
        }
        for col in CELLS {
            if !["yes", "partial", "no"].contains(&r[col].as_str()) {
                errors.push(format!("row `{}`: {col} is `{}`", r["kind"], r[col]));
            }
            if kk.is_empty() && !matches!(r["kind"].as_str(), "Axis" | "Formula") && r[col] != "no"
            {
                errors.push(format!(
                    "row `{}` has no kernel kind but {col} is `{}`",
                    r["kind"], r[col]
                ));
            }
        }
    }
    for u in UPSTREAM_2D {
        if !rows.iter().any(|r| r["upstream"] == u) {
            errors.push(format!("the upstream class {u} has no row"));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn rust_source_scans_skip_comments_and_strings() {
    let src = "let a = Value::Point; // Value::Circle\n/* Value::Ray /* nested */ Value::Ray */\nlet s = \"Value::Segment\"; let r = r#\"Value::Line\"#;\nlet q = '\"'; let t = Value::Vector; fn f<'a>(x: &'a str) {} let e = '\\'';";
    let code = code_only(src);
    assert!(code.contains("Value::Point") && code.contains("Value::Vector"));
    for gone in [
        "Value::Circle",
        "Value::Ray",
        "Value::Segment",
        "Value::Line",
        "nested",
    ] {
        assert!(!code.contains(gone), "{gone} in {code}");
    }
    assert!(code.contains("fn f<'a>(x: &'a str)"));
    assert_eq!(grade([true, true]), Some("yes"));
    assert_eq!(grade([true, false]), Some("partial"));
    assert_eq!(grade([false]), Some("no"));
    assert_eq!(grade([]), None);
}

/// Grades one sample's algebra text: 2 proper, 1 the debug form, 0 unknown.
fn text_grade(c: &Construction, id: Id) -> u8 {
    let v = c.value(id);
    if !describe::is_known(&v) {
        0
    } else if v == Value::PieChart && describe::object_text(c, id) == "PieChart" {
        2
    } else if describe::object_text(c, id) == format!("{v:?}") {
        1
    } else {
        2
    }
}

#[test]
fn checked_cells_agree_with_the_code() {
    let rows: Vec<_> = table()
        .into_iter()
        .filter(|r| !r["kernel_kind"].is_empty())
        .collect();
    let s = samples();
    let imported = imported_kinds();
    let draw = code_only(
        &(read("crates/geo-app/src/draw.rs")
            + "\n"
            + &read("crates/geo-app/src/curves/mod.rs")
            + "\n"
            + &read("crates/geo-app/src/draw/image.rs")
            + "\n"
            + &read("crates/geo-app/src/draw/charts.rs")),
    );
    let mut errors = Vec::new();
    for r in &rows {
        let k = &r["kernel_kind"];
        // An exact value, or only whether the code handles the kind at all.
        let mut exact = |col: &str, actual: Option<&str>| match actual {
            Some(a) if a != r[col] => {
                errors.push(format!("{k}: {col} is `{}`, the code says `{a}`", r[col]))
            }
            None if r[col] != "no" => errors.push(format!(
                "{k}: {col} is `{}`, but golden data has no object of the kind to check it with",
                r[col]
            )),
            _ => {}
        };
        exact(
            "kernel_golden",
            Some(if s.recorded.contains(k) { "yes" } else { "no" }),
        );
        if k != "List" {
            exact(
                "svg",
                if matches!(k.as_str(), "Number" | "Boolean") {
                    grade(s.any.get(k).into_iter().flatten().map(|(lines, label)| {
                        let (c, id) = object(lines, label);
                        let frame = View::fit(&c);
                        let without =
                            svg::render_visible(&c, frame, 400, 300, |candidate| candidate != id);
                        let with = svg::render(&c, frame, 400, 300);
                        // A definition by itself contributes a description, which
                        // is not a rendered SVG shape.
                        let graphic = |s: String| s[s.find("<rect").unwrap()..].to_owned();
                        graphic(with) != graphic(without)
                    }))
                } else {
                    s.single.get(k).and_then(|v| {
                        grade(
                            v.iter()
                                .filter(|l| has_something_to_draw(l))
                                .map(|l| svg_draws(l)),
                        )
                    })
                },
            );
        }
        let objects: Vec<(Construction, Id)> = s
            .any
            .get(k)
            .map(|v| v.iter().map(|(l, label)| object(l, label)).collect())
            .unwrap_or_default();
        if k != "List" {
            let pointer_grade = grade(
                objects
                    .iter()
                    .filter(|(c, id)| stroke_has_points(c, *id))
                    .filter(|(c, id)| {
                        c.kind(*id) != Kind::Text
                            || c.text(*id).is_some_and(|text| !text.is_empty())
                    })
                    .filter(|(c, id)| c.kind(*id) != Kind::Angle || c.angle_geometry(*id).is_some())
                    .map(|(c, id)| pointer_hits(c, *id)),
            );
            // All short witnesses must remain hittable. The partial grade is
            // solely for the separate, fixed desktop drawing and picking prefix.
            let pointer_grade = if k == "Locus" {
                assert_eq!(
                    pointer_grade,
                    Some("yes"),
                    "every short golden locus must remain hittable"
                );
                let point = geo_kernel::LocusPoint {
                    x: 0.0,
                    y: 0.0,
                    line_to: false,
                };
                let locus = vec![point; 10_002];
                let shown = geo_app_core::hit::canvas_locus_samples(&locus);
                assert_eq!(shown.len(), 10_001);
                Some("partial")
            } else {
                pointer_grade
            };
            exact("pointer_hit", pointer_grade);
        }
        let texts: Vec<u8> = objects.iter().map(|(c, id)| text_grade(c, *id)).collect();
        exact(
            "algebra_text",
            (!texts.is_empty()).then(|| {
                if texts.iter().all(|&t| t == 2) {
                    "yes"
                } else if texts.iter().all(|&t| t == 0) {
                    "no"
                } else {
                    "partial"
                }
            }),
        );
        let mut handles = |col: &str, actual: bool| {
            if actual != (r[col] != "no") {
                errors.push(format!(
                    "{k}: {col} is `{}`, the code says {}",
                    r[col],
                    if actual { "it handles the kind" } else { "no" }
                ));
            }
        };
        handles("app_draw", draw.contains(&draw_needle(k)));
        handles("import", imported.contains(k));
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn function_tools_cover_the_function_tool_cell() {
    use geo_app_core::tools::{Tool, ToolState};

    let table = Table::parse(&read("docs/kinds.csv"), "docs/kinds.csv").unwrap();
    let tools_support_functions = [Tool::Extremum, Tool::Roots]
        .into_iter()
        .all(|tool| ToolState::new(tool).accepts_object("f", Kind::Function));
    let tool = table.header.iter().position(|name| name == "tool").unwrap();
    let function = table.rows.iter().find(|row| row[0] == "Function").unwrap();
    assert_eq!(
        function[tool],
        if tools_support_functions { "yes" } else { "no" }
    );
}

/// Confirm the list contributes the drawing itself, not a named operand or
/// an SVG description. Probe the actual boundary and an unrelated position.
fn list_graphics(topic: &str, mark: Option<&str>, target: (f64, f64), picked: bool) {
    let key = format!("canvas-list{topic}-001");
    let path = format!("golden/canvas/list{topic}.json");
    let file = model::parse_file(&read(&path), &path).unwrap();
    let case = file.cases.iter().find(|case| case.id == key).unwrap();
    let (c, id) = object(&case.script, "L");
    let expected = &case.snapshots[0]["L"];
    geo_golden::compare::compare_list(&c, expected, Kind::List, c.elements(id)).unwrap();
    let view = View {
        xmin: -4.0,
        xmax: 4.0,
        ymin: -3.0,
        ymax: 3.0,
    };
    let tr = ViewTransform::new(view, 400.0, 300.0);
    let reach = Reach::square(3.0);
    let pick = |(x, y)| hit::hit(&c, id, &tr, tr.world_to_screen(x, y), 3.0, &reach);
    let with = svg::render_visible(&c, view, 400, 300, |candidate| candidate == id);
    let without = svg::render_visible(&c, view, 400, 300, |_| false);
    let drawing = |s: String| s[s.find("<rect").unwrap()..].to_owned();
    let with = drawing(with);
    let without = drawing(without);
    let draws = mark.map_or(with == without, |m| with != without && with.contains(m));
    assert_eq!(
        (draws, pick(target).is_some(), pick((-3.5, -2.5))),
        (true, picked, None),
        "{key}: {with}"
    );
    let row = table().into_iter().find(|r| r["kind"] == "List").unwrap();
    assert_eq!(
        [row["svg"].as_str(), row["pointer_hit"].as_str()],
        ["yes"; 2]
    );
}

#[test]
fn list_graphics_points() {
    list_graphics(
        "points",
        Some(r#"<circle cx="250.00" cy="50.00""#),
        (1.0, 2.0),
        true,
    );
}

#[test]
fn list_graphics_numbers_stay_in_algebra() {
    list_graphics("numbers", None, (1.0, 0.0), false);
}

#[test]
fn list_graphics_segments() {
    list_graphics(
        "segments",
        Some(r#"<line x1="100.00" y1="50.00""#),
        (0.0, 2.0),
        true,
    );
}

#[test]
fn list_graphics_lines() {
    list_graphics("lines", Some(r#"y1="50.00""#), (0.0, 2.0), true);
}

#[test]
fn list_graphics_circles() {
    list_graphics(
        "circles",
        Some(r#"<circle cx="200.00" cy="150.00" r="100.00""#),
        (2.0, 0.0),
        true,
    );
}

#[test]
fn list_graphics_conics() {
    list_graphics("conicmembers", Some("<polyline"), (4.0, 0.0), true);
}

#[test]
fn list_graphics_rays() {
    list_graphics(
        "rays",
        Some(r#"<line x1="100.00" y1="50.00""#),
        (3.0, 2.0),
        true,
    );
}

#[test]
fn list_graphics_named_members_keep_their_own_svg_drawable() {
    list_graphics("named", None, (0.0, 1.0), true);
}

#[test]
fn round_eleven_kind_cells_are_closed() {
    let rows = table();
    for kind in ["InputBox", "List"] {
        let row = rows.iter().find(|r| r["kind"] == kind).unwrap();
        for col in CELLS.into_iter().filter(|col| *col != "export") {
            assert_eq!(row[col], "yes", "{kind}.{col}");
        }
    }
    let missing = rows
        .iter()
        .flat_map(|row| CELLS.map(|col| &row[col]))
        .filter(|cell| cell.as_str() == "no")
        .count();
    assert!(missing <= 30, "{missing} missing cells");
}

#[test]
fn list_graphics_expensive_members_charge_the_shared_budget() {
    let c = script::load(&format!(
        "q: x^2 + 4*y^2 = 1\nbase = {{{}}}\nL = Dilate(base, 2)",
        ["q"; 40].join(", ")
    ))
    .unwrap();
    let id = c.id("L").unwrap();
    let list = c.elements(id).unwrap();
    assert_eq!(list.len(), 40);
    let view = View::fit(&c);
    let svg = svg::render_visible(&c, view, 400, 300, |candidate| candidate == id);
    assert_eq!(svg.matches("<polyline ").count(), 4);

    let tr = ViewTransform::new(view, 400.0, 300.0);
    let reach = Reach::square(3.0);
    let mut remaining = 65;
    let pick =
        |pos, left: &mut usize| hit::hit_with_list_budget(&c, id, &tr, pos, 3.0, &reach, left);
    assert!(pick(tr.world_to_screen(3.0, 2.0), &mut remaining).is_none());
    assert_eq!(remaining, 0);
    assert!(pick(tr.world_to_screen(2.0, 0.0), &mut remaining).is_none());
}

#[test]
fn axis_view_settings_round_trip_without_axis_kind() {
    use geo_app_core::workbench::Workbench;

    let row = table().into_iter().find(|r| r["kind"] == "Axis").unwrap();
    assert_eq!(
        CELLS.map(|col| row[col].as_str()),
        ["no", "no", "yes", "yes", "yes", "no", "yes", "yes"]
    );
    let canvas = code_only(&read("crates/geo-app/src/canvas.rs"));
    assert!(canvas.contains("grid::paint("));
    assert!(canvas.contains("axis_under(&pt, (sx, sy))"));
    assert!(canvas.contains("camera::axis_scale("));
    assert!(canvas.contains("tool == Tool::MoveView"));
    let mut wb = Workbench::default();
    let file = include_bytes!("../../../golden/ggb/saved/saved-angle-001.xml");
    wb.load_for_export(file, "axes").unwrap();
    let mut view = wb.file_view().unwrap();
    assert_eq!(view.axes, [true; 2]);
    view.axes = [false, true];
    let bytes = wb.export_ggb(Some(view)).unwrap();
    wb.load_for_export(&bytes, "axes").unwrap();
    assert_eq!(wb.file_view().unwrap(), view);
    for label in ["xAxis", "yAxis"] {
        let c = wb.construction();
        let axis = c.id(label).unwrap();
        assert_eq!(c.kind(axis), Kind::Line);
        assert!(!c.ids().any(|id| id == axis));
    }
}

#[test]
fn formula_is_latex_text_on_the_canvas_and_in_files() {
    use geo_app_core::workbench::Workbench;

    let row = table()
        .into_iter()
        .find(|r| r["kind"] == "Formula")
        .unwrap();
    assert_eq!(
        CELLS.map(|col| row[col].as_str()),
        ["yes", "yes", "yes", "yes", "no", "yes", "yes", "yes"]
    );
    let draw = code_only(&read("crates/geo-app/src/draw.rs"));
    assert!(draw.contains("Value::Text"));
    let text_draw = code_only(&read("crates/geo-app/src/draw/text.rs"));
    assert!(text_draw.contains("text_presentation(") && text_draw.contains("style.latex"));
    let cases = model::load_all(&repo_root()).unwrap();
    let case = cases
        .iter()
        .find(|case| case.id == "export-texts-001")
        .unwrap();
    let (c, id) = object(&case.script, "t");
    assert_eq!(case.snapshots[0]["t"].kind, Kind::Text);
    assert_eq!(c.kind(id), Kind::Text);
    assert!(c.text_presentation(id).unwrap().latex);
    assert_eq!(text_grade(&c, id), 2);
    assert!(pointer_hits(&c, id));
    let svg = svg::render_visible(&c, View::fit(&c), 400, 300, |candidate| candidate == id);
    assert!(svg.contains("<path"), "{svg}");

    let file = include_bytes!("../../../golden/ggb/saved/saved-texts-001.ggb");
    let mut wb = Workbench::default();
    wb.load_for_export(file, "formula").unwrap();
    let c = wb.construction();
    let id = c.id("formula").unwrap();
    let style = c.text_presentation(id).unwrap();
    assert!(style.latex);
    let source = c.text(id).unwrap().to_owned();
    let bytes = wb.export_ggb(wb.file_view()).unwrap();
    assert_eq!(
        wb.export_ggb_without_definition(wb.file_view(), "formula")
            .unwrap(),
        bytes
    );
    wb.load_for_export(&bytes, "formula").unwrap();
    let c = wb.construction();
    let id = c.id("formula").unwrap();
    assert_eq!(c.text(id), Some(source.as_str()));
    assert_eq!(c.text_presentation(id), Some(style));
}

#[test]
fn list_graphics_many_circles_share_the_curved_member_budget() {
    let c = script::load(&format!(
        "A = (0, 0)\nq = Circle(A, 100)\nbase = {{{}}}\nL = Dilate(base, 2)",
        ["q"; 40].join(", ")
    ))
    .unwrap();
    let id = c.id("L").unwrap();
    let view = View::fit(&c);
    let drawing = svg::render_visible(&c, view, 400, 300, |candidate| candidate == id);
    assert_eq!(drawing.matches("<circle cx=").count(), 4);
}

#[test]
fn list_import_restores_named_and_anonymous_members() {
    let file = include_bytes!("../../../golden/ggb/saved/saved-styles-005.xml");
    let imported = geo_import::import(file).unwrap();
    let c = script::load(&imported.script.join("\n")).unwrap();
    let list = c.elements(c.id("pts").unwrap()).unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list.object(0), c.id("A"));
    assert_eq!(list.object(1), c.id("B"));
    assert_eq!(list.object(2), None);
    assert!(matches!(
        list.get(2),
        Some(geo_kernel::expr::Element::Value(
            Kind::Point,
            Value::Point { x: 2.0, y: 2.0 }
        ))
    ));
}

#[test]
fn editor_tool_cells_match_the_toolbar() {
    use geo_app_core::tools::Tool;
    let rows = table();
    let tools: BTreeSet<_> = Tool::all().map(|tool| format!("{tool:?}")).collect();
    for (kind, makers) in [
        ("Image", &["Image"][..]),
        ("Function", &["FunctionInspector", "FreehandFunction"][..]),
        ("Number", &["Slider"][..]),
        ("Segment", &["Segment", "FreehandShape"][..]),
        ("Conic", &["Ellipse", "FreehandShape"][..]),
        ("Polygon", &["Polygon", "FreehandShape"][..]),
    ] {
        let row = rows.iter().find(|row| row["kind"] == kind).unwrap();
        assert_eq!(row["tool"], "yes", "{kind}.tool");
        for maker in makers {
            assert!(tools.contains(*maker), "{kind}.tool requires {maker}");
        }
    }
}
