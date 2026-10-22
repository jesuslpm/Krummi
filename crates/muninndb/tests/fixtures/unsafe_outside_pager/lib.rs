// Fixture for the unsafe-code policy test: unsafe code outside the Pager.
pub fn first_byte(bytes: &[u8]) -> u8 {
    unsafe { *bytes.as_ptr() }
}
