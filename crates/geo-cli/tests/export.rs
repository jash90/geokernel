use std::fs;
use std::process::Command;

#[test]
fn exports_the_upstream_midpoint_and_segment_forms() {
    // CommandsTest.cmdMidpoint and cmdSegment (verified t-pairs).
    let dir = std::env::temp_dir().join(format!("geokernel-export-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("in.geo");
    let output = dir.join("out.ggb");
    fs::write(
        &input,
        "A = (0,0)\nB = (2,2)\nM = Midpoint(A,B)\nC = (1,1)\nD = (2,0.5)\ns = Segment(C,D)\n",
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_geokernel"))
        .arg("export")
        .arg(input)
        .arg(&output)
        .output()
        .unwrap();
    let bytes = fs::read(output);
    fs::remove_dir_all(dir).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = bytes.unwrap();
    let imported = geo_import::import(&bytes).unwrap();
    assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
    let c = geo_kernel::script::load(&imported.script.join("\n")).unwrap();
    assert_eq!(
        c.value(c.id("M").unwrap()),
        geo_kernel::Value::Point { x: 1.0, y: 1.0 }
    );
    assert_eq!(
        c.value(c.id("s").unwrap()),
        geo_kernel::Value::Segment {
            x1: 1.0,
            y1: 1.0,
            x2: 2.0,
            y2: 0.5
        }
    );
}

#[test]
fn export_refuses_to_replace_its_input() {
    let path = std::env::temp_dir().join(format!(
        "geokernel-export-source-{}.ggb",
        std::process::id()
    ));
    let source = br#"<geogebra><construction><element type="point" label="A"><coords x="1" y="2" z="1"/></element></construction></geogebra>"#;
    fs::write(&path, source).unwrap();
    let alias = path.with_extension("alias.ggb");
    fs::hard_link(&path, &alias).unwrap();
    for destination in [&path, &alias] {
        let result = Command::new(env!("CARGO_BIN_EXE_geokernel"))
            .arg("export")
            .arg(&path)
            .arg(destination)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert_eq!(fs::read(destination).unwrap(), source);
    }
    fs::remove_file(alias).unwrap();
    fs::remove_file(path).unwrap();
}
