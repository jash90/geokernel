// The named palette uses data from GeoGebra's GeoGebraColorConstants.java
// and GColor.java (112919f2d450ec7430a5cd3b65dba226fdc43b74).
// Copyright (c) GeoGebra GmbH. Licensed under the EUPL 1.2.

use super::commands::number;
use crate::Color;

pub(super) fn color(model: Option<&str>, raw: &str) -> Option<Color> {
    let raw = raw.trim();
    let model = model.or_else(|| raw.contains(',').then_some("RGB"));
    if matches!(model, Some("rgb" | "RGB" | "gray")) {
        let mut values = raw.split(',').map(number);
        let first = values.next()??;
        let channels = if model == Some("gray") {
            [first; 3]
        } else {
            [first, values.next()??, values.next()??]
        };
        let scale = if model == Some("RGB") { 1.0 } else { 255.0 };
        return values
            .next()
            .is_none()
            .then(|| channels.map(|c| (c * scale).clamp(0.0, 255.0).round() as u8));
    }
    if model.is_some_and(|m| m != "HTML") {
        return None;
    }
    let hex = raw.strip_prefix('#').unwrap_or(raw);
    if hex.len() == 6 && hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        let n = u32::from_str_radix(hex, 16).ok()?;
        return Some([(n >> 16) as u8, (n >> 8) as u8, n as u8]);
    }
    named_color(&raw.to_ascii_lowercase())
}

pub(super) fn named_color(name: &str) -> Option<Color> {
    Some(match name {
        "black" => [0, 0, 0],
        "white" => [255, 255, 255],
        "red" => [255, 0, 0],
        "green" => [0, 255, 0],
        "blue" => [0, 0, 255],
        "cyan" => [0, 255, 255],
        "magenta" => [255, 0, 255],
        "yellow" => [255, 255, 0],
        "orange" => [255, 127, 0],
        "pink" => [255, 192, 203],
        "purple" => [128, 0, 128],
        "brown" => [153, 51, 0],
        "lime" => [191, 255, 0],
        "violet" => [127, 0, 255],
        "lightorange" => [255, 239, 213],
        "lightyellow" => [255, 250, 205],
        "aqua" => [188, 212, 230],
        "lightpurple" => [204, 204, 255],
        "lightviolet" => [224, 176, 255],
        "turquoise" => [175, 238, 238],
        "lightgreen" => [208, 240, 192],
        "darkblue" => [28, 57, 187],
        "maroon" => [128, 0, 0],
        "gold" => [255, 215, 0],
        "darkgreen" => [0, 100, 0],
        "lightblue" => [125, 125, 255],
        "indigo" => [75, 0, 130],
        "crimson" => [220, 20, 60],
        "gray" | "grey" => [128; 3],
        "lightgray" | "lightgrey" | "gray3" => [160; 3],
        "darkgray" | "darkgrey" | "gray7" => [32; 3],
        "silver" | "gray6" => [64; 3],
        "gray1" => [224; 3],
        "gray2" => [192; 3],
        "gray5" => [96; 3],
        _ => return None,
    })
}
