//! The journal syntax for raster assets and their instances.

use super::{object, split_call};
use crate::kernel::control::{decode_text, encode_text};
use crate::kernel::image::{ImageCorner, ImagePlacement, ImageStyle};
use crate::{Construction, Id};

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

fn corner(c: &Construction, arg: &str) -> Result<Option<ImageCorner>, String> {
    if arg == "_" {
        return Ok(None);
    }
    let Some((form, args)) = split_call(arg) else {
        return Err("invalid image corner".into());
    };
    Ok(Some(match (form, args.as_slice()) {
        ("point", [name]) => ImageCorner::Point(object(c, &decode_text(name)?)?),
        ("xy", &[x, y]) => ImageCorner::Coordinates(fields([x, y])?),
        _ => return Err("invalid image corner".into()),
    }))
}

pub(super) fn define(c: &mut Construction, label: &str, rhs: &str) -> Result<Id, String> {
    let Some((FORM, args)) = split_call(rhs) else {
        return Err("invalid image definition".into());
    };
    let [name, alpha, background, interpolate, crop, placement] = args.as_slice() else {
        return Err("invalid image fields".into());
    };
    let boolean = |s: &str| s.parse().map_err(|_| "invalid image flag");
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
    let placement = match (form, args.as_slice()) {
        ("screen", &[x, y]) => ImagePlacement::Screen(fields([x, y])?),
        ("world", [centered, screen_coords, a, b, d]) => ImagePlacement::World {
            centered: boolean(centered)?,
            screen_coords: boolean(screen_coords)?,
            corners: [corner(c, a)?, corner(c, b)?, corner(c, d)?],
        },
        _ => return Err("invalid image placement".into()),
    };
    c.add_image(
        label,
        &decode_text(name)?,
        placement,
        ImageStyle {
            alpha: alpha.parse().map_err(|_| "invalid image opacity")?,
            background: boolean(background)?,
            interpolate: boolean(interpolate)?,
            crop,
        },
    )
}
