use std::sync::OnceLock;

use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::Face;

fn fonts() -> &'static [FontRef<'static>; 2] {
    static FONTS: OnceLock<[FontRef<'static>; 2]> = OnceLock::new();
    FONTS.get_or_init(|| {
        [
            epaint_default_fonts::UBUNTU_LIGHT,
            epaint_default_fonts::HACK_REGULAR,
        ]
        .map(|data| FontRef::new(data).expect("valid bundled font"))
    })
}

/// Some variant shapes have no cmap entry in either bundled text face.
pub(crate) fn bundled_symbol(ch: char) -> char {
    match ch {
        'ϑ' => 'θ',
        'ϕ' => 'φ',
        'ϖ' => 'π',
        'ϰ' => 'κ',
        'ϱ' => 'ρ',
        'ϵ' => 'ε',
        '∖' => '\\',
        '∥' => '‖',
        _ => ch,
    }
}

pub(crate) fn has_symbol(ch: char) -> bool {
    fonts()
        .iter()
        .any(|font| font.charmap().map(bundled_symbol(ch)).is_some())
}

pub(crate) fn drawn_symbol(ch: char) -> bool {
    matches!(ch, 'ℏ' | '⊥' | '⋮' | '⋱')
}

pub(crate) fn glyph(c: char, face: Face) -> (&'static FontRef<'static>, GlyphId) {
    let c = bundled_symbol(c);
    let fonts = fonts();
    let order = if face.monospace { [1, 0] } else { [0, 1] };
    order
        .into_iter()
        .find_map(|i| fonts[i].charmap().map(c).map(|id| (&fonts[i], id)))
        .unwrap_or_else(|| {
            (
                &fonts[0],
                fonts[0].charmap().map('□').unwrap_or(GlyphId::NOTDEF),
            )
        })
}

pub(crate) fn advance(font: &FontRef<'_>, id: GlyphId, size: f32) -> f32 {
    font.glyph_metrics(Size::new(size), LocationRef::default())
        .advance_width(id)
        .unwrap_or(0.55 * size)
}

// These pairs cover the most visible collisions in the bundled sans face.
// Longer text runs use the native font shaper on the canvas.
pub(crate) fn kern(left: char, right: char, size: f32, face: Face) -> f32 {
    if face.monospace {
        return 0.0;
    }
    match (left, right) {
        ('A', 'V' | 'W' | 'Y' | 'T') | ('V' | 'W' | 'Y' | 'T', 'A' | 'a' | 'o') => -0.05 * size,
        ('f', ')' | ']' | '}') => 0.05 * size,
        _ => 0.0,
    }
}

pub(crate) fn measure(text: &str, size: f32, face: Face) -> (f32, f32, f32) {
    let mut width = 0.0;
    let mut height = 0.0_f32;
    let mut depth = 0.0_f32;
    let mut last = None;
    for c in text.chars() {
        let (font, id) = glyph(c, face);
        let metrics = font.glyph_metrics(Size::new(size), LocationRef::default());
        if let Some(old) = last {
            width += kern(old, c, size, face);
        }
        width += metrics.advance_width(id).unwrap_or(0.55 * size);
        if let Some(bounds) = metrics.bounds(id) {
            height = height.max(bounds.y_max);
            depth = depth.max(-bounds.y_min);
        }
        last = Some(c);
    }
    // Keep space for the baseline even in an empty run.
    let slant = if face.italic { 0.15 * height } else { 0.0 };
    let bold = if face.bold { 0.035 * size } else { 0.0 };
    (width + slant + bold, height, depth)
}
