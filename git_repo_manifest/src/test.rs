use crate::Manifest;
use insta::{assert_debug_snapshot, glob};
use quick_xml::de::{from_str, DeError};
use std::fs;
#[test]
fn run_test_inputs() {
    glob!("test_inputs/*.xml", |path| {
        let input = fs::read_to_string(path).unwrap();
        let foo: Result<Manifest, DeError> = from_str(&input);

        assert_debug_snapshot!(foo);
    });
}
