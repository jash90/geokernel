//! The textures egui asked for, kept as their current images, so that a
//! renderer that starts late (or renders seldom) gets what it needs without
//! the texture changes of every frame piling up in between.

use eframe::egui::{
    epaint::{ImageData, ImageDelta},
    ColorImage, TextureId, TextureOptions, TexturesDelta,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct Textures {
    current: BTreeMap<TextureId, (ColorImage, TextureOptions)>,
    /// Changed since a renderer last took them.
    dirty: BTreeSet<TextureId>,
    freed: BTreeSet<TextureId>,
}

impl Textures {
    /// Applies the texture changes of one frame and empties `delta`.
    pub(super) fn apply(&mut self, delta: &mut TexturesDelta) {
        // Deterministic order: by texture id.
        let mut set: Vec<_> = delta.set.drain().collect();
        set.sort_by_key(|(id, _)| *id);
        for (id, list) in set {
            for d in list {
                let ImageData::Color(image) = d.image;
                match d.pos {
                    None => {
                        self.current.insert(id, ((*image).clone(), d.options));
                    }
                    Some([x, y]) => {
                        if let Some((target, options)) = self.current.get_mut(&id) {
                            blit(target, &image, x, y);
                            *options = d.options;
                        }
                    }
                }
            }
            self.dirty.insert(id);
            self.freed.remove(&id);
        }
        for id in delta.free.drain() {
            self.current.remove(&id);
            self.dirty.remove(&id);
            self.freed.insert(id);
        }
    }

    /// What a renderer needs: every texture (`all`, for a new renderer) or
    /// those changed since the last call, as whole images, and the freed
    /// ones.
    pub(super) fn take(&mut self, all: bool) -> TexturesDelta {
        let mut out = TexturesDelta::default();
        let ids: Vec<TextureId> = if all {
            self.current.keys().copied().collect()
        } else {
            self.dirty.iter().copied().collect()
        };
        for id in ids {
            if let Some((image, options)) = self.current.get(&id) {
                out.set
                    .insert(id, [ImageDelta::full(image.clone(), *options)].into());
            }
        }
        out.free = std::mem::take(&mut self.freed).into_iter().collect();
        self.dirty.clear();
        out
    }

    /// Pixels held.
    pub(super) fn pixels(&self) -> usize {
        self.current.values().map(|(i, _)| i.pixels.len()).sum()
    }
}

fn blit(target: &mut ColorImage, patch: &ColorImage, x: usize, y: usize) {
    let [tw, th] = target.size;
    let [pw, ph] = patch.size;
    for row in 0..ph.min(th.saturating_sub(y)) {
        let width = pw.min(tw.saturating_sub(x));
        let from = row * pw;
        let to = (y + row) * tw + x;
        target.pixels[to..to + width].copy_from_slice(&patch.pixels[from..from + width]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;

    #[test]
    fn patches_land_in_the_image_and_frees_forget_it() {
        let id = TextureId::Managed(0);
        let mut t = Textures::default();
        let mut d = TexturesDelta::default();
        let full = ColorImage::new([4, 3], vec![Color32::BLACK; 12]);
        d.set
            .insert(id, [ImageDelta::full(full, TextureOptions::LINEAR)].into());
        t.apply(&mut d);
        let patch = ColorImage::new([2, 2], vec![Color32::WHITE; 4]);
        d.set.insert(
            id,
            [ImageDelta::partial([3, 2], patch, TextureOptions::LINEAR)].into(),
        );
        t.apply(&mut d);
        let mut out = t.take(false);
        let ImageData::Color(img) = &out.set[&id][0].image;
        // Clipped to the image: only (3, 2) is covered.
        let white: Vec<usize> = (0..12)
            .filter(|&i| img.pixels[i] == Color32::WHITE)
            .collect();
        assert_eq!(white, [11]);
        out.clear();
        assert!(t.take(false).is_empty());
        let mut all = t.take(true);
        assert_eq!(all.set.len(), 1);
        all.clear();
        d.free.insert(id);
        t.apply(&mut d);
        assert_eq!(t.pixels(), 0);
        let mut freed = t.take(true);
        assert!(freed.free.contains(&id));
        freed.clear();
    }
}
