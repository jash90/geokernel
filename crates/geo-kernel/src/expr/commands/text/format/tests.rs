use crate::kernel::text::{TextFont, TextOverrides, TextPosition};
use crate::{script, Construction};

// AlgoRotateTextTest.serifPropertyShouldBePersistent
#[test]
fn rotated_text_preserves_its_initial_serif_style() {
    let mut c = script::load("source = \"sample\"\na = 1").unwrap();
    let source = c.id("source").unwrap();
    let paint = |c: &mut Construction, serif| {
        c.apply_text_properties(
            source,
            TextOverrides {
                font: Some(TextFont {
                    serif,
                    ..TextFont::default()
                }),
                ..TextOverrides::default()
            },
        )
        .unwrap();
    };
    paint(&mut c, true);
    let rotated = script::exec(&mut c, "rotated = RotateText(source, a)")
        .unwrap()
        .unwrap();
    assert!(c.text_presentation(rotated).unwrap().serif);
    paint(&mut c, false);
    c.set_number(c.id("a").unwrap(), 2.0).unwrap();
    assert!(c.text_presentation(rotated).unwrap().serif);
}

// AlgoTextTest.testZipCommandWithText checks these three alignments
// with Zip. Here a slider feeds the same argument of Text directly.
#[test]
fn text_alignment_follows_its_numeric_argument() {
    let mut c =
        script::load("A = (0,1)\na = -1\nt = Text(\"sample\", A, true, true, a, 0)").unwrap();
    let t = c.id("t").unwrap();
    let a = c.id("a").unwrap();
    let anchor = TextPosition::Point(c.id("A").unwrap());
    for alignment in [-1, 0, 1] {
        c.set_number(a, f64::from(alignment)).unwrap();
        let presentation = c.text_presentation(t).unwrap();
        assert_eq!(presentation.position, anchor);
        assert_eq!(presentation.horizontal, Some(alignment));
        assert_eq!(presentation.vertical, Some(0));
        assert!(presentation.latex);
    }
}
