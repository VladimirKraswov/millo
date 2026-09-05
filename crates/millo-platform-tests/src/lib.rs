//! Platform regressions run separately so optimized tests do not rebuild the desktop application.

#[cfg(all(test, target_os = "linux"))]
#[test]
fn patched_glib_string_iterator_supports_all_access_paths() {
    use glib::variant::ToVariant;

    let value = ["first", "middle", "last"].to_variant();
    assert_eq!(
        value.array_iter_str().unwrap().collect::<Vec<_>>(),
        ["first", "middle", "last"]
    );
    assert_eq!(
        value.array_iter_str().unwrap().rev().collect::<Vec<_>>(),
        ["last", "middle", "first"]
    );
    assert_eq!(value.array_iter_str().unwrap().nth(1), Some("middle"));
    assert_eq!(value.array_iter_str().unwrap().nth_back(1), Some("middle"));
    assert_eq!(value.array_iter_str().unwrap().last(), Some("last"));
    let empty = Vec::<String>::new().to_variant();
    assert_eq!(empty.array_iter_str().unwrap().next(), None);
}
