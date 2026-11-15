//! Expression trees live in a side table, so the per-object types stay small.

#[test]
fn values_and_definitions_stay_small() {
    assert!(std::mem::size_of::<geo_kernel::Value>() <= 48);
    assert!(std::mem::size_of::<geo_kernel::Def>() <= 48);
    assert_eq!(std::mem::size_of::<geo_kernel::Kind>(), 1);
}
