//! Compile-fail tests for `#[derive(Operation)]` attribute misuse.

#![cfg(feature = "derive")]

#[test]
fn derive_rejects_bad_input() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
