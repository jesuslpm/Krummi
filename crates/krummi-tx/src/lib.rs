//! Transactions of Krummi (module 2): read transactions that see a snapshot, one write transaction at a time and
//! the queue of those waiting for it (Transactions PRD, T1–T18).
//!
//! The crate is internal to the engine. Its page API (`page`, `page_mut`, `allocate`) is for the engine's own
//! modules and tests; applications use the `krummi` crate, which exposes only opening, committing and aborting
//! transactions (T1).

mod transaction;

use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use krummi_pager::format::{HEADER_PAGES, PAGE_PAYLOAD_SIZE};
pub use krummi_pager::{Error, ErrorKind, Info, OpenOptions, Result};
use krummi_pager::{Pager, Scratch};
pub use transaction::{ReadTransaction, WriteTransaction};

/// Page that keeps the transaction layer's own data: the number of the next page to reserve. It is an ordinary
/// data page, not part of the header (T17); a blank one is a store where nothing was committed yet.
pub const META_PAGE: u64 = HEADER_PAGES;
/// First page that write transactions reserve.
pub const FIRST_PAGE: u64 = META_PAGE + 1;
/// Bytes of a page that a transaction can use.
pub const PAYLOAD_SIZE: usize = PAGE_PAYLOAD_SIZE;

/// A point of a commit or of the hand-off of the write turn where tests can stop or interrupt the engine
/// (AC-T5, AC-T18, AC-T19, AC-E6). The first three come before the commit's point of no return, when it enters the
/// journal's queue (E2 of the grouped journal PRD); the others after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Point {
    /// The commit starts.
    CommitFlush,
    /// The commit is about to seal its pages.
    CommitSeal,
    /// The commit is about to write its pages to the temporary area and the journal's queue and publish them.
    CommitPublish,
    /// The commit passed its point of no return and is about to release the turn.
    CommitQueued,
    /// A write transaction ended and others are waiting for the turn.
    Handoff,
    /// The commit released the turn and is about to wait until it is durable.
    CommitWait,
}

#[cfg(feature = "test-hooks")]
type Hook = std::sync::Arc<dyn Fn(Point) + Send + Sync>;

/// An open store with its transactions.
pub struct Engine {
    scratch: Scratch,
    folder: Option<PathBuf>,
    max_size: Option<u64>,
    /// The write turn, shared with the journal's writer, which wakes the waiting requests when it fails (T12).
    turns: Arc<Turns>,
    /// Next page to reserve, as of the newest commit.
    next_page: AtomicU64,
    /// Whether the meta page was ever committed.
    meta_written: AtomicBool,
    #[cfg(feature = "test-hooks")]
    hook: Mutex<Option<Hook>>,
}

struct Turns {
    queue: Mutex<Queue>,
    turn: Condvar,
}

impl Turns {
    /// The store refuses writes (T12): the requests waiting for the turn fail at once, and so do new ones.
    fn refuse_writes(&self) {
        self.queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .io_failed = true;
        self.turn.notify_all();
    }
}

struct Queue {
    /// Thread that holds the write transaction.
    owner: Option<ThreadId>,
    /// Tickets of the requests waiting for the turn, in order of arrival (T3).
    waiting: VecDeque<u64>,
    next_ticket: u64,
    /// The store refuses writes after a failed write (T12).
    io_failed: bool,
}

impl Engine {
    /// Opens the store in `folder` (flow of the Pager PRD) and reads the transaction layer's data.
    pub fn open(folder: impl AsRef<Path>, options: &OpenOptions) -> Result<Engine> {
        let folder = folder.as_ref();
        let pager = Pager::open(folder, options)?;
        Engine::new(pager, Some(folder.to_path_buf()))
    }

    /// Opens a store that lives only in memory (G9, T15).
    pub fn open_temporary(options: &OpenOptions) -> Result<Engine> {
        Engine::new(Pager::open_temporary(options)?, None)
    }

    fn new(pager: Pager, folder: Option<PathBuf>) -> Result<Engine> {
        let scratch = Scratch::open(pager)?;
        let pager = scratch.pager();
        let fresh = pager.page_count() <= META_PAGE || pager.page_is_blank(META_PAGE);
        let next_page = if fresh {
            FIRST_PAGE
        } else {
            let meta = pager.page(META_PAGE)?;
            let next = u64::from_le_bytes(meta[..8].try_into().expect("8 bytes"));
            if next < FIRST_PAGE {
                return Err(Error::Damaged {
                    folder,
                    page: Some(META_PAGE),
                });
            }
            next
        };
        let turns = Arc::new(Turns {
            queue: Mutex::new(Queue {
                owner: None,
                waiting: VecDeque::new(),
                next_ticket: 0,
                io_failed: false,
            }),
            turn: Condvar::new(),
        });
        // The journal's writer calls it when a group fails: the store already refuses writes, and the writer has
        // woken its own waiters (E8).
        let woken = Arc::clone(&turns);
        scratch.set_failure_hook(Some(Arc::new(move || woken.refuse_writes())));
        Ok(Engine {
            max_size: pager.info().max_size,
            scratch,
            folder,
            turns,
            next_page: AtomicU64::new(next_page),
            meta_written: AtomicBool::new(!fresh),
            #[cfg(feature = "test-hooks")]
            hook: Mutex::new(None),
        })
    }

