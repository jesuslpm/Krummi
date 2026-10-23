//! The transfer thread (J5 of the Journal PRD): it owns every transfer of committed versions to `krummi.data`.
//!
//! A commit, the end of a read and a journal group becoming durable only raise a [`Signal`]: none of them does
//! I/O. The signal keeps one deadline, when the next pass is due: no sooner than a minimum interval after the
//! previous pass, so the signals raised meanwhile collapse into one pass and a page that many commits rewrite is
//! written once per pass, not once per commit; at once when the scratch area grew by a threshold since the last
//! pass. A pass that finds journal files to release but flushed `krummi.data` too recently, or that failed without
//! making the store refuse writes, sets the deadline to try again.
//!
//! No other thread transfers while the store is open: a writer that needs room (T14) and the tests that run a
//! transfer or a checkpoint send a [`Demand`] and wait for the thread to serve it. Only the close and the drop of
//! the store, once the thread has ended, write versions in place themselves.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::scratch::Shared;
use crate::sys;

/// Minimum time between the starts of two passes.
const PASS_INTERVAL: Duration = Duration::from_millis(200);
/// Growth of the scratch area since the last pass from which a signal starts a pass without waiting for the
/// interval.
const PASS_BYTES: u64 = 32 << 20;
/// Minimum time between two flushes of `krummi.data` that release journal files.
const DATA_SYNC_INTERVAL: Duration = Duration::from_secs(2);

/// When the transfer thread runs its passes and flushes `krummi.data`; tests change it.
#[derive(Clone, Copy)]
pub(crate) struct Pacing {
    pub(crate) interval: Duration,
    pub(crate) bytes: u64,
    pub(crate) data_sync_interval: Duration,
}

impl Default for Pacing {
    fn default() -> Pacing {
        Pacing {
            interval: PASS_INTERVAL,
            bytes: PASS_BYTES,
            data_sync_interval: DATA_SYNC_INTERVAL,
        }
    }
}

/// Work that a caller waits for the transfer thread to do.
#[derive(Clone, Copy)]
pub(crate) enum Demand {
    /// A pass like the thread's own, which releases journal files however recently `krummi.data` was flushed
    /// (tests).
    Pass,
    /// Every version no read needs, after making every queued commit durable (a writer that needs room, T14).
    Flush,
    /// A flush, then a new journal file and both header copies recording it (tests: AC-J18).
    Checkpoint,
}

/// Wakes the transfer thread. Raising it takes a lock and may notify a condvar; it never does I/O.
pub(crate) struct Signal {
    state: Mutex<SignalState>,
    /// Wakes the transfer thread.
    wake: Condvar,
    /// Wakes the callers waiting for a demand.
    answered: Condvar,
    /// Whether the transfer thread is running, changed under the lock (tests too).
    alive: Arc<AtomicBool>,
}

struct SignalState {
    /// When the next pass is due, once one is asked for.
    due: Option<Instant>,
    /// When the last pass started.
    last_pass: Option<Instant>,
    /// Bytes the scratch area kept after the last work of the thread.
    kept: u64,
    /// The store closes: the thread ends.
    stop: bool,
    /// No pass starts while set, but demands are served (tests).
    paused: bool,
    pacing: Pacing,
    /// Demands waiting for the thread, with their tickets.
    demands: VecDeque<(u64, Demand)>,
    next_ticket: u64,
    /// Outcomes of served demands, until their callers take them.
    answers: Vec<(u64, Result<()>)>,
}

impl SignalState {
    /// The earliest a pass may start: the pass interval after the last one.
    fn ready(&self) -> Instant {
        self.last_pass
            .map_or_else(Instant::now, |last| last + self.pacing.interval)
    }

    /// Moves the deadline to `at` if that is sooner; returns whether it moved.
    fn ask(&mut self, at: Instant) -> bool {
        if self.due.is_some_and(|due| due <= at) {
            return false;
        }
        self.due = Some(at);
        true
    }
}

/// What the transfer thread does next.
enum Work {
    Demand(u64, Demand, Pacing),
    Pass(Pacing),
    Stop,
}

