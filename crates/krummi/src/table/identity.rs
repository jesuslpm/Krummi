//! Random catalog identities (P12 of the table design, D6 of the execution design): table and column identities and
//! schema states are random 64-bit values from the operating system's generator, drawn again while they equal a
//! value the store knows. A test hook replaces the generator on the calling thread, where a write transaction stays.

use std::cell::RefCell;

use krummi_pager::Result;

/// A source of random bytes, as [`krummi_pager::fill_random`] is.
type Source = Box<dyn FnMut(&mut [u8]) -> Result<()>>;

thread_local! {
    static SOURCE: RefCell<Option<Source>> = const { RefCell::new(None) };
}

/// A random 64-bit identity for which `known` says false: one equal to a known value is drawn again. A failure of
/// the generator is returned as it is, and nothing weaker is drawn (document 11, §2.2).
pub(crate) fn draw(mut known: impl FnMut(u64) -> Result<bool>) -> Result<u64> {
    loop {
        let mut bytes = [0u8; 8];
        fill(&mut bytes)?;
        let id = u64::from_le_bytes(bytes);
        if !known(id)? {
            return Ok(id);
        }
    }
}

fn fill(bytes: &mut [u8]) -> Result<()> {
    SOURCE.with(|source| match source.borrow_mut().as_mut() {
        Some(source) => source(bytes),
        None => krummi_pager::fill_random(bytes),
    })
}

/// Test hooks, not part of the interface.
#[doc(hidden)]
pub mod testing {
    use super::*;

    /// Runs `f` with `source` in place of the operating system's generator for the identities drawn on this thread,
    /// so that a test can make the generator repeat a value or fail.
    pub fn with_identity_source<R>(
        source: impl FnMut(&mut [u8]) -> Result<()> + 'static,
        f: impl FnOnce() -> R,
    ) -> R {
        struct Restore(Option<Source>);
        impl Drop for Restore {
            fn drop(&mut self) {
                let previous = self.0.take();
                SOURCE.with(|source| *source.borrow_mut() = previous);
            }
        }
        let previous = SOURCE.with(|s| s.borrow_mut().replace(Box::new(source)));
        let _restore = Restore(previous);
        f()
    }
}
