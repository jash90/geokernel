//! Automatic object labels: the first free one of a kind-specific sequence.

use geo_kernel::kernel::Kind;

const LINE_LETTERS: &str = "fghijklmnopqrstuvwxyzabcde";
const CIRCLE_LETTERS: &str = "cdefghijklmnopqrstuvwxyzab";
const NUMBER_LETTERS: &str = "abcdefghijklmnopqrstuvwxyz";
const VECTOR_LETTERS: &str = "uvwxyzabcdefghijklmnopqrst";
const POINT_LETTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// The letters of the sequence that `kind` draws its labels from.
fn alphabet(kind: Kind) -> &'static str {
    match kind {
        Kind::Point => POINT_LETTERS,
        Kind::Circle => CIRCLE_LETTERS,
        Kind::Number => NUMBER_LETTERS,
        Kind::Vector => VECTOR_LETTERS,
        _ => LINE_LETTERS,
    }
}

/// The first label that `is_taken` does not report as used: points `A`, `B`,
/// ...; circles `c`, `d`, ...; numbers `a`, `b`, ...; vectors `u`, `v`, ...; every other kind `f`, `g`, `h`, .... After the
/// alphabet is used up, the sequence continues with `A_1`, `B_1`, ...
pub fn next_label(kind: Kind, is_taken: impl Fn(&str) -> bool) -> String {
    let letters = alphabet(kind);
    for round in 0.. {
        for ch in letters.chars() {
            let label = if round == 0 {
                ch.to_string()
            } else {
                format!("{ch}_{round}")
            };
            if !is_taken(&label) {
                return label;
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
}
