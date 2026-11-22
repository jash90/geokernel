//! The journal syntax for raster assets and their instances.

use super::{object, saved_argument, split_call};
use crate::kernel::control::{decode_text, encode_text};
use crate::kernel::image::{ImageCorner, ImagePlacement, ImageStyle};
use crate::{Construction, Id, Kind};

pub const FORM: &str = "$image";

pub fn definition(name: &str, placement: ImagePlacement<&str>, style: ImageStyle) -> String {
    let corner = |corner| match corner {
        None => "_".into(),
        Some(ImageCorner::Coordinates([x, y])) => format!("xy({x}, {y})"),
        Some(ImageCorner::Point(label)) => format!("point({})", encode_text(label)),
    };
    let placement = match placement {
        ImagePlacement::Screen([x, y]) => format!("screen({x}, {y})"),
        ImagePlacement::World {
            corners,
            centered,
            screen_coords,
        } => {
            let [a, b, d] = corners.map(corner);
            format!("world({centered}, {screen_coords}, {a}, {b}, {d})")
        }
    };
    let crop = style.crop.map_or_else(
        || "_".into(),
        |[x, y, w, h]| format!("crop({x}, {y}, {w}, {h})"),
    );
    format!(
        "{FORM}({}, {}, {}, {}, {crop}, {placement})",
        encode_text(name),
        style.alpha,
        style.background,
        style.interpolate
    )
}

fn fields<const N: usize>(args: [&str; N]) -> Result<[f64; N], String> {
    let mut values = [0.0; N];
    for (value, arg) in values.iter_mut().zip(args) {
        *value = arg.parse().map_err(|_| "invalid image coordinate")?;
    }
    Ok(values)
}

fn corner(c: &mut Construction, arg: &str) -> Result<Option<ImageCorner>, String> {
    if arg == "_" {
        return Ok(None);
    }
    let Some((form, args)) = split_call(arg) else {
        return Err("invalid image corner".into());
    };
    Ok(Some(match (form, args.as_slice()) {
        ("point", [source]) => {
            let source = decode_text(source)?;
            let label = saved_argument(c, &source, 0)?;
            let id = object(c, &label)?;
            if c.kind(id) != Kind::Point {
                return Err("expected a point for the image corner".into());
            }
            ImageCorner::Point(id)
        }
        ("xy", &[x, y]) => ImageCorner::Coordinates(fields([x, y])?),
        _ => return Err("invalid image corner".into()),
    }))
}

pub(super) fn define(c: &mut Construction, label: &str, rhs: &str) -> Result<Id, String> {
    let checkpoint = c.checkpoint();
    let result = define_inner(c, label, rhs);
    if result.is_err() {
        c.discard_since(checkpoint);
    }
    result
}

fn define_inner(c: &mut Construction, label: &str, rhs: &str) -> Result<Id, String> {
    let Some((FORM, args)) = split_call(rhs) else {
        return Err("invalid image definition".into());
    };
    let [name, alpha, background, interpolate, crop, placement] = args.as_slice() else {
        return Err("invalid image fields".into());
    };
    let boolean = |s: &str| s.parse::<bool>().map_err(|_| "invalid image flag");
    let crop = if *crop == "_" {
        None
    } else if let Some(("crop", values)) = split_call(crop) {
        Some(fields(
            values.try_into().map_err(|_| "invalid image crop")?,
        )?)
    } else {
        return Err("invalid image crop".into());
    };
    let (form, args) = split_call(placement).ok_or("invalid image placement")?;
    let name = decode_text(name)?;
    if !c.has_image_asset(&name) {
        return Err("missing image asset".into());
    }
    let placement = match (form, args.as_slice()) {
        ("screen", &[x, y]) => ImagePlacement::Screen(fields([x, y])?),
        ("world", [centered, screen_coords, a, b, d]) => {
            let centered = boolean(centered)?;
            let screen_coords = boolean(screen_coords)?;
            let a = corner(c, a)?;
            let (b, d) = if a.is_some() && !centered && !screen_coords {
                (corner(c, b)?, corner(c, d)?)
            } else {
                (None, None)
            };
            ImagePlacement::World {
                centered,
                screen_coords,
                corners: [a, b, d],
            }
        }
        _ => return Err("invalid image placement".into()),
    };
    c.add_image(
        label,
        &name,
        placement,
        ImageStyle {
            alpha: alpha.parse().map_err(|_| "invalid image opacity")?,
            background: boolean(background)?,
            interpolate: boolean(interpolate)?,
            crop,
        },
    )
}

/// Point expressions in an image definition are encoded in the journal.
pub fn corner_sources(rhs: &str) -> impl Iterator<Item = String> + '_ {
    rhs.strip_prefix("$image(")
        .into_iter()
        .flat_map(|rhs| rhs.split("point(").skip(1))
        .filter_map(|source| decode_text(source.split_once(')')?.0).ok())
}
