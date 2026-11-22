use geo_import::import;
use geo_kernel::{script, Construction, Kind, Value};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;

const INITIAL: &[u8] = include_bytes!("../../../golden/ggb/saved/saved-images-001.ggb");
const MOVED: &[u8] = include_bytes!("../../../golden/ggb/saved/saved-images-002.ggb");
const INITIAL_VALUES: &str = include_str!("../../../golden/ggb/saved/saved-images-001.snapshot");
const MOVED_VALUES: &str = include_str!("../../../golden/ggb/saved/saved-images-002.snapshot");

#[derive(Deserialize)]
struct Snapshot {
    objects: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct Image {
    defined: bool,
    file: String,
    size: [u32; 2],
    background: bool,
    alpha: f32,
    screen: Option<[f64; 2]>,
    corners: Option<[[f64; 2]; 4]>,
    crop: Option<[f64; 4]>,
}

fn load(bytes: &[u8]) -> Construction {
    let imported = import(bytes).unwrap();
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    assert!(imported.mismatched.is_empty(), "{:?}", imported.mismatched);
    script::load(&imported.script.join("\n")).unwrap()
}

fn check_images(c: &Construction, source: &str) {
    let snapshot: Snapshot = serde_json::from_str(source).unwrap();
    for (label, expected) in snapshot.objects {
        if expected["type"] != "image" {
            continue;
        }
        let expected: Image = serde_json::from_value(expected).unwrap();
        let id = c.id(&label).unwrap();
        let image = c.image(id).unwrap();
        assert_eq!(c.kind(id), Kind::Image);
        assert_eq!(c.value(id).is_defined(), expected.defined);
        assert_eq!(&*image.name, expected.file);
        assert_eq!(image.asset.size(), expected.size);
        assert_eq!(image.style.background, expected.background);
        assert_eq!(image.style.alpha, expected.alpha);
        assert_eq!(image.style.crop, expected.crop);
        assert_eq!(image.placement.is_screen(), expected.screen.is_some());
        let expected_quad = expected.corners.unwrap_or_else(|| {
            let [x, y] = expected.screen.unwrap();
            let [w, h] = expected.size.map(f64::from);
            [[x, y - h], [x + w, y - h], [x + w, y], [x, y]]
        });
        for (actual, expected) in c
            .image_quad(id, 50.0)
            .unwrap()
            .into_iter()
            .flatten()
            .zip(expected_quad.into_iter().flatten())
        {
            assert!(
                (actual - expected).abs() <= 1e-9,
                "{label}: {actual} != {expected}"
            );
        }
    }
}

#[test]
fn saved_raster_images_import_and_replay() {
    let mut c = load(INITIAL);
    check_images(&c, INITIAL_VALUES);
    check_images(&load(MOVED), MOVED_VALUES);
    let image = |label| c.image(c.id(label).unwrap()).unwrap();
    let png = &image("fixedImage").asset;
    assert!(Arc::ptr_eq(png, &image("anchoredImage").asset));
    assert_eq!(&png.rgba()[..4], &[255, 0, 0, 255]);
    assert_eq!(&png.rgba()[24..28], &[0, 255, 0, 128]);
    let gif = &image("animatedImage").asset;
    assert_eq!(&gif.rgba()[..4], &[255, 0, 0, 255]);
    assert_eq!(&gif.rgba()[24..28], &[0, 255, 0, 255]);
    let jpg = &image("backImage").asset;
    assert!(jpg.rgba()[0] > 200 && jpg.rgba()[1] < 30 && jpg.rgba()[3] == 255);
    for label in ["A", "D"] {
        let moved = load(MOVED);
        let Value::Point { x, y } = moved.value(moved.id(label).unwrap()) else {
            panic!("{label}")
        };
        c.move_point(c.id(label).unwrap(), x, y).unwrap();
    }
    check_images(&c, MOVED_VALUES);
}

#[test]
fn an_absent_or_unusable_asset_leaves_other_objects_available() {
    let xml = r#"<geogebra><construction>
      <element type="image" label="picture"><file name="missing.png"/></element>
      <element type="point" label="P"><coords x="1" y="2" z="1"/></element>
    </construction></geogebra>"#;
    let imported = import(xml.as_bytes()).unwrap();
    assert_eq!(imported.skipped.len(), 1);
    assert_eq!(imported.skipped[0].label, "picture");
    assert_eq!(imported.script, ["P = (1, 2)"]);
}

#[test]
fn saved_image_corner_order_and_screen_coordinates() {
    use std::io::{Cursor, Read, Write};

    let mut fixture = zip::ZipArchive::new(Cursor::new(INITIAL)).unwrap();
    let file = load(INITIAL)
        .images()
        .find(|(_, image)| image.name.ends_with(".png"))
        .unwrap()
        .1
        .name
        .clone();
    let mut png = Vec::new();
    fixture
        .by_name(&file)
        .unwrap()
        .read_to_end(&mut png)
        .unwrap();
    let element = |label, placement| {
        format!(
            r#"<element type="image" label="{label}"><file name="{file}"/>{placement}</element>"#
        )
    };
    let mut xml = String::from("<geogebra><construction>");
    xml += r#"<element type="numeric" label="a"><value val="3"/></element>"#;
    xml += &element("ignored", r#"<startPoint number="2" x="0" y="20" z="1"/>"#);
    xml += &element(
        "last",
        r#"<startPoint number="0" x="1" y="2" z="1" absolute="true"/><startPoint number="1" x="3" y="2" z="1"/>"#,
    );
    xml += &element(
        "center",
        r#"<startPoint number="0" x="1" y="2" z="1"/><startPoint number="2" exp="(a+1, 5)"/><centered val="true"/>"#,
    );
    xml += &element("screen", r#"<absoluteScreenLocation x="12.9" y="-7.9"/>"#);
    xml += &element(
        "unusable",
        r#"<startPoint exp="missingPoint"/><file name="unused.png"/>"#,
    );
    xml += "</construction></geogebra>";

    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        ("geogebra.xml", xml.as_bytes()),
        (&file, &png),
        ("unused.png", &png),
    ] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    let archive = writer.finish().unwrap().into_inner();
    let imported = import(&archive).unwrap();
    assert_eq!(imported.skipped.len(), 1, "{:?}", imported.skipped);
    assert_eq!(
        imported
            .script
            .iter()
            .filter(|s| s.starts_with("$imageAsset("))
            .count(),
        1
    );
    let mut c = script::load(&imported.script.join("\n")).unwrap();
    let quad = |c: &Construction, label| c.image_quad(c.id(label).unwrap(), 50.0).unwrap();
    assert_eq!(quad(&c, "ignored")[3], [0.0, 0.0]);
    assert_eq!(quad(&c, "last")[2], [3.0, 2.0]);
    assert_eq!(quad(&c, "screen")[3], [12.0, -7.0]);
    let center = |c: &Construction| {
        let [a, _, b, _] = quad(c, "center");
        [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
    };
    assert_eq!(center(&c), [4.0, 5.0]);
    c.set_number(c.id("a").unwrap(), 5.0).unwrap();
    assert_eq!(center(&c), [6.0, 5.0]);
}

#[test]
fn replay_shares_decoded_assets_and_svg_embeds_each_source_once() {
    let imported = geo_import::import_for_replay(INITIAL).unwrap();
    assert_eq!(imported.image_assets.len(), 3);
    assert!(imported
        .script
        .iter()
        .all(|line| !line.starts_with("$imageAsset(")));
    let source = imported.script.join("\n");
    let c = script::load_with_assets(&source, &imported.image_assets).unwrap();
    for property in &imported.image_assets {
        let script::saved::Property::ImageAsset { name, asset } = property else {
            unreachable!()
        };
        for (_, image) in c.images().filter(|(_, image)| &image.name == name) {
            assert!(Arc::ptr_eq(asset, &image.asset));
        }
    }
    let svg = geo_kernel::svg::render(&c, geo_kernel::svg::View::fit(&c), 800, 600);
    assert_eq!(svg.matches("href=\"data:image/").count(), 3);
    assert_eq!(svg.matches("<use href=\"#raster-").count(), 5);
    assert!(svg.contains("viewBox=\"2 1 8 6\""), "{svg}");
    assert!(
        svg.find("data-label=\"backImage\"").unwrap()
            < svg.find("data-label=\"fixedImage\"").unwrap()
    );
}
