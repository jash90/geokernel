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
