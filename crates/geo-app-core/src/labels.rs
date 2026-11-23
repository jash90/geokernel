// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPolygon.java (getDefaultLabel)
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/LabelManager.java (getNextIndexedLabel)
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/LabelType.java (the label alphabets)
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoElement.java (getDefaultLabel)
//   source/shared/editor-base/src/main/java/org/geogebra/editor/share/util/Greek.java (getGreekLowerCaseNoPi)
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Automatic object labels: the first free one of a kind-specific sequence
//! (spec E9).

use geo_kernel::kernel::Kind;

/// `LabelType.pointLabels`: no `X` and `Y`.
const POINT_LETTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWZ";
/// `LabelType.functionLabels`.
const FUNCTION_LETTERS: &str = "fghpqrst";
/// `LabelType.lineLabels`: no `o` and no `u` to `z`.
const LINE_LETTERS: &str = "fghijklmnpqrstabcde";
/// `LabelType.vectorLabels`: no `o` and no `x` to `z`.
const VECTOR_LETTERS: &str = "uvwabcdefghijklmnpqrst";
/// `LabelType.conicLabels`.
const CONIC_LETTERS: &str = "cdefghkpqrst";
/// `LabelType.lowerCaseLabels`: `a` to `w`.
const LOWER_CASE_LETTERS: &str = "abcdefghijklmnopqrstuvw";
/// `LabelType.greekLowerCaseLabels`: the lower-case Greek letters without
/// pi and the final sigma, phi in its straight form.
const GREEK_LETTERS: &str = "αβγδεζηθικλμνξορστυϕχψω";

/// The letters of the sequence that `kind` draws its labels from
/// (`GeoElement.getDefaultLabel`): segments, rays and polylines are named
/// like lines, arcs and sectors like conics; numbers, booleans and anything
/// else take the lower-case letters (a polygon has its own names,
/// [`polygon_label`]).
fn alphabet(kind: Kind) -> &'static str {
    match kind {
        Kind::Point => POINT_LETTERS,
        Kind::Line | Kind::Segment | Kind::Ray | Kind::PolyLine => LINE_LETTERS,
        Kind::Vector => VECTOR_LETTERS,
        Kind::Circle | Kind::Conic | Kind::Arc | Kind::Sector => CONIC_LETTERS,
        Kind::Function => FUNCTION_LETTERS,
        _ => LOWER_CASE_LETTERS,
    }
}

/// The first label that `is_taken` does not report as used: points `A`, `B`,
/// ...; lines `f`, `g`, ...; conics `c`, `d`, ...; vectors `u`, `v`, ...;
/// functions `f`, `g`, `h`, `p`, ...; numbers `a`, `b`, ...; lists `l1`,
/// `l2`, ...; pen strokes `stroke1`, `stroke2`, .... After the alphabet is used up, the sequence continues with
/// `A_1`, `B_1`, ... and from the tenth round `A_{10}`.
pub fn next_label(kind: Kind, is_taken: impl Fn(&str) -> bool) -> String {
    // `GeoList.getDefaultLabel`: `getNextNumberedLabel("l")`; a pen stroke
    // is numbered after its English name, `stroke1` in the recorded
    // upstream test of `PenStroke()`.
    let numbered = match kind {
        Kind::List => Some("l"),
        Kind::PenStroke => Some("stroke"),
        _ => None,
    };
    if let Some(prefix) = numbered {
        return (1..)
            .map(|i| format!("{prefix}{i}"))
            .find(|l| !is_taken(l))
            .unwrap_or_default();
    }
    next_from(alphabet(kind), is_taken)
}

/// A polygon is named by its shape and the first free index: `t1` for a
/// triangle, `q1` for a quadrilateral, `poly1` for any other count.
pub fn polygon_label(vertices: usize, is_taken: impl Fn(&str) -> bool) -> String {
    let prefix = match vertices {
        3 => "t",
        4 => "q",
        _ => "poly",
    };
    (1..)
        .map(|i| format!("{prefix}{i}"))
        .find(|l| !is_taken(l))
        .unwrap_or_default()
}

/// Angles are named with Greek letters: `α`, `β`, `γ`, ...
pub fn next_greek(is_taken: impl Fn(&str) -> bool) -> String {
    next_from(GREEK_LETTERS, is_taken)
}

/// The label of an image under a transformation: the source label with a
/// prime (`A'`, `A''`, ...). A source label that cannot take a prime (an
/// indexed one such as `t_{1}`) falls back to the next label of `kind`.
pub fn primed(source: &str, kind: Kind, is_taken: impl Fn(&str) -> bool) -> String {
    let simple = source
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '\'');
    if simple {
        let mut label = format!("{source}'");
        while is_taken(&label) {
            label.push('\'');
            if label.len() > source.len() + 8 {
                break;
            }
        }
        if !is_taken(&label) {
            return label;
        }
    }
    next_label(kind, is_taken)
}

