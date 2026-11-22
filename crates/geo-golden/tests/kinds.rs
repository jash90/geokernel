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
//!   functions) is not a sample, as there is nothing to draw;
//! * `pointer_hit`, exact: `geo_app_core::hit::hit` hits the object somewhere
//!   on a 400 x 300 view, graded over the samples as for `svg`;
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
//! * `export`, exact: `yes` for editor objects serialized from their own
//!   stored data and references (raster images, buttons and input boxes).
//!   `partial` for construction kinds whose export also depends on supported
//!   journal forms and command history. `no` for kinds without a writer.
//!   The Java oracle checks a defined example of each `yes` and `partial` kind.
//!
//! Tool coverage is declared in the table.
//! A row without a kernel kind is `no` in every column.
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
use geo_kernel::{script, Construction, Id, Value};
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
    "arcs-points-001",
    "arcs-points-002",
    "arcs-points-003",
    "arcs-points-004",
    "arcs-points-005",
    "arcs-points-006",
    "arcs-points-007",
    "arcs-points-008",
    "arcs-points-009",
    "arcs-points-010",
    "arcs-undefined-002",
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
    "cas-coefficients-005",
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
    "cas-simplify-004",
    "cas-solve-018",
    "cas-solve-019",
    "cas-solve-020",
    "cas-solve-022",
    "cas-solve-023",
    "cas-solve-024",
    "cas-solve-026",
    "cas-solve-027",
    "cas-solve-028",
    "cas-solve-030",
    "cas-solve-031",
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
    "commontangents-arc-001",
    "commontangents-arc-002",
    "commontangents-arc-003",
    "commontangents-arc-004",
    "commontangents-circles-001",
    "commontangents-circles-002",
    "commontangents-circles-003",
    "commontangents-conics-001",
    "commontangents-conics-002",
    "commontangents-conics-003",
    "conic-diameter-001",
    "conic-diameter-002",
    "conic-diameter-003",
    "conic-diameter-004",
    "conic-diameter-005",
    "conic-diameter-006",
    "conic-diameter-008",
    "conic-diameter-009",
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
    "conicforms-coefficients-001",
    "conicforms-coefficients-002",
    "conicforms-coefficients-003",
    "conicforms-list-001",
    "conicforms-list-002",
    "conicforms-list-003",
    "conicforms-list-004",
    "conicforms-pole-001",
    "conicforms-pole-002",
    "conicpaths-degenerate-001",
    "conicpaths-degenerate-002",
    "conicpaths-degenerate-003",
    "conicpaths-degenerate-005",
    "conicpaths-degenerate-006",
    "conicpaths-ellipse-001",
    "conicpaths-ellipse-002",
    "conicpaths-ellipse-003",
    "conicpaths-ellipse-004",
    "conicpaths-ellipse-005",
    "conicpaths-locus-001",
    "conicpaths-locus-002",
    "conicpaths-locus-003",
    "conicpaths-locus-004",
    "conicpaths-locus-005",
    "conicreview-circlepole-001",
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
    "corners-dilate-001",
    "corners-dilate-002",
    "corners-dilate-003",
    "corners-dilate-004",
    "corners-geometry-001",
    "corners-growth-001",
    "corners-growth-002",
    "corners-inequality-001",
    "corners-inequality-003",
    "corners-inequality-004",
    "corners-inequality-005",
    "corners-matrix-001",
    "corners-matrix-002",
    "corners-matrix-003",
    "corners-matrix-004",
    "corners-raster-001",
    "corners-vertex-001",
    "corners-vertex-004",
    "corners-vertex-005",
    "corners-vertex-006",
    "corners-vertex-007",
    "corners-vertex-008",
    "corners-vertex-009",
    "curvepairs-bounded-001",
    "curvepairs-coincident-001",
    "curvepairs-coincident-002",
    "curvepairs-independent-001",
    "curvepairs-order-001",
    "curvepairs-parabola-001",
    "curvepairs-parabola-002",
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
    "discrete-hull-001",
    "discrete-hull-003",
    "discrete-hull-004",
    "discrete-hull-005",
    "discrete-hull-006",
    "discrete-hull-007",
    "discrete-hullreview-001",
    "discrete-hullreview-002",
    "discrete-hullreview-003",
    "discrete-hullreview-004",
    "discrete-hullreview-005",
    "discrete-hullreview-006",
    "discrete-path-001",
    "discrete-path-002",
    "discrete-tour-006",
    "discrete-tree-008",
    "discrete-voronoi-006",
    "export-conics-001",
    "export-curve-001",
    "export-geometry-001",
    "export-paths-001",
    "export-texts-001",
    "export-transforms-001",
    "expr-apply-015",
    "expr-cmd-011",
    "expr-fncmd-006",
    "expr-fnundef-009",
    "expr-label-001",
    "expr-label-002",
    "expr-label-003",
    "expr-label-004",
    "expr-label-005",
    "expr-label-006",
    "expr-negpow-007",
    "expr-point-001",
    "financial-future-010",
    "financial-future-011",
    "financial-future-012",
    "funcanalysis-curvatureupstream-002",
    "funcanalysis-curvatureupstream-005",
    "funcanalysis-curvatureupstream-008",
    "funcanalysis-curveedge-003",
    "funcanalysis-curveedge-004",
    "funcanalysis-curveedge-005",
    "funcanalysis-curveedge-006",
    "funcanalysis-curveedge-007",
    "funcanalysis-curveedge-008",
    "funcanalysis-curveedge-009",
    "funcanalysis-curveedge-010",
    "funcanalysis-curveedge-011",
    "funcanalysis-curveedge-012",
    "funcanalysis-curveupstream-002",
    "funcanalysis-curveupstream-003",
    "funcanalysis-forms-012",
    "funcanalysis-forms-015",
    "funcanalysis-forms-016",
    "funcanalysis-forms-023",
    "funcanalysis-nsode-002",
    "funcanalysis-nsode-003",
    "funcanalysis-reviewode-002",
    "funcanalysis-splineupstream-001",
    "funcanalysis-splineupstream-002",
    "funcanalysis-splineupstream-003",
    "funcanalysis-splineupstream-005",
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
    "geomclosure-comparison-001",
    "geomclosure-comparison-002",
    "geomclosure-comparison-003",
    "geomclosure-curvelength-001",
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
    "geomextras-circumferencepolygon-001",
    "geomextras-conicparts-001",
    "geomextras-conicparts-002",
    "geomextras-pathandregion-001",
    "geomextras-polygonlist-001",
    "geomextras-polylineflag-001",
    "geomextras-polylinelist-001",
    "geomextras-polylinelist-002",
    "geomextras-predicates-001",
    "geomforms-area-002",
    "geomforms-closest-008",
    "geomforms-ray-001",
    "geomforms-ray-002",
    "geomforms-ray-003",
    "geomforms-tangent-001",
    "geomforms-tangent-002",
    "geomformsreview-angle-001",
    "geomformsreview-booleanfunction-001",
    "geomformsreview-function-001",
    "geomformsreview-functionofy-001",
    "geomformsreview-odenamed-001",
    "geomformsreview-shapes-001",
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
    "georestclip-paths-003",
    "georestclip-polyline-001",
    "georestclip-polyline-002",
    "georestclip-polyline-003",
    "georestclip-polyline-004",
    "georestclip-region-003",
    "georestintersect-affine-001",
    "georestintersect-affine-002",
    "georestintersect-affine-003",
    "georestintersect-affine-004",
    "georestintersect-bounded-001",
    "georestintersect-growth-001",
    "georestsmall-dilatecurve-001",
    "georestsmall-dilatecurve-002",
    "georestsmall-dilatecurve-003",
    "georestsmall-ifgeometry-002",
    "georestsmall-matrixcurve-001",
    "georestsmall-matrixcurve-002",
    "georestsmall-matrixcurve-003",
    "georestsmall-matrixfunction-001",
    "georestsmall-matrixfunction-002",
    "georestsmall-matrixfunction-003",
    "georestsmall-matrixfunction-004",
    "georestsmall-surdpoint-001",
    "georestsmall-surdpoint-002",
    "georesttangent-curve-003",
    "importgaps-curve-001",
    "intersectconics-conicboundary-001",
    "intersectconics-conicboundary-002",
    "intersectconics-conicboundary-003",
    "intersectconics-conicboundary-004",
    "intersectconics-conicboundary-005",
    "intersectconics-conicboundary-006",
    "intersectconics-lineboundary-001",
    "intersectconics-lineboundary-002",
    "intersectconics-lineboundary-003",
    "intersectconics-lineboundary-004",
    "intersectconics-lineboundary-006",
    "intersectconics-pathboundary-001",
    "intersectconics-pathboundary-002",
    "intersectconics-pathboundary-003",
    "intersectconics-pathboundary-004",
    "intersectconics-pathboundary-005",
    "intersectfunctions-line-011",
    "intersections-boundary-001",
    "intersections-boundary-002",
    "intersections-boundary-003",
    "intersections-boundary-004",
    "intersections-boundary-005",
    "intersections-boundary-006",
    "intersections-boundary-007",
    "intersections-boundary-008",
    "intersections-boundsreview-002",
    "intersections-limited-001",
    "intersectreview-growth-003",
    "lists-index-016",
    "lists-locusfirst-001",
    "lists-locusfirst-002",
    "lists-locusfirst-003",
    "lists-locusfirst-004",
    "lists-locusfirst-005",
    "lists-locusfirst-006",
    "lists-objects-027",
    "lists-textelem-001",
    "lists-textelem-002",
    "lists-textelem-004",
    "lists-textelem-006",
    "lists-textelem-007",
    "lists-textelem-008",
    "lists-textelem-009",
    "lists-textelem-011",
    "lists-textelem-012",
    "lists-textelem-013",
    "lists-textelem-016",
    "lists-textelem-017",
    "lists-textelem-018",
    "lists-textelem-019",
    "lists-textlists-020",
    "locus-arc-001",
    "locus-arc-002",
    "locus-arc-003",
    "locus-circle-001",
    "locus-circle-002",
    "locus-circle-003",
    "locus-circle-004",
    "locus-circle-005",
    "locus-circle-006",
    "locus-circle-007",
    "locus-line-001",
    "locus-line-002",
    "locus-line-003",
    "locus-line-004",
    "logic-boolconst-001",
    "logic-boolconst-002",
    "matrixdilate-applypoly-003",
    "misc-name-001",
    "misc-sum-004",
    "misc-sum-005",
    "paths-closest-003",
    "paths-ray-001",
    "paths-ray-002",
    "paths-ray-003",
    "paths-ray-004",
    "paths-ray-005",
    "paths-ray-006",
    "points-closestray-001",
    "points-closestray-002",
    "points-closestray-003",
    "points-closestray-004",
    "points-closestray-005",
    "points-closestray-006",
    "predicates-concurrent-010",
    "predicates-concurrent-012",
    "relationreview-arcs-001",
    "relationreview-asymptotes-001",
    "relationreview-conics-001",
    "relationreview-conics-002",
    "relationreview-paths-001",
    "relationreview-paths-002",
    "relationreview-polygons-001",
    "relationreview-segments-001",
    "relationreview-vectors-001",
    "textconcat-geometry-001",
    "textconcat-geometry-002",
    "textconcat-lists-001",
    "textconcat-other-001",
    "textconcat-other-002",
    "textconcat-other-003",
    "textequations-typed-001",
    "textformat-formula-001",
    "textformat-formula-002",
    "textformat-formula-003",
    "textformat-formula-004",
    "textformat-formula-005",
    "textformat-formula-006",
    "textformat-formula-007",
    "textformat-formula-008",
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
    let rasters = include_bytes!("../../../golden/ggb/saved/saved-images-001.ggb");
    let imported = geo_import::import(rasters).unwrap();
    let c = script::load(&imported.script.join("\n")).unwrap();
    s.any.insert(
        "Image".into(),
        c.images()
            .map(|(id, _)| (imported.script.clone(), c.label(id).into()))
            .collect(),
    );
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
/// independently of the plotter; an isolated defined point is not drawn).
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
        !matches!(c.value(id), Value::Function)
            || (0..400).any(|i| inside(id, i) && inside(id, i + 1))
    })
}

