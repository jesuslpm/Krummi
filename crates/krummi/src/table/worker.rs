//! The background work on statistics (ST6, ST9, ST10, ST14 of the statistics PRD): one thread per store, started by
//! the first request and joined when the store closes, that saves the statistics estimates sampled and renews stale
//! ones. A request is of a table by its identity; at most one save and one renewal wait per table, in order of
//! arrival. Its failures are dropped: the statistics in memory stay, and the next estimate asks again.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::catalog::ReadTable;
use super::statistics::Built;
use crate::{ErrorKind, ReadTransaction, Shared, TreeKind};

/// How long a save waits for the write turn before it looks whether the store is closing (ST10).
const TURN_POLL: Duration = Duration::from_millis(20);

/// A request to the background thread.
pub(crate) enum Job {
    /// Save statistics an estimate sampled.
    Save(Built),
    /// Renew the stale statistics of the table of this identity, last named so.
    Renew { table: u64, name: String },
}

impl Job {
    fn table(&self) -> u64 {
        match self {
            Job::Save(built) => built.table,
            Job::Renew { table, .. } => *table,
        }
    }
}

/// A point of the background work that a test can hold (ST14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[doc(hidden)]
pub enum StatisticsPoint {
    /// An estimate's sample, read and not yet published.
    EstimateSampled,
    /// A sample of the background thread or of analyze on the store, read and not yet published.
    WorkSampled,
    /// A save that has the write turn and has not committed.
    SaveTurn,
}

/// The work of a store.
#[derive(Default)]
pub(crate) struct Work {
    state: Mutex<State>,
    wake: Condvar,
    thread: Mutex<Option<JoinHandle<()>>>,
    stop: AtomicBool,
    held: Mutex<Vec<StatisticsPoint>>,
    released: Condvar,
}