    /// Information about the store (P12): `krummi.data` as it is (T14).
    pub fn info(&self) -> Info {
        self.scratch.info()
    }

    /// Opens a read transaction: a snapshot of everything committed so far (T2, T7).
    pub fn read(&self) -> Result<ReadTransaction<'_>> {
        Ok(ReadTransaction::new(self.scratch.snapshot()?))
    }

    /// Opens the write transaction, waiting for the turn in order of arrival (T3), at most `limit` when given
    /// (T4). Fails without waiting with the misuse class when this thread already has one open (T5) or the limit
    /// cannot be counted from now, and with the I/O class when the store refuses writes (T12), in that order
    /// (T13).
    pub fn write(&self, limit: Option<Duration>) -> Result<WriteTransaction<'_>> {
        let me = thread::current().id();
        let mut queue = self.lock_queue();
        if queue.owner == Some(me) {
            return Err(self.misuse("a write transaction is already open on this thread"));
        }
        let deadline = match limit {
            None => None,
            Some(limit) => match Instant::now().checked_add(limit) {
                Some(deadline) => Some(deadline),
                None => return Err(self.misuse("the wait limit is too large")),
            },
        };
        if queue.io_failed || !self.scratch.pager().accepts_writes() {
            return Err(self.writes_refused());
        }
        if queue.owner.is_some() || !queue.waiting.is_empty() {
            if limit == Some(Duration::ZERO) {
                return Err(self.timeout(limit));
            }
            queue = self.wait_turn(queue, deadline, limit)?;
        }
        queue.owner = Some(me);
        drop(queue);
        match self.scratch.begin_write() {
            Ok(writer) => Ok(WriteTransaction::new(self, writer)),
            Err(error) => {
                self.release_turn();
                Err(error)
            }
        }
    }

    /// Makes durable every commit that passed its point of no return, deferred ones and ones that have not returned
    /// included (J11 of the Journal PRD, E10 of the grouped journal PRD). It does not wait for the open write
    /// transaction. Fails at once with the I/O class when the store refuses writes (T12).
    pub fn make_durable(&self) -> Result<()> {
        let result = self.scratch.make_durable();
        if result.is_err() {
            self.note_failure();
        }
        result
    }

    /// Closes the store: transfers what was committed to `krummi.data`, removes the journal and releases its
    /// lock, which is released even if writing fails; then the journal stays for the next opening (J14, P14).
    /// Dropping an `Engine` does the same, without reporting a failure.
    pub fn close(self) -> Result<()> {
        self.scratch.close()
    }

    /// Waits until commit `txid` is durable (J4, E4); a failed group fails it with the I/O class and the waiting
    /// requests with it (E8, T12).
    fn wait_durable(&self, txid: u64) -> Result<()> {
        let result = self.scratch.wait_durable(txid);
        if result.is_err() {
            self.note_failure();
        }
        result
    }

    /// Transfers to `krummi.data` what no read needs; a failure makes the store refuse writes, which the next
    /// operations report (J5, J13).
    fn transfer(&self) {
        if self.scratch.transfer().is_err() {
            self.note_failure();
        }
    }

    /// Runs a transfer now, as after a commit, and returns whether the store still accepts writes (tests: AC-E17).
    #[cfg(feature = "test-hooks")]
    pub fn transfer_now(&self) -> bool {
        self.transfer();
        self.scratch.pager().accepts_writes()
    }

    /// Transfers everything no read needs and records it in both header copies (tests: AC-J18).
    #[cfg(feature = "test-hooks")]
    pub fn checkpoint(&self) -> Result<()> {
        self.scratch.checkpoint()
    }

    /// Bytes of committed page versions that are not in `krummi.data` yet (tests).
    #[cfg(feature = "test-hooks")]
    pub fn scratch_bytes(&self) -> u64 {
        self.scratch.scratch_bytes()
    }

    /// Sets the size at which the journal goes on in a new file (tests).
    #[cfg(feature = "test-hooks")]
    pub fn set_journal_file_limit(&self, bytes: u64) {
        self.scratch.set_journal_file_limit(bytes);
    }

    /// Sets a function a durable commit calls between writing its journal record and flushing it (tests: AC-J13).
    #[cfg(feature = "test-hooks")]
    pub fn set_journal_sync_hook(&self, hook: Option<std::sync::Arc<dyn Fn() + Send + Sync>>) {
        self.scratch.set_journal_sync_hook(hook);
    }

    /// Stops the journal's writer from taking queued commits, or lets it go on (tests).
    #[cfg(feature = "test-hooks")]
    pub fn set_journal_paused(&self, paused: bool) {
        self.scratch.set_journal_paused(paused);
    }

    /// Number of commits waiting in the journal's queue (tests).
    #[cfg(feature = "test-hooks")]
    pub fn journal_queued(&self) -> usize {
        self.scratch.journal_queued()
    }

    /// Writes of groups so far, one per journal file a group touches (tests).
    #[cfg(feature = "test-hooks")]
    pub fn journal_writes(&self) -> u64 {
        self.scratch.journal_writes()
    }

    /// Last durable commit (tests).
    #[cfg(feature = "test-hooks")]
    pub fn journal_durable(&self) -> u64 {
        self.scratch.journal_durable()
    }

    /// Sets the bytes the journal's queue holds before a commit waits for room (tests).
    #[cfg(feature = "test-hooks")]
    pub fn set_journal_queue_limit(&self, bytes: u64) {
        self.scratch.set_journal_queue_limit(bytes);
    }

    /// Sets a function the transfer calls while it holds its lock, or none (tests: AC-J17).
    #[cfg(feature = "test-hooks")]
    pub fn set_transfer_hook(&self, hook: Option<std::sync::Arc<dyn Fn() + Send + Sync>>) {
        self.scratch.set_transfer_hook(hook);
    }

    /// Sets the function called at each [`Point`], or none.
    #[cfg(feature = "test-hooks")]
    pub fn set_hook(&self, hook: Option<Hook>) {
        *self.hook.lock().unwrap_or_else(PoisonError::into_inner) = hook;
    }

    /// Number of requests waiting for the write turn, so tests can queue them in a known order.
    #[cfg(feature = "test-hooks")]
    pub fn queued(&self) -> usize {
        self.lock_queue().waiting.len()
    }

    fn hook(&self, point: Point) {
        #[cfg(feature = "test-hooks")]
        {
            let hook = self
                .hook
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            if let Some(hook) = hook {
                hook(point);
            }
        }
        #[cfg(not(feature = "test-hooks"))]
        let _ = point;
    }

    /// Queues a request and waits until it is first in the queue and the write transaction is free.
    fn wait_turn<'q>(
        &'q self,
        mut queue: MutexGuard<'q, Queue>,
        deadline: Option<Instant>,
        limit: Option<Duration>,
    ) -> Result<MutexGuard<'q, Queue>> {
        let ticket = queue.next_ticket;
        queue.next_ticket += 1;
        queue.waiting.push_back(ticket);
        loop {
            if queue.io_failed {
                leave(&mut queue, ticket);
                self.turns.turn.notify_all();
                return Err(self.writes_refused());
            }
            if queue.owner.is_none() && queue.waiting.front() == Some(&ticket) {
                queue.waiting.pop_front();
                return Ok(queue);
            }
            queue = match deadline {
                None => self
                    .turns
                    .turn
                    .wait(queue)
                    .unwrap_or_else(PoisonError::into_inner),
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        leave(&mut queue, ticket);
                        self.turns.turn.notify_all();
                        return Err(self.timeout(limit));
                    }
                    self.turns
                        .turn
                        .wait_timeout(queue, deadline - now)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
            };
        }
    }

    /// Ends the turn of the write transaction; the first request in the queue gets it (T3, T10).
    fn release_turn(&self) {
        let waiting = {
            let mut queue = self.lock_queue();
            queue.owner = None;
            !queue.waiting.is_empty()
        };
        if waiting {
            self.hook(Point::Handoff);
        }
        self.turns.turn.notify_all();
    }

    /// Records that the store refuses writes, if it does: the waiting requests fail at once (T12).
    fn note_failure(&self) {
        if !self.scratch.pager().accepts_writes() {
            self.turns.refuse_writes();
            self.scratch.wake();
        }
    }

    fn lock_queue(&self) -> MutexGuard<'_, Queue> {
        self.turns
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn misuse(&self, reason: &'static str) -> Error {
        Error::Misuse {
            folder: self.folder.clone(),
            reason,
        }
    }

    fn timeout(&self, limit: Option<Duration>) -> Error {
        Error::Timeout {
            folder: self.folder.clone(),
            limit: limit.unwrap_or_default(),
        }
    }

    fn writes_refused(&self) -> Error {
        Error::Io {
            folder: self.folder.clone().unwrap_or_default(),
            source: io::Error::other("writes are refused after a failed write; reopen the store"),
        }
    }
}

fn leave(queue: &mut Queue, ticket: u64) {
    queue.waiting.retain(|&t| t != ticket);
}
