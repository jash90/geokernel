//! An infinite number is a defined value, so every command that takes a
//! number can be handed one. This runs every form of
//! `tools/command-matrix/forms.csv` with `Infinity` and `-Infinity` in each
//! number position and checks that the kernel answers in bounded time:
//! a refusal or an undefined result is fine, a loop that follows the count
//! to infinity is not.

use geo_golden::{forms, repo_root};
use geo_kernel::script::{exec, load};
use std::time::{Duration, Instant};

/// One call may take this long at most. The budgets of the kernel end
/// long before it; a debug build under load still needs the headroom.
const BOUND: Duration = Duration::from_secs(20);

/// The objects the forms are filled with, one per argument kind.
const BASE: &str = "A = (1, 2)\n\
    B = (4, 3)\n\
    C = (2, 5)\n\
    n = 3\n\
    p = 1 / 0\n\
    q = -1 / 0\n\
    l = {1, 2, 3}\n\
    f(x) = x^2\n\
    k = Circle(A, 2)\n\
    b = true\n\
    g = Line(A, B)\n\
    P = Polygon(A, B, C)\n\
    v = Vector(A, B)\n\
    t = \"abc\"\n\
    w = Curve(cos(s), sin(s), s, 0, 6)\n\
    L = PolyLine(A, B, C)\n\
    h = Segment(A, B)\n\
    r = Ray(A, B)\n\
    alpha = Angle(A, B, C)\n";

/// The label of [`BASE`] for an argument token, `None` for a kind the
/// base construction has no object of.
fn object(token: &str) -> Option<&'static str> {
    Some(match token {
        "number" | "any" => "n",
        "point" => "A",
        "list" => "l",
        "function" => "f",
        "conic" | "circle" | "path" | "region" => "k",
        "boolean" => "b",
        "line" => "g",
        "polygon" => "P",
        "vector" => "v",
        "text" => "t",
        "curve" => "w",
        "polyline" => "L",
        "segment" => "h",
        "ray" => "r",
        "angle" => "alpha",
        _ => return None,
    })
}

#[test]
fn an_infinite_number_argument_ends_in_bounded_time() {
    let text = std::fs::read_to_string(repo_root().join("tools/command-matrix/forms.csv"))
        .expect("forms.csv");
    let all = forms::parse_forms(&text).expect("forms.csv parses");
    let mut calls = 0usize;
    let mut ran = 0usize;
    let mut slowest = (Duration::ZERO, String::new());
    for (command, entry) in &all {
        for form in &entry.forms {
            let tokens: Vec<&str> = form
                .split(',')
                .filter(|token| *token != "..." && *token != "()")
                .collect();
            let Some(labels) = tokens
                .iter()
                .map(|token| object(token))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            // each number position alone, then all of them together
            let positions: Vec<usize> = (0..tokens.len())
                .filter(|&i| matches!(tokens[i], "number" | "any"))
                .collect();
            let mut variants: Vec<Vec<usize>> = positions.iter().map(|&i| vec![i]).collect();
            if positions.len() > 1 {
                variants.push(positions.clone());
            }
            for infinite in ["p", "q"] {
                for variant in &variants {
                    let args: Vec<&str> = labels
                        .iter()
                        .enumerate()
                        .map(|(i, label)| {
                            if variant.contains(&i) {
                                infinite
                            } else {
                                label
                            }
                        })
                        .collect();
                    let line = format!("out = {command}({})", args.join(", "));
                    let mut c = load(BASE).expect("the base construction loads");
                    let start = Instant::now();
                    // the answer does not matter here, only that there is one
                    ran += usize::from(exec(&mut c, &line).is_ok());
                    let took = start.elapsed();
                    assert!(took < BOUND, "`{line}` took {took:?}");
                    if took > slowest.0 {
                        slowest = (took, line);
                    }
                    calls += 1;
                }
            }
        }
    }
    println!(
        "{calls} calls with an infinite argument, {ran} accepted, the slowest `{}` in {:?}",
        slowest.1, slowest.0
    );
    // most forms accept their arguments: the loop above is not vacuous
    assert!(calls > 1000 && ran > 800, "{ran} of {calls} calls ran");
}
