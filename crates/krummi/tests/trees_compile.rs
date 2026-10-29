//! The compile-time guarantees of trees (Trees PRD: the gate before 5.1, B1, B9; Trees and key operations PRD:
//! A10, A14; AC-A3, AC-A20, AC-B10, AC-B11, and AC20 of the root PRD; Multi-value trees PRD: M4, M9, AC-M9 and the
//! lifetime part of AC-M16).
//!
//! What must compile is in `tests/ui/trees_pass` and only compiles: the bodies of the trees arrive with T3. What
//! must not compile is in `tests/ui/trees`, each case named after what it proves.

#[test]
fn ac_a20_ac_b11_tree_handles_and_references_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/trees_pass/*.rs");
    cases.compile_fail("tests/ui/trees/*.rs");
}
