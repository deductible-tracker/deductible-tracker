use chrono::Utc;
use std::path::{Path, PathBuf};

// Test the donations helper by using the crate path
#[test]
fn parse_utc_from_opt_string_tests() {
    let s = Some("2020-01-02T03:04:05+00:00".to_string());
    let dt = deductible_tracker::db::oracle::donations::parse_utc_from_opt_string(s);
    let expected = chrono::DateTime::parse_from_rfc3339("2020-01-02T03:04:05+00:00")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(dt, expected);

    let before = Utc::now();
    let dt2 = deductible_tracker::db::oracle::donations::parse_utc_from_opt_string(Some(
        "not-a-date".to_string(),
    ));
    let after = Utc::now();
    assert!(dt2 >= before && dt2 <= after);

    let before2 = Utc::now();
    let dt3 = deductible_tracker::db::oracle::donations::parse_utc_from_opt_string(None);
    let after2 = Utc::now();
    assert!(dt3 >= before2 && dt3 <= after2);
}

// Include asset_helpers.rs in a local test module so we can test its free functions
mod asset_helpers_test {
    use std::fs;
    use std::path::{Path, PathBuf};
    include!("../src/main_sections/assets/asset_helpers.rs");
    #[test]
    fn has_fingerprint_suffix_and_relative() {
        let p = Path::new("static/assets/app-1a2b3c4d5e6f.css");
        assert!(has_fingerprint_suffix(p));

        let p2 = Path::new("static/assets/app.css");
        assert!(!has_fingerprint_suffix(p2));

        let cur = Path::new("static/js/views");
        let res = resolve_js_relative(cur, "./foo/bar.js").unwrap();
        assert_eq!(res, PathBuf::from("static/js/views/foo/bar.js"));

        let cur2 = Path::new("static/js/views/routes");
        let res2 = resolve_js_relative(cur2, "../lib/util.js").unwrap();
        assert_eq!(res2, PathBuf::from("static/js/views/lib/util.js"));

        let from = Path::new("a/b/c");
        let to = Path::new("a/d/e.js");
        let rel = relative_path(from, to);
        assert_eq!(rel, PathBuf::from("../../d/e.js"));

        // minify_js_asset smoke test
        let src = "function add(a, b) { return a + b; }";
        let m = minify_js_asset(src);
        assert!(!m.is_empty());
    }
}
