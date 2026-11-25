//! The worker of the vector indexes (VI3, VI4, VI9, VI12 and VI13 of the vector indexes PRD; document 12 of the table
//! design, *The worker*): one thread of the store that trains, builds, reconciles and rebuilds them in short batches of
//! its own writes, which take the write turn as any writer.

use std::sync::atomic::{AtomicBool, Ordering};

/// The vector work of a store.
#[derive(Default)]
pub(crate) struct VectorWork {
    /// A test hook: while set, the worker takes no batch.
    paused: AtomicBool,
}

impl VectorWork {
    pub(crate) fn pause(&self, paused: bool) {
        self.paused.store(paused, Ordering::Release);
    }

    #[allow(dead_code, reason = "used by the worker (T6)")]
    pub(crate) fn paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }
}

impl crate::Store {
    /// A test hook, not part of the interface: pauses the vector indexes' worker, which takes no batch while paused
    /// (the batch it holds goes on), or resumes it.
    #[doc(hidden)]
    pub fn pause_vector_work(&self, paused: bool) {
        self.shared().vectors.pause(paused);
    }
}
