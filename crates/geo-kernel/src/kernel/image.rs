// Placement follows GeoGebra's DrawImage.java, commit 112919f2d450ec7430a5cd3b65dba226fdc43b74.
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

//! Raster assets and the placement of their instances in a construction.

use super::{Construction, Def, Id, Value};
use image::{AnimationDecoder as _, ImageDecoder, ImageFormat, ImageReader, Limits};
use std::collections::{hash_map::RandomState, BTreeMap, HashMap, HashSet};
use std::hash::BuildHasher;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, Weak};

pub const MAX_ASSET_BYTES: u64 = 16 << 20;
const MAX_DIMENSION: u32 = 4096;
const MAX_RASTER_MEMORY: u64 = 128 << 20;

const MAX_ASSET_ENTRIES: usize = 4096;
// Charge cumulative input (before inflation or hex parsing) and attempted pixels.
// Failed decodes do not return work to either allowance.
const MAX_ASSET_INPUT: u64 = 64 << 20;
const MAX_DECODE_BYTES: u64 = 128 << 20;
static NEXT_ASSET: AtomicU64 = AtomicU64::new(0);

fn digest(bytes: &[u8]) -> u64 {
    static STATE: OnceLock<RandomState> = OnceLock::new();
    STATE.get_or_init(RandomState::new).hash_one(bytes)
}

#[derive(Debug)]
pub struct RasterAsset {
    id: u64,
    digest: u64,
    bytes: Vec<u8>,
    size: [u32; 2],
    rgba: Vec<u8>,
}

impl PartialEq for RasterAsset {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id || self.bytes == other.bytes
    }
}

impl Eq for RasterAsset {}

impl RasterAsset {
    pub fn decode(bytes: Vec<u8>) -> Result<Self, String> {
        let mut budget = MAX_DECODE_BYTES;
        Self::decode_with_budget(bytes, MAX_RASTER_MEMORY, &mut budget)
    }