/// The construction of a script and the id of `label` in it.
fn object(lines: &[String], label: &str) -> (Construction, Id) {
    let c = build(lines);
    let id = c.id(label).unwrap_or_else(|| panic!("no object `{label}`"));
    (c, id)
}

/// Whether a pointer anywhere on a 400 x 300 view (probed every 3 pixels)
/// hits the object.
fn pointer_hits(c: &Construction, id: Id) -> bool {
    let tr = ViewTransform::new(View::fit(c), 400.0, 300.0);
    let reach = Reach::square(3.0);
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
        let export = match kk.as_str() {
            "" => "no",
            "Image" | "Button" | "InputBox" => "yes",
            "Point" | "Line" | "Segment" | "Ray" | "Vector" | "Circle" | "Conic" | "Arc"
            | "Sector" | "Number" | "Angle" | "Boolean" | "Function" | "Polygon" | "PolyLine"
            | "List" | "Text" | "Locus" | "Curve" | "FunctionNVar" | "PieChart" => "partial",
            _ => "no",
        };
        if r["export"] != export {
            errors.push(format!(
                "row `{}`: export is `{}`, expected `{export}`",
                r["kind"], r["export"]
            ));
        }
        for col in CELLS {
            if !["yes", "partial", "no"].contains(&r[col].as_str()) {
                errors.push(format!("row `{}`: {col} is `{}`", r["kind"], r[col]));
            }
            if kk.is_empty() && r[col] != "no" {
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
        exact(
            "svg",
            s.single.get(k).and_then(|v| {
                grade(
                    v.iter()
                        .filter(|l| has_something_to_draw(l))
                        .map(|l| svg_draws(l)),
                )
            }),
        );
        let objects: Vec<(Construction, Id)> = s
            .any
            .get(k)
            .map(|v| v.iter().map(|(l, label)| object(l, label)).collect())
            .unwrap_or_default();
        exact(
            "pointer_hit",
            grade(objects.iter().map(|(c, id)| pointer_hits(c, *id))),
        );
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
