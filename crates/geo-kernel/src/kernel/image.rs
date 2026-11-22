// Placement follows GeoGebra's DrawImage.java, commit 112919f2d450ec7430a5cd3b65dba226fdc43b74.
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

//! Raster assets and the placement of their instances in a construction.

use super::{Construction, Def, Id, Value};
use image::{AnimationDecoder as _, ImageDecoder as _, ImageFormat, ImageReader, Limits};
use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::sync::Arc;

pub const MAX_ASSET_BYTES: u64 = 16 << 20;
const MAX_DIMENSION: u32 = 4096;
const MAX_RASTER_MEMORY: u64 = 128 << 20;

#[derive(Debug, PartialEq, Eq)]
pub struct RasterAsset {
    bytes: Vec<u8>,
    size: [u32; 2],
    rgba: Vec<u8>,
}

impl RasterAsset {
    pub fn decode(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_ASSET_BYTES {
            return Err("image asset is too large".into());
        }
        let mut reader = ImageReader::new(Cursor::new(bytes.as_slice()))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(MAX_RASTER_MEMORY);
        reader.limits(limits.clone());
        let pixels = match reader.format() {
            Some(ImageFormat::Gif) => {
                let mut decoder = image::codecs::gif::GifDecoder::new(Cursor::new(&bytes))
                    .map_err(|e| e.to_string())?;
                decoder.set_limits(limits).map_err(|e| e.to_string())?;
                decoder
                    .into_frames()
                    .next()
                    .ok_or("GIF has no frames")?
                    .map_err(|e| e.to_string())?
                    .into_buffer()
            }
            Some(ImageFormat::Png | ImageFormat::Jpeg) => {
                reader.decode().map_err(|e| e.to_string())?.into_rgba8()
            }
            _ => return Err("expected a PNG, JPEG or GIF image asset".into()),
        };
        let size = [pixels.width(), pixels.height()];
        Ok(Self {
            bytes,
            size,
            rgba: pixels.into_raw(),
        })
    }

    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImageCorner<T = Id> {
    Coordinates([f64; 2]),
    Point(T),
}

/// Lower-left, lower-right and upper-left corners; centered images use
/// the first field for the center.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImagePlacement<T = Id> {
    Screen([f64; 2]),
    World {
        corners: [Option<ImageCorner<T>>; 3],
        centered: bool,
        screen_coords: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageStyle {
    pub background: bool,
    pub interpolate: bool,
    pub alpha: f32,
    /// Source pixel rectangle (x, y, width, height), from the upper-left.
    pub crop: Option<[f64; 4]>,
}

impl Default for ImageStyle {
    fn default() -> Self {
        Self {
            background: false,
            interpolate: true,
            alpha: 1.0,
            crop: None,
        }
    }
}

pub struct RasterImage {
    pub asset: Arc<RasterAsset>,
    pub name: Arc<str>,
    pub placement: ImagePlacement,
    pub style: ImageStyle,
}

#[derive(Default)]
pub(super) struct Images {
    assets: HashMap<Arc<str>, (usize, Arc<RasterAsset>)>,
    bytes: u64,
    instances: BTreeMap<Id, RasterImage>,
}

impl Images {
    pub(super) fn asset_count(&self) -> usize {
        self.assets.len()
    }

    pub(super) fn discard_since(&mut self, objects: usize, assets: usize) {
        self.instances.split_off(&Id(objects));
        if self.assets.len() > assets {
            self.assets.retain(|_, (i, asset)| {
                if *i >= assets {
                    self.bytes -= asset.rgba.len() as u64 + asset.bytes.len() as u64;
                    false
                } else {
                    true
                }
            });
        }
    }
}

impl Construction {
    pub fn register_image_asset(
        &mut self,
        name: Arc<str>,
        asset: Arc<RasterAsset>,
    ) -> Result<(), String> {
        if let Some((_, previous)) = self.images.assets.get(&name) {
            return if previous == &asset {
                Ok(())
            } else {
                Err("image asset name is already in use".into())
            };
        }
        let bytes = asset.rgba.len() as u64 + asset.bytes.len() as u64;
        if self.images.bytes + bytes > MAX_RASTER_MEMORY {
            return Err("construction image assets exceed the memory limit".into());
        }
        self.images.bytes += bytes;
        let index = self.images.assets.len();
        self.images.assets.insert(name, (index, asset));
        Ok(())
    }

    pub fn add_image(
        &mut self,
        label: &str,
        name: &str,
        placement: ImagePlacement,
        style: ImageStyle,
    ) -> Result<Id, String> {
        let (name, (_, asset)) = self
            .images
            .assets
            .get_key_value(name)
            .ok_or("missing image asset")?;
        let (name, asset) = (name.clone(), asset.clone());
        let mut parents = [Id(0); 3];
        let mut count = 0;
        let coordinate = |coords: [f64; 2]| {
            if coords.into_iter().all(f64::is_finite) {
                Ok(())
            } else {
                Err("image coordinates must be finite")
            }
        };
        match placement {
            ImagePlacement::Screen(coords) => coordinate(coords)?,
            ImagePlacement::World { corners, .. } => {
                for corner in corners.into_iter().flatten() {
                    match corner {
                        ImageCorner::Coordinates(coords) => coordinate(coords)?,
                        ImageCorner::Point(id) => {
                            parents[count] = id;
                            count += 1;
                        }
                    }
                }
            }
        }
        if !(0.0..=1.0).contains(&style.alpha)
            || style
                .crop
                .is_some_and(|r| !r.into_iter().all(f64::is_finite))
        {
            return Err("invalid image style".into());
        }
        let id = self
            .add_with_list(label, &parents[..count], Def::Image)
            .map_err(|e| e.to_string())?;
        self.images.instances.insert(
            id,
            RasterImage {
                asset,
                name,
                placement,
                style,
            },
        );
        Ok(id)
    }

    pub fn image(&self, id: Id) -> Option<&RasterImage> {
        self.images.instances.get(&id)
    }

    pub fn images(&self) -> impl ExactSizeIterator<Item = (Id, &RasterImage)> {
        self.images.instances.iter().map(|(&id, image)| (id, image))
    }

    /// Corners in texture order (top-left clockwise) and in the requested
    /// screen or world coordinates. The view scale is needed only when
    /// the image has fewer than two world anchors.
    pub fn image_quad(&self, id: Id, scale: f64) -> Option<[[f64; 2]; 4]> {
        let image = self.image(id)?;
        let [width, height] = image.asset.size.map(f64::from);
        let resolve = |corner: ImageCorner| match corner {
            ImageCorner::Coordinates(coords) => Some(coords),
            ImageCorner::Point(p) => match self.value(p) {
                Value::Point { x, y } => Some([x, y]),
                _ => None,
            },
        };
        let (a, right, up) = match image.placement {
            ImagePlacement::Screen(a) => (a, [width, 0.0], [0.0, -height]),
            ImagePlacement::World {
                corners,
                centered,
                screen_coords,
            } => {
                let [a, b, d] = corners;
                let mut a = a.map_or(Some([0.0, 0.0]), resolve)?;
                if screen_coords {
                    a = a.map(f64::trunc);
                    return Some(quad(a, [width, 0.0], [0.0, -height]));
                }
                let (right, up) = match (b, d) {
                    (Some(b), Some(d)) => (delta(resolve(b)?, a), delta(resolve(d)?, a)),
                    (Some(b), None) => {
                        let r = delta(resolve(b)?, a);
                        (r, [-r[1] * height / width, r[0] * height / width])
                    }
                    (None, Some(d)) => {
                        let u = delta(resolve(d)?, a);
                        ([u[1] * width / height, -u[0] * width / height], u)
                    }
                    (None, None) => ([width / scale, 0.0], [0.0, height / scale]),
                };
                if centered {
                    a[0] -= right[0] * 0.5;
                    a[1] -= up[1] * 0.5;
                }
                (a, right, up)
            }
        };
        let area = right[0] * up[1] - right[1] * up[0];
        (area.is_finite() && area != 0.0).then(|| quad(a, right, up))
    }
}

impl<T> ImagePlacement<T> {
    pub fn is_screen(&self) -> bool {
        matches!(
            self,
            Self::Screen(_)
                | Self::World {
                    screen_coords: true,
                    ..
                }
        )
    }
}

fn delta(to: [f64; 2], from: [f64; 2]) -> [f64; 2] {
    [to[0] - from[0], to[1] - from[1]]
}

fn quad(a: [f64; 2], r: [f64; 2], u: [f64; 2]) -> [[f64; 2]; 4] {
    let b = [a[0] + r[0], a[1] + r[1]];
    [[a[0] + u[0], a[1] + u[1]], [b[0] + u[0], b[1] + u[1]], b, a]
}