    fn decode_with_budget(
        bytes: Vec<u8>,
        remaining: u64,
        decode_budget: &mut u64,
    ) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_ASSET_BYTES {
            return Err("image asset is too large".into());
        }
        if *decode_budget == 0 || remaining <= Self::source_cost(bytes.len()) {
            return Err("construction image assets exceed the memory or decode limit".into());
        }
        let mut reader = ImageReader::new(Cursor::new(bytes.as_slice()))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(remaining);
        let mut reserve = |width, height, native_bytes: u64| {
            if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
                return Err("image dimensions exceed the limit");
            }
            let rgba = u64::from(width) * u64::from(height) * 4;
            if rgba.max(native_bytes) + Self::source_cost(bytes.len()) > remaining {
                return Err("construction image assets exceed the memory limit");
            }
            *decode_budget = decode_budget
                .checked_sub(native_bytes.max(rgba))
                .ok_or("construction image assets exceed the decode limit")?;
            Ok(())
        };
        let pixels = match reader.format() {
            Some(ImageFormat::Gif) => {
                let mut decoder = image::codecs::gif::GifDecoder::new(Cursor::new(&bytes))
                    .map_err(|e| e.to_string())?;
                decoder.set_limits(limits).map_err(|e| e.to_string())?;
                let (w, h) = decoder.dimensions();
                reserve(w, h, decoder.total_bytes())?;
                decoder
                    .into_frames()
                    .next()
                    .ok_or("GIF has no frames")?
                    .map_err(|e| e.to_string())?
                    .into_buffer()
            }
            Some(ImageFormat::Png) => {
                let header = bytes
                    .get(12..25)
                    .filter(|header| &header[..4] == b"IHDR")
                    .ok_or("invalid PNG header")?;
                let w = u32::from_be_bytes(header[4..8].try_into().expect("width field"));
                let h = u32::from_be_bytes(header[8..12].try_into().expect("height field"));
                let native = u64::from(w.min(MAX_DIMENSION))
                    * u64::from(h.min(MAX_DIMENSION))
                    * if header[12] > 8 { 8 } else { 4 };
                reserve(w, h, native)?;
                let mut decoder = png::Decoder::new(Cursor::new(&bytes));
                decoder.set_ignore_text_chunk(true);
                decoder.set_ignore_iccp_chunk(true);
                decoder.set_transformations(png::Transformations::EXPAND);
                let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
                let mut data = vec![0; reader.output_buffer_size().ok_or("invalid PNG size")?];
                let info = reader.next_frame(&mut data).map_err(|e| e.to_string())?;
                let (w, h) = (info.width, info.height);
                data.truncate(info.buffer_size());
                if info.bit_depth == png::BitDepth::Sixteen {
                    let samples = data.len() / 2;
                    for index in 0..samples {
                        let value = u16::from_be_bytes([data[2 * index], data[2 * index + 1]]);
                        data[index] = ((u32::from(value) + 128) / 257) as u8;
                    }
                    data.truncate(samples);
                }
                if data.capacity() != data.len() {
                    data.shrink_to_fit();
                }
                let bad = "invalid PNG pixels";
                match info.color_type {
                    png::ColorType::Rgba => image::RgbaImage::from_raw(w, h, data).ok_or(bad)?,
                    png::ColorType::Rgb => image::DynamicImage::ImageRgb8(
                        image::RgbImage::from_raw(w, h, data).ok_or(bad)?,
                    )
                    .into_rgba8(),
                    png::ColorType::Grayscale => image::DynamicImage::ImageLuma8(
                        image::GrayImage::from_raw(w, h, data).ok_or(bad)?,
                    )
                    .into_rgba8(),
                    png::ColorType::GrayscaleAlpha => image::DynamicImage::ImageLumaA8(
                        image::GrayAlphaImage::from_raw(w, h, data).ok_or(bad)?,
                    )
                    .into_rgba8(),
                    png::ColorType::Indexed => return Err(bad.into()),
                }
            }
            Some(ImageFormat::Jpeg) => {
                reader.limits(limits);
                let decoder = reader.into_decoder().map_err(|e| e.to_string())?;
                let (w, h) = decoder.dimensions();
                reserve(w, h, decoder.total_bytes())?;
                image::DynamicImage::from_decoder(decoder)
                    .map_err(|e| e.to_string())?
                    .into_rgba8()
            }
            _ => return Err("expected a PNG, JPEG or GIF image asset".into()),
        };
        let size = [pixels.width(), pixels.height()];
        Ok(Self {
            id: NEXT_ASSET.fetch_add(1, Ordering::Relaxed),
            digest: digest(&bytes),
            bytes,
            size,
            rgba: pixels.into_raw(),
        })
    }

    // Original bytes and one serialized copy for a save or conversion.
    fn source_cost(bytes: usize) -> u64 {
        3 * bytes as u64
    }

    pub fn id(&self) -> u64 {
        self.id
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

impl RasterImage {
    pub fn source_rect(&self) -> Option<[f64; 4]> {
        let [w, h] = self.asset.size().map(f64::from);
        let Some([x, y, width, height]) = self.style.crop.filter(|_| !self.placement.is_screen())
        else {
            return Some([0.0, 0.0, w, h]);
        };
        let [x, y, width, height] = [x, y, width, height].map(f64::trunc);
        let [x0, y0] = [x.clamp(0.0, w), y.clamp(0.0, h)];
        let [x1, y1] = [(x + width).clamp(0.0, w), (y + height).clamp(0.0, h)];
        (x1 > x0 && y1 > y0).then_some([x0, y0, x1, y1])
    }
}

pub(super) struct Images {
    assets: HashMap<Arc<str>, (usize, Arc<RasterAsset>)>,
    contents: HashMap<u64, Vec<Weak<RasterAsset>>>,
    bad_contents: HashMap<u64, String>,
    next: usize,
    bytes: u64,
    input_budget: u64,
    attempts: usize,
    decode_budget: u64,
    instances: BTreeMap<Id, RasterImage>,
}

impl Default for Images {
    fn default() -> Self {
        Self {
            assets: HashMap::new(),
            contents: HashMap::new(),
            bad_contents: HashMap::new(),
            next: 0,
            bytes: 0,
            input_budget: MAX_ASSET_INPUT,
            attempts: MAX_ASSET_ENTRIES,
            decode_budget: MAX_DECODE_BYTES,
            instances: BTreeMap::new(),
        }
    }
}

impl Images {
    pub(super) fn asset_count(&self) -> usize {
        self.next
    }

    pub(super) fn discard_since(&mut self, objects: usize, assets: usize) {
        self.instances.split_off(&Id(objects));
        if self.next != assets {
            self.assets.retain(|_, (i, _)| *i < assets);
            self.next = assets;
            self.refresh_accounting();
        }
    }

    fn refresh_accounting(&mut self) {
        self.contents.clear();
        self.bytes = 0;
        for (name, (_, asset)) in &self.assets {
            let key = asset.digest;
            let bucket = self.contents.entry(key).or_default();
            if !bucket.iter().any(|old| old.as_ptr() == Arc::as_ptr(asset)) {
                self.bytes += asset.rgba.len() as u64;
                bucket.push(Arc::downgrade(asset));
            }
            self.bytes += RasterAsset::source_cost(asset.bytes.len() + name.len());
        }
    }
}

impl Construction {
    /// Charge the declared compressed raster size before allocating its bytes.
    pub fn reserve_image_input(&mut self, bytes: u64) -> Result<(), String> {
        if bytes > MAX_ASSET_BYTES {
            return Err("image asset is too large".into());
        }
        self.images.attempts = self
            .images
            .attempts
            .checked_sub(1)
            .ok_or("too many image asset declarations")?;
        let budget = &mut self.images.input_budget;
        *budget = budget
            .checked_sub(bytes.max(1))
            .ok_or("construction image assets exceed the input limit")?;
        Ok(())
    }

    /// Register bytes already charged to the input allowance. The return is
    /// empty when the name and contents were already registered.
    pub fn decode_image_asset(
        &mut self,
        name: Arc<str>,
        bytes: Vec<u8>,
    ) -> Result<Option<Arc<RasterAsset>>, String> {
        if let Some((_, previous)) = self.images.assets.get(&name) {
            return if previous.bytes == bytes {
                Ok(None)
            } else {
                Err("image asset name is already in use".into())
            };
        }
        if self.images.assets.len() >= MAX_ASSET_ENTRIES {
            return Err("too many image assets in construction".into());
        }
        let remaining = (MAX_RASTER_MEMORY - self.images.bytes)
            .checked_sub(RasterAsset::source_cost(name.len()))
            .ok_or("construction image assets exceed the memory limit")?;
        let key = digest(&bytes);
        let shared = self.images.contents.get(&key).and_then(|assets| {
            assets
                .iter()
                .filter_map(Weak::upgrade)
                .find(|asset| asset.bytes == bytes)
        });
        let asset = if let Some(asset) = shared {
            asset
        } else {
            if let Some(error) = self.images.bad_contents.get(&key) {
                return Err(error.clone());
            }
            let work = self.images.decode_budget;
            match RasterAsset::decode_with_budget(bytes, remaining, &mut self.images.decode_budget)
            {
                Ok(asset) => Arc::new(asset),
                Err(error) => {
                    if self.images.decode_budget != work {
                        self.images.bad_contents.insert(key, error.clone());
                    }
                    return Err(error);
                }
            }
        };
        self.register_image_asset(name, asset.clone())?;
        Ok(Some(asset))
    }

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
        if self.images.assets.len() >= MAX_ASSET_ENTRIES {
            return Err("too many image assets in construction".into());
        }
        let key = asset.digest;
        let bucket = self.images.contents.entry(key).or_default();
        let shared = bucket
            .iter()
            .filter_map(Weak::upgrade)
            .find(|old| **old == *asset);
        let mut bytes = RasterAsset::source_cost(asset.bytes.len() + name.len());
        if shared.is_none() {
            bytes += asset.rgba.len() as u64;
        }
        if self.images.bytes + bytes > MAX_RASTER_MEMORY {
            return Err("construction image assets exceed the memory limit".into());
        }
        let asset = shared.unwrap_or_else(|| {
            bucket.push(Arc::downgrade(&asset));
            asset
        });
        self.images.bytes += bytes;
        let index = self.images.next;
        self.images.next += 1;
        self.images.assets.insert(name, (index, asset));
        Ok(())
    }

    /// Release sources that have no instance in this construction.
    pub fn prune_image_assets(&mut self) {
        let images = &mut self.images;
        images.bad_contents.clear();
        if images.assets.is_empty() {
            return;
        }
        let live: HashSet<_> = images.instances.values().map(|image| &image.name).collect();
        let before = images.assets.len();
        images.assets.retain(|name, _| live.contains(name));
        if images.assets.len() != before {
            images.refresh_accounting();
        }
    }

    pub fn image_asset_count(&self) -> usize {
        self.images.assets.len()
    }

    pub fn has_image_asset(&self, name: &str) -> bool {
        self.images.assets.contains_key(name)
    }

    pub fn add_image(
        &mut self,
        label: &str,
        name: &str,
        mut placement: ImagePlacement,
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
        if let ImagePlacement::World {
            corners,
            centered,
            screen_coords,
        } = &mut placement
        {
            if corners[0].is_none() || *centered || *screen_coords {
                corners[1..].fill(None);
            }
        }
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
        self.image_quad_with_scales(id, scale, scale)
    }

    pub fn image_quad_with_scales(
        &self,
        id: Id,
        xscale: f64,
        yscale: f64,
    ) -> Option<[[f64; 2]; 4]> {
        let image = self.image(id)?;
        let [a, right, up] = self.image_frame(image, xscale, yscale)?;
        let area = right[0] * up[1] - right[1] * up[0];
        (area.is_finite() && area != 0.0).then(|| quad(a, right, up))
    }

    pub(crate) fn image_vertex(&self, id: Id, index: i32) -> Option<Value> {
        let image = self.image(id)?;
        if image.placement.is_screen() {
            return None;
        }
        let i = match index {
            1..=4 => 4 - index as usize,
            _ => return None,
        };
        let [_, _, _, _, xs, ys] = self.locus_view().unwrap_or(super::locus::HEADLESS_VIEW);
        let [a, right, up] = self.image_frame(image, xs, ys)?;
        let [x, y] = quad(a, right, up)[i];
        Some(Value::Point { x, y })
    }

    fn image_frame(&self, image: &RasterImage, xscale: f64, yscale: f64) -> Option<[[f64; 2]; 3]> {
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
                    a = a.map(|coordinate| f64::from(coordinate as i32));
                    return Some([a, [width, 0.0], [0.0, -height]]);
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
                    (None, None) => ([width / xscale, 0.0], [0.0, height / yscale]),
                };
                if centered {
                    a[0] -= right[0] * 0.5;
                    a[1] -= up[1] * 0.5;
                }
                (a, right, up)
            }
        };
        Some([a, right, up])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_dimensions_are_charged_before_reading_image_data() {
        // Only write IHDR. A decode that reads any pixels would report
        // a truncated image instead of the budget diagnostic.
        let mut header = Vec::new();
        let mut encoder = png::Encoder::new(&mut header, 4096, 4096);
        encoder.set_color(png::ColorType::Rgba);
        let writer = encoder.write_header().unwrap();
        // PNG's writer may append IEND on drop.
        drop(writer);
        let mut work = MAX_DECODE_BYTES;
        let err = RasterAsset::decode_with_budget(header.clone(), 32 << 20, &mut work).unwrap_err();
        assert!(err.contains("memory limit"), "{err}");
        assert_eq!(work, MAX_DECODE_BYTES);
        work = 32 << 20;
        let err =
            RasterAsset::decode_with_budget(header, MAX_RASTER_MEMORY, &mut work).unwrap_err();
        assert!(err.contains("decode limit"), "{err}");
    }
}