impl Signal {
    pub(crate) fn new() -> Signal {
        Signal {
            state: Mutex::new(SignalState {
                due: None,
                last_pass: None,
                kept: 0,
                stop: false,
                paused: false,
                pacing: Pacing::default(),
                demands: VecDeque::new(),
                next_ticket: 0,
                answers: Vec::new(),
            }),
            wake: Condvar::new(),
            answered: Condvar::new(),
            alive: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Asks for a pass, with `bytes` in the scratch area: once the interval since the last pass is over, or at
    /// once if the area grew by the pacing's bytes since the last pass. A pass already due as soon absorbs it.
    pub(crate) fn raise(&self, bytes: u64) {
        let mut state = self.lock();
        let at = if bytes >= state.kept.saturating_add(state.pacing.bytes) {
            Instant::now()
        } else {
            state.ready()
        };
        if state.ask(at) {
            drop(state);
            self.wake.notify_one();
        }
    }

    /// Asks the transfer thread to do `demand` and waits for its outcome; `None` when there is no thread to do it,
    /// so the caller, being the only one, does it.
    pub(crate) fn demand(&self, demand: Demand) -> Option<Result<()>> {
        let mut state = self.lock();
        if !self.alive.load(Ordering::Acquire) {
            return None;
        }
        let ticket = state.next_ticket;
        state.next_ticket += 1;
        state.demands.push_back((ticket, demand));
        self.wake.notify_one();
        loop {
            if let Some(index) = state.answers.iter().position(|(t, _)| *t == ticket) {
                return Some(state.answers.swap_remove(index).1);
            }
            // A thread that ended without serving it (a panic) left nothing half done that the caller cannot redo.
            if !self.alive.load(Ordering::Acquire) {
                return None;
            }
            state = self
                .answered
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn answer(&self, ticket: u64, outcome: Result<()>) {
        self.lock().answers.push((ticket, outcome));
        self.answered.notify_all();
    }

    /// Ends the transfer thread once its current pass ends.
    pub(crate) fn stop(&self) {
        self.lock().stop = true;
        self.wake.notify_one();
    }

    /// Stops the thread from starting passes, or lets it go on (tests).
    pub(crate) fn set_paused(&self, paused: bool) {
        self.lock().paused = paused;
        self.wake.notify_one();
    }

    pub(crate) fn set_pacing(&self, pacing: Pacing) {
        self.lock().pacing = pacing;
        self.wake.notify_one();
    }

    /// Whether the transfer thread is running (tests).
    pub(crate) fn alive(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.alive)
    }

    /// Records what the thread's last work left: `kept` bytes in the scratch area, and a pass to try again at
    /// `retry_at`, no sooner than the pass interval. Only the thread calls it, so nobody needs waking.
    fn after_work(&self, kept: u64, retry_at: Option<Instant>) {
        let mut state = self.lock();
        state.kept = kept;
        if let Some(at) = retry_at {
            let at = at.max(state.ready());
            state.ask(at);
        }
    }

    /// Waits for the next work: a demand, served first; or a pass, once it is due. Paused or `failed`, it only
    /// serves demands.
    fn next_work(&self, failed: bool) -> Work {
        let mut state = self.lock();
        loop {
            if state.stop {
                return Work::Stop;
            }
            if let Some((ticket, demand)) = state.demands.pop_front() {
                return Work::Demand(ticket, demand, state.pacing);
            }
            let now = Instant::now();
            state = match state.due.filter(|_| !state.paused && !failed) {
                Some(due) if now >= due => break,
                Some(due) => {
                    self.wake
                        .wait_timeout(state, due - now)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
                None => self
                    .wake
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner),
            };
        }
        // Signals raised from here on ask for the next pass: this one may have built its list before them.
        state.due = None;
        state.last_pass = Some(Instant::now());
        Work::Pass(state.pacing)
    }

    fn lock(&self) -> MutexGuard<'_, SignalState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Starts the transfer thread, which waits for the store to be sent and ends without it if the sender is dropped
/// first (an opening that failed). Starting it before recovery means failing to start it leaves the store
/// untouched, as for the journal's writer (E13 of the grouped journal PRD): it is the out-of-memory class.
pub(crate) fn start(signal: &Arc<Signal>) -> Result<(JoinHandle<()>, Sender<Arc<Shared>>)> {
    let out_of_memory = || Error::OutOfMemory { requested: 0 };
    sys::thread_point().map_err(|_| out_of_memory())?;
    let (sender, receiver) = mpsc::channel::<Arc<Shared>>();
    #[cfg(any(test, feature = "test-hooks"))]
    let context = sys::faults::context();
    let signal = Arc::clone(signal);
    signal.alive.store(true, Ordering::Release);
    let alive = Arc::clone(&signal.alive);
    let handle = std::thread::Builder::new()
        .name("krummi-transfer".into())
        .spawn(move || {
            #[cfg(any(test, feature = "test-hooks"))]
            sys::faults::adopt(&context);
            let _alive = Alive(Arc::clone(&signal));
            if let Ok(shared) = receiver.recv() {
                run(&signal, &shared);
            }
        })
        .map_err(|_| {
            // Nobody can wait for a demand yet.
            alive.store(false, Ordering::Release);
            out_of_memory()
        })?;
    Ok((handle, sender))
}

/// Clears the flag when the thread ends, even by a panic, and wakes the callers waiting for a demand.
struct Alive(Arc<Signal>);

impl Drop for Alive {
    fn drop(&mut self) {
        let state = self.0.lock();
        self.0.alive.store(false, Ordering::Release);
        drop(state);
        self.0.answered.notify_all();
    }
}

/// Serves demands and runs passes until the store closes. After a failure that makes the store refuse writes (J13)
/// the thread only serves demands, which fail as well: the close writes what is left.
fn run(signal: &Signal, shared: &Shared) {
    let mut failed = false;
    loop {
        let outcome = match signal.next_work(failed) {
            Work::Stop => return,
            Work::Demand(ticket, demand, pacing) => {
                let outcome = shared.serve(demand, pacing.data_sync_interval);
                let kept = match &outcome {
                    Ok(next) => Ok(*next),
                    Err(error) => Err(error.duplicate()),
                };
                signal.answer(ticket, outcome.map(|_| ()));
                kept
            }
            Work::Pass(pacing) => shared.pass(pacing.data_sync_interval),
        };
        let retry_at = match outcome {
            // A release put off by the data flush interval.
            Ok(next) => next,
            // Only a failure that made the store refuse writes ends the passes (J13).
            Err(_) if shared.refuses_writes() => {
                if !failed {
                    failed = true;
                    shared.report_failure();
                }
                None
            }
            // After another one (a full disk, T12) the pass is tried again once the pass interval is over, so
            // what it left reaches `krummi.data` without another commit, and a failing pass never spins.
            Err(_) => Some(Instant::now()),
        };
        signal.after_work(shared.scratch_bytes(), retry_at);
    }
}
