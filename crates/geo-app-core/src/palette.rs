// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/ConstructionDefaults.java
//   source/shared/common/src/main/java/org/geogebra/common/main/GeoGebraColorConstants.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The default colours of objects, which the logic decides (`style`). The
//! window's theme takes them from here.

use crate::geom::Rgba;

pub const FREE_POINT: Rgba = Rgba::from_rgb(77, 77, 255);
pub const DEPENDENT_POINT: Rgba = Rgba::from_rgb(0x6e, 0x6d, 0x73);
pub const CURVE: Rgba = Rgba::from_rgb(0x1c, 0x1c, 0x1f);
pub const POLYGON: Rgba = Rgba::from_rgb(153, 51, 0);
pub const ANGLE: Rgba = Rgba::from_rgb(0, 100, 0);