#[derive(Default)]
struct State {
    queue: VecDeque<Job>,
    busy: bool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Work {
    /// Queues `job` and starts the thread if it is not running, unless the store is closing or refuses writes (T12).
    pub(crate) fn request(&self, shared: &Shared, job: Job) {
        if self.stop.load(Ordering::Acquire) || !shared.engine.accepts_writes() {
            return;
        }
        {
            let mut state = lock(&self.state);
            let table = job.table();
            let same = state.queue.iter_mut().find(|queued| {
                queued.table() == table
                    && matches!(queued, Job::Save(_)) == matches!(job, Job::Save(_))
            });
            match (same, job) {
                (Some(Job::Save(queued)), Job::Save(built)) => {
                    for (column, stats) in built.columns {
                        match queued.columns.iter_mut().find(|(c, _)| *c == column) {
                            Some((_, kept)) if kept.counter <= stats.counter => *kept = stats,
                            Some(_) => {}
                            None => queued.columns.push((column, stats)),
                        }
                    }
                }
                (Some(_), _) => {}
                (None, job) => state.queue.push_back(job),
            }
        }
        self.wake.notify_all();
        let mut thread = lock(&self.thread);
        if thread.is_none()
            && let Some(shared) = shared.this.upgrade()
        {
            let spawned = thread::Builder::new()
                .name("krummi-statistics".into())
                .spawn(move || run(&shared));
            // A thread the system does not give leaves the work undone; the next estimate asks again.
            *thread = spawned.ok();
        }
    }

    /// Stops the thread and waits for it: a sample in progress is dropped, the queue emptied, and a save waiting for
    /// its turn dropped; a save that has its turn commits (ST10).
    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        lock(&self.state).queue.clear();
        self.wake.notify_all();
        self.released.notify_all();
        let handle = lock(&self.thread).take();
        if let Some(handle) = handle {
            // A panic of the thread has already dropped its work.
            let _ = handle.join();
        }
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    /// Waits while a test holds `point`, or until the store closes.
    pub(crate) fn pass(&self, point: StatisticsPoint) {
        let mut held = lock(&self.held);
        while held.contains(&point) && !self.stopping() {
            held = self
                .released
                .wait(held)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// A test hook: holds `point` or releases it.
    pub(crate) fn hold(&self, point: StatisticsPoint, held: bool) {
        let mut points = lock(&self.held);
        points.retain(|p| *p != point);
        if held {
            points.push(point);
        }
        drop(points);
        self.released.notify_all();
    }

    /// A test hook: whether the thread is running, and the saves and renewals queued, not counting the job it runs.
    pub(crate) fn status(&self) -> (bool, usize, usize) {
        let alive = lock(&self.thread)
            .as_ref()
            .is_some_and(|handle| !handle.is_finished());
        let state = lock(&self.state);
        let saves = state
            .queue
            .iter()
            .filter(|j| matches!(j, Job::Save(_)))
            .count();
        (alive, saves, state.queue.len() - saves)
    }

    /// A test hook: waits until the thread has nothing to do.
    pub(crate) fn wait_idle(&self) {
        let mut state = lock(&self.state);
        while (state.busy || !state.queue.is_empty()) && !self.stopping() {
            state = self
                .wake
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// The thread: runs the queued jobs until the store closes.
fn run(shared: &Shared) {
    let work = &shared.work;
    loop {
        let job = {
            let mut state = lock(&work.state);
            state.busy = false;
            work.wake.notify_all();
            loop {
                if work.stopping() {
                    return;
                }
                if let Some(job) = state.queue.pop_front() {
                    state.busy = true;
                    break job;
                }
                state = work
                    .wake
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        // Failures drop the job (ST9).
        let _ = match job {
            Job::Save(built) => save(shared, built),
            Job::Renew { table, name } => renew(shared, table, &name),
        };
    }
}

/// The table of identity `id` that `read` sees, by its last known name `name` or, once renamed, among the tables.
fn find_table<'r>(
    read: &'r ReadTransaction<'_>,
    id: u64,
    name: &str,
) -> crate::Result<Option<ReadTable<'r>>> {
    match read.open_table(name) {
        Ok(table) if table.id == id => return Ok(Some(table)),
        Ok(_) => {}
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::TreeNotFound | ErrorKind::DifferentTreeType
            ) => {}
        Err(error) => return Err(error),
    }
    for (other, kind) in read.list_trees()? {
        if kind == TreeKind::Table && other != name {
            let table = read.open_table(&other)?;
            if table.id == id {
                return Ok(Some(table));
            }
        }
    }
    Ok(None)
}

/// Renews the statistics of the table `id` from a new snapshot: publishes them in memory and saves them.
fn renew(shared: &Shared, id: u64, name: &str) -> crate::Result<()> {
    let built = {
        let read = shared.read()?;
        let Some(table) = find_table(&read, id, name)? else {
            return Ok(());
        };
        read.build_statistics(&table, None)?
    };
    shared.work.pass(StatisticsPoint::WorkSampled);
    if shared.work.stopping() {
        return Ok(());
    }
    shared.statistics.publish(&built);
    save(shared, built)
}

/// Saves `built` with a short write of its own, which waits for its turn like any write (ST6), and gives it up when
/// the store closes before it gets it.
pub(crate) fn save(shared: &Shared, built: Built) -> crate::Result<()> {
    let mut write = loop {
        if shared.work.stopping() {
            return Ok(());
        }
        match shared.open_write(Some(TURN_POLL)) {
            Ok(write) => break write,
            Err(error) if error.kind() == ErrorKind::Timeout => {}
            Err(error) => return Err(error),
        }
    };
    let mut found = None;
    for name in std::iter::once(built.name.clone()).chain(
        write
            .list_trees()?
            .into_iter()
            .filter_map(|(name, kind)| (kind == TreeKind::Table).then_some(name)),
    ) {
        match write.open_table(&name) {
            Ok(table) if table.id == built.table => {
                found = Some(table);
                break;
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::TreeNotFound | ErrorKind::DifferentTreeType
                ) => {}
            Err(error) => return Err(error),
        }
    }
    if let Some(table) = found {
        write.save_statistics(&table, built)?;
    }
    shared.work.pass(StatisticsPoint::SaveTurn);
    write.commit()
}