/// `LabelManager.getNextIndexedLabel`: the letters, then `X_1` ... `X_9`,
/// then `X_{10}` ...; an indexed label is free only when both its forms
/// (`X_1` and `X_{1}`) are.
fn next_from(letters: &str, is_taken: impl Fn(&str) -> bool) -> String {
    for round in 0u32.. {
        for ch in letters.chars() {
            if round == 0 {
                if !is_taken(&ch.to_string()) {
                    return ch.to_string();
                }
                continue;
            }
            let (plain, braced) = (format!("{ch}_{round}"), format!("{ch}_{{{round}}}"));
            if !is_taken(&plain) && !is_taken(&braced) {
                return if round < 10 { plain } else { braced };
            }
        }
    }
    unreachable!("the label sequence is infinite")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequences_start_where_the_reference_does() {
        assert_eq!(next_label(Kind::Point, |_| false), "A");
        assert_eq!(next_label(Kind::Line, |_| false), "f");
        assert_eq!(next_label(Kind::Segment, |_| false), "f");
        assert_eq!(next_label(Kind::Ray, |_| false), "f");
        assert_eq!(next_label(Kind::Circle, |_| false), "c");
        assert_eq!(next_label(Kind::PenStroke, |_| false), "stroke1");
        assert_eq!(next_label(Kind::PenStroke, |l| l == "stroke1"), "stroke2");
    }

    #[test]
    fn angles_are_greek_and_images_are_primed() {
        assert_eq!(next_greek(|_| false), "α");
        assert_eq!(next_greek(|l| l == "α"), "β");
        assert_eq!(primed("A", Kind::Point, |_| false), "A'");
        assert_eq!(primed("A", Kind::Point, |l| l == "A'"), "A''");
        assert_eq!(primed("t_{1}", Kind::Line, |_| false), "f");
    }

    #[test]
    fn the_first_free_label_is_used_even_after_gaps() {
        let taken = ["A", "B", "D"];
        assert_eq!(next_label(Kind::Point, |l| taken.contains(&l)), "C");
        let taken = ["f", "g", "h", "i"];
        assert_eq!(next_label(Kind::Line, |l| taken.contains(&l)), "j");
        let taken = ["c", "d"];
        assert_eq!(next_label(Kind::Circle, |l| taken.contains(&l)), "e");
    }

    #[test]
    fn kinds_are_independent() {
        let taken = ["A", "f"];
        assert_eq!(next_label(Kind::Circle, |l| taken.contains(&l)), "c");
        assert_eq!(next_label(Kind::Point, |l| taken.contains(&l)), "B");
    }

    #[test]
    fn exhausted_alphabets_continue_with_subscripts() {
        let letters: Vec<String> = POINT_LETTERS.chars().map(String::from).collect();
        assert_eq!(
            next_label(Kind::Point, |l| letters.iter().any(|t| t == l)),
            "A_1"
        );
        let mut taken: Vec<String> = letters;
        taken.push("A_1".into());
        assert_eq!(
            next_label(Kind::Point, |l| taken.iter().any(|t| t == l)),
            "B_1"
        );
    }

    /// The reference's sequences (`LabelType`): points skip `X` and `Y`,
    /// lines skip `o` and `u` to `z`, conics are `c d e f g h k p q r s t`,
    /// vectors skip `o` and `x` to `z`, functions are `f g h p q r s t`,
    /// numbers stop at `w`, angles skip pi and phi is straight. Fails with
    /// the old whole-alphabet sequences.
    #[test]
    fn sequences_follow_the_reference() {
        let after = |kind: Kind, last: &str| {
            let letters = alphabet(kind);
            let end = letters.find(last).unwrap() + last.len();
            next_label(kind, |l| {
                letters[..end].contains(l) && l.chars().count() == 1
            })
        };
        assert_eq!(after(Kind::Point, "W"), "Z");
        assert_eq!(after(Kind::Line, "n"), "p");
        assert_eq!(after(Kind::Line, "t"), "a");
        assert_eq!(after(Kind::Segment, "e"), "f_1");
        assert_eq!(after(Kind::Circle, "h"), "k");
        assert_eq!(after(Kind::Conic, "t"), "c_1");
        assert_eq!(after(Kind::Arc, "c"), "d");
        assert_eq!(after(Kind::Vector, "w"), "a");
        assert_eq!(after(Kind::Vector, "n"), "p");
        assert_eq!(after(Kind::Function, "h"), "p");
        assert_eq!(after(Kind::Number, "w"), "a_1");
        assert_eq!(next_label(Kind::List, |l| l == "l1"), "l2");
        let greek: Vec<char> = GREEK_LETTERS.chars().collect();
        assert_eq!(greek.len(), 23);
        assert_eq!(greek[14..17], ['ο', 'ρ', 'σ']);
        assert_eq!(greek[19], 'ϕ');
    }

    /// An indexed label is free only when both forms are, and from the
    /// tenth round it takes braces (`getNextIndexedLabel`).
    #[test]
    fn indexed_labels_check_both_forms_and_brace_from_ten() {
        let one = |l: &str| l.chars().count() == 1;
        assert_eq!(next_label(Kind::Point, |l| one(l) || l == "A_{1}"), "B_1");
        let rounds = |l: &str| {
            one(l)
                || l.split_once('_')
                    .and_then(|(_, n)| n.trim_matches(['{', '}']).parse::<u32>().ok())
                    .is_some_and(|n| n < 10)
        };
        assert_eq!(next_label(Kind::Point, rounds), "A_{10}");
    }
}
