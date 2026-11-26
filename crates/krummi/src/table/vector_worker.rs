//! The worker of the vector indexes (VI3, VI4, VI9, VI12 and VI13 of the vector indexes PRD; document 12 of the table
//! design, *The worker*, *Reconcile*): one thread of the store that trains, builds, reconciles and rebuilds them in
//! short batches of its own writes, which take the write turn as any writer, in their order of arrival (G4).
//!
//! A batch is computed in a short read and applied in a write: the read takes up to a batch of pending keys and, while
//! a generation is building, the next keys of the table after the build's cursor, and computes each row's list and code
//! outside the turn; the write checks that the table, the index, its instance and its state are those of the read
//! (otherwise it drops the batch, writing nothing), reads each row again and recomputes it only when its vector
//! changed, applies them in order until a time budget inside the turn, and commits without waiting for the disk. A
//! crash may lose the last batches, always together with every later commit, and their work is done again when the store
//! opens: the pending keys and the cursor are still there.
//!
//! The thread starts when the store opens with a vector index, and with the first commit that creates one or changes
//! rows of a table with one; it ends when the store closes, abandoning a training in progress and waiting for the batch
//! that holds the turn. A batch that fails (damage, input and output, memory, a full disk or the maximum size) changes
//! nothing and is retried at the next pass, without telling anyone; a failure that makes the store refuse writes ends
//! the thread (T12). A batch commits, then waits outside the turn until it is durable; a failure of that wait is a
//! failed flush, which also makes the store refuse writes and ends the thread.

use std::collections::HashMap;
use std::ops::Bound;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use krummi_pager::{ErrorKind, Result, TreeKind};
use krummi_tree::{Interval, PageSource};

use super::catalog::{ReadTable, WriteTable};
use super::distance::work_vector;
use super::ivf::{Generation, MIN_TRAINING_ROWS, Rng, SAMPLE_PER_LIST, lists_for};
use super::rows::{At, Reader, Versions};
use super::value::{ColumnType, Value, VectorType};
use super::vector_index::{IndexState, Indicators, VectorRoots, read_state, tag};
use crate::{ReadTransaction, Shared, WriteTransaction};

/// The longest a batch applies rows inside the write turn, so that with its commit it holds the turn at most 15 ms
/// (VI3): the commit of a batch has taken up to twice its apply.
const APPLY_BUDGET: Duration = Duration::from_millis(5);
/// The turn a batch aims at, its commit included; the next batch's rows follow from the last one's pace.
const TURN_TARGET: Duration = Duration::from_millis(5);
/// The rows a sampling read takes at a time, so that no snapshot stays open long (document 12).
const SAMPLE_CHUNK: usize = 2048;
/// The keys of a retired generation one batch deletes.
const CLEANUP_BATCH: usize = 4096;
/// The most bytes of trained generations the store keeps in memory for its searches and its worker.
const CACHE_BYTES: usize = 256 << 20;
/// The first batch's rows; later batches adapt to the time they took.
const FIRST_BATCH: usize = 128;
/// The default thresholds of an automatic rebuild (VI9, *Dejado a la implementación*): the fraction of saturated
/// codes, the largest list against the mean, and the rows against the rows at training (above it, or below its
/// inverse).
const REBUILD_SATURATED: f64 = 0.10;
const REBUILD_LARGEST: f64 = 12.0;
const REBUILD_ROWS: f64 = 2.0;
const REBUILD_GROWTH: f64 = 1.5;

/// A point of the worker that a test can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[doc(hidden)]
pub enum VectorPoint {
    /// A training that sampled the table and has not trained yet.
    Sampled,
    /// A batch of pending or build keys that has the write turn and has not committed.
    BatchTurn,
}

/// The thresholds of an automatic rebuild (VI9).
#[derive(Debug, Clone, Copy, PartialEq)]
#[doc(hidden)]
pub struct RebuildThresholds {
    pub saturated: f64,
    pub largest_list: f64,
    pub rows: f64,
    /// How much an indicator must grow over the generation's own when it was published: a rebuild that cannot lower
    /// an indicator below its threshold (clustered data, say) does not repeat.
    pub growth: f64,
}

impl Default for RebuildThresholds {
    fn default() -> Self {
        RebuildThresholds {
            saturated: REBUILD_SATURATED,
            largest_list: REBUILD_LARGEST,
            rows: REBUILD_ROWS,
            growth: REBUILD_GROWTH,
        }
    }
}

/// The vector work of a store.
#[derive(Default)]
pub(crate) struct VectorWork {
    state: Mutex<WorkState>,
    wake: Condvar,
    thread: Mutex<Option<JoinHandle<()>>>,
    stop: AtomicBool,
    /// A test hook: while set, the worker takes no batch.
    paused: AtomicBool,
    hooks: Mutex<Hooks>,
    held: Mutex<Vec<VectorPoint>>,
    released: Condvar,
    /// Where the thread waits after a failed pass, woken only by the close.
    backoff: Condvar,
    /// The held point where the thread waits, for the tests.
    waiting: Mutex<Option<VectorPoint>>,
    /// The trained generations read so far, by index, instance and generation (immutable once written).
    cache: Mutex<HashMap<(u64, u64, u32), Arc<Generation>>>,
}

#[derive(Default)]
struct WorkState {
    /// Whether a commit left work since the thread last looked.
    kicked: bool,
    busy: bool,
    /// The rows of the next batch.
    batch: usize,
    /// The table counter at which an index found too few rows to train, by index and instance: it waits for a change.
    untrainable: HashMap<(u64, u64), u64>,
}

/// The test hooks of the worker.
#[derive(Default)]
struct Hooks {
    /// The longest a batch held the write turn since the last reset.
    longest_turn: Duration,
    batches: u64,
    /// The rows the training counts, instead of those it read (AC-VI15).
    training_rows: Option<u64>,
    thresholds: Option<RebuildThresholds>,
    search: SearchHooks,
}

/// The test hooks of the searches: the threshold of rows that pass instead of the default, and a rerank of every
/// candidate of the probed lists (AC-VI5, AC-VI6).
#[derive(Default, Clone, Copy)]
pub(crate) struct SearchHooks {
    pub(crate) threshold: Option<u64>,
    pub(crate) rerank_all: bool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl VectorWork {
    /// Tells the worker that a commit may have left work, starting its thread if it is not running, unless the store
    /// is closing or refuses writes (T12).
    pub(crate) fn kick(&self, shared: &Shared) {
        if self.stopping() || !shared.engine.accepts_writes() {
            return;
        }
        lock(&self.state).kicked = true;
        self.wake.notify_all();
        let mut thread = lock(&self.thread);
        // `stop` sets its flag before it takes the handle under this lock: a kick that comes after sees it, and never
        // starts a thread that the close would not join.
        if self.stopping() {
            return;
        }
        if thread.as_ref().is_none_or(JoinHandle::is_finished)
            && let Some(shared) = shared.this.upgrade()
        {
            #[cfg(feature = "test-hooks")]
            let faults = krummi_pager::testing::context();
            let spawned = thread::Builder::new()
                .name("krummi-vectors".into())
                .spawn(move || {
                    #[cfg(feature = "test-hooks")]
                    krummi_pager::testing::adopt(&faults);
                    run(&shared)
                });
            *thread = spawned.ok();
        }
    }

    /// Starts the thread when the store opens (VI12): it reads the catalog in the background, and a vector index left
    /// with work when the store closed goes on.
    pub(crate) fn resume(&self, shared: &Shared) {
        self.kick(shared);
    }

    /// Stops the thread and waits for it: a training in progress is abandoned, a batch waiting for its turn dropped,
    /// and a batch that has its turn commits (VI12).
    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        // Taking each mutex after the flag is set orders this notification after any waiter's check of the flag: a
        // waiter either sees the flag or is already waiting when the notification comes, so none is lost.
        drop(lock(&self.state));
        self.wake.notify_all();
        drop(lock(&self.held));
        self.released.notify_all();
        self.backoff.notify_all();
        let handle = lock(&self.thread).take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    pub(crate) fn pause(&self, paused: bool) {
        self.paused.store(paused, Ordering::Release);
        if !paused {
            lock(&self.state).kicked = true;
        }
        self.wake.notify_all();
    }

    fn paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// Waits while a test holds `point`, or until the store closes.
    fn pass(&self, point: VectorPoint) {
        let mut held = lock(&self.held);
        while held.contains(&point) && !self.stopping() {
            *lock(&self.waiting) = Some(point);
            self.released.notify_all();
            held = self
                .released
                .wait(held)
                .unwrap_or_else(PoisonError::into_inner);
        }
        *lock(&self.waiting) = None;
    }

    /// The generation `generation` of the index `index` of instance `instance`, from the cache or read with `read`.
    pub(crate) fn generation(
        &self,
        key: (u64, u64, u32),
        read: impl FnOnce() -> Result<Generation>,
    ) -> Result<Arc<Generation>> {
        if let Some(found) = lock(&self.cache).get(&key) {
            return Ok(Arc::clone(found));
        }
        let generation = Arc::new(read()?);
        let mut cache = lock(&self.cache);
        // At most [`CACHE_BYTES`] of centroids and quantizers: past it, only the generations read from then on.
        let size = |g: &Generation| 4 * (g.lo.len() + g.step.len() + g.centroids.len());
        let held: usize = cache.values().map(|g| size(g)).sum();
        if held + size(&generation) > CACHE_BYTES {
            cache.clear();
        }
        cache.insert(key, Arc::clone(&generation));
        Ok(generation)
    }

    fn thresholds(&self) -> RebuildThresholds {
        lock(&self.hooks).thresholds.unwrap_or_default()
    }

    pub(crate) fn search_hooks(&self) -> SearchHooks {
        lock(&self.hooks).search
    }

    fn training_rows(&self) -> Option<u64> {
        lock(&self.hooks).training_rows
    }

    fn record_turn(&self, held: Duration) {
        let mut hooks = lock(&self.hooks);
        hooks.batches += 1;
        hooks.longest_turn = hooks.longest_turn.max(held);
    }
}

/// The thread: waits for work, then takes batches until there is none, until the store closes.
fn run(shared: &Shared) {
    let work = &shared.vectors;
    loop {
        {
            let mut state = lock(&work.state);
            state.busy = false;
            work.wake.notify_all();
            loop {
                if work.stopping() || !shared.engine.accepts_writes() {
                    return;
                }
                if state.kicked && !work.paused() {
                    state.kicked = false;
                    state.busy = true;
                    break;
                }
                state = work
                    .wake
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        }
        let mut failures = 0u32;
        while !work.stopping() && !work.paused() && shared.engine.accepts_writes() {
            match pass(shared) {
                Ok(true) => failures = 0,
                Ok(false) => break,
                // A failed batch changes nothing; it is retried, after a pause that grows with the failures (VI13).
                // The pause is its own: commits do not cut it short, and it ends only with the close.
                Err(_) => {
                    failures += 1;
                    let pause = Duration::from_millis(10u64 << failures.min(7));
                    let held = lock(&work.held);
                    if !work.stopping() {
                        let _ = work.backoff.wait_timeout(held, pause);
                    }
                }
            }
        }
    }
}

/// A vector index as the worker finds it in a snapshot.
#[derive(Clone)]
struct Target {
    table: u64,
    name: String,
    index: u64,
    at: usize,
    roots: VectorRoots,
    vector: VectorType,
    /// The identity of the indexed column: its place among the live columns is taken from each snapshot's schema,
    /// since dropping another column moves it.
    column: u64,
    rows: u64,
    counter: u64,
}

/// The place among the live columns of `schema` of the column of identity `column`.
fn place_of(schema: &super::schema::TableSchema, column: u64) -> usize {
    schema
        .column_position(column)
        .expect("a live column of the index")
}

/// The vector indexes of the tables `read` sees.
fn targets(read: &ReadTransaction<'_>) -> Result<Vec<Target>> {
    let mut found = Vec::new();
    for (name, kind) in read.list_trees()? {
        if kind != TreeKind::Table {
            continue;
        }
        let table = read.open_table(&name)?;
        for (at, index) in table.schema.vector_indexes().iter().enumerate() {
            let column = table
                .schema
                .column_by_id(index.column.id)
                .expect("a live column of the index");
            let ColumnType::Vector(vector) = column.ty else {
                unreachable!("an index over a vector column")
            };
            found.push(Target {
                table: table.id,
                name: name.clone(),
                index: index.id,
                at,
                roots: table.vector_roots[at],
                vector,
                column: index.column.id,
                rows: read.row_count(&table)?,
                counter: table.stats.counter,
            });
        }
    }
    Ok(found)
}

/// One pass over every vector index: one step of work on each that has some. Whether any did.
fn pass(shared: &Shared) -> Result<bool> {
    let targets = targets(&shared.read()?)?;
    let mut worked = false;
    let mut failed = None;
    for target in &targets {
        if shared.vectors.stopping() {
            return Ok(worked);
        }
        // A failing index is retried at the next pass; it never holds back the others (VI13).
        match step(shared, target) {
            Ok(did) => worked |= did,
            Err(error) => failed = failed.or(Some(error)),
        }
    }
    match failed {
        Some(error) if !worked => Err(error),
        _ => Ok(worked),
    }
}

/// One step of work on `target`: training, a batch of pending or build keys, a batch of cleanup, or the start of an
/// automatic rebuild. Whether it did something.
fn step(shared: &Shared, target: &Target) -> Result<bool> {
    // The index's roots, rows and state as one snapshot sees them: never roots of an older one.
    let (state, target) = {
        let read = shared.read()?;
        let Some(table) = find(&read, target)? else {
            return Ok(false);
        };
        let roots = table.vector_roots[target.at];
        let fresh = Target {
            roots,
            rows: read.row_count(&table)?,
            counter: table.stats.counter,
            ..target.clone()
        };
        (read_state(&read.inner, &roots)?, fresh)
    };
    let target = &target;
    // Pending rows first while a generation serves the searches, so that a long training lets them wait less.
    if state.active != 0 && target.roots.pending_count > 0 && state.building == 0 {
        return batch(shared, target, &state);
    }
    let training = state.building == 0 && (state.active == 0 || state.rebuilding);
    if training && train(shared, target, &state)? {
        return Ok(true);
    }
    if target.roots.pending_count > 0 || state.building != 0 {
        return batch(shared, target, &state);
    }
    if cleanup(shared, target)? {
        return Ok(true);
    }
    if state.active != 0 && !state.rebuilding {
        return start_rebuild_if_due(shared, target, &state);
    }
    Ok(false)
}

// ---- Keys of the data tree ------------------------------------------------------------------------------------

pub(crate) fn generation_key(tag: u8, generation: u32) -> Vec<u8> {
    let mut key = vec![tag];
    key.extend(generation.to_be_bytes());
    key
}

pub(crate) fn list_key(tag: u8, generation: u32, list: u32) -> Vec<u8> {
    let mut key = generation_key(tag, generation);
    key.extend(list.to_be_bytes());
    key
}

pub(crate) fn posting_key(generation: u32, list: u32, primary: &[u8]) -> Vec<u8> {
    let mut key = list_key(tag::POSTING, generation, list);
    key.extend(primary);
    key
}

pub(crate) fn locator_key(generation: u32, primary: &[u8]) -> Vec<u8> {
    let mut key = generation_key(tag::LOCATOR, generation);
    key.extend(primary);
    key
}

/// The longest primary key the keys of an entry hold whole: the tree's maximum less the tag, the generation and the
/// list. A longer one, which a table admits up to the tree's maximum, gets a number of its own (`tag::LONG_KEY`).
pub(crate) const INLINE_KEY: usize = krummi_tree::MAX_KEY - 9;

/// How the keys of a data tree name a row: by its primary key, or by the number a longer primary key gets.
#[derive(Clone, Copy)]
pub(crate) enum RowRef<'k> {
    Key(&'k [u8]),
    Long(u64),
}

impl RowRef<'_> {
    pub(crate) fn locator(&self, generation: u32) -> Vec<u8> {
        match self {
            RowRef::Key(primary) => locator_key(generation, primary),
            RowRef::Long(number) => {
                let mut key = generation_key(tag::LONG_LOCATOR, generation);
                key.extend(number.to_be_bytes());
                key
            }
        }
    }

    pub(crate) fn posting(&self, generation: u32, list: u32) -> Vec<u8> {
        match self {
            RowRef::Key(primary) => posting_key(generation, list, primary),
            RowRef::Long(number) => {
                let mut key = list_key(tag::LONG_POSTING, generation, list);
                key.extend(number.to_be_bytes());
                key
            }
        }
    }
}

/// The key of the bucket of the long primary keys that begin as `primary`.
fn bucket_key(primary: &[u8]) -> Vec<u8> {
    let mut key = vec![tag::LONG_BUCKET];
    key.extend(&primary[..primary.len().min(krummi_tree::MAX_KEY - 1)]);
    key
}

/// The number of `primary` in a bucket's bytes: entries of a number (8 bytes), a length (4) and a key.
fn in_bucket(bytes: &[u8], primary: &[u8]) -> Option<u64> {
    let mut rest = bytes;
    while rest.len() >= 12 {
        let number = u64::from_be_bytes(rest[..8].try_into().ok()?);
        let len = u32::from_be_bytes(rest[8..12].try_into().ok()?) as usize;
        let key = rest.get(12..12 + len)?;
        if key == primary {
            return Some(number);
        }
        rest = &rest[12 + len..];
    }
    None
}

/// How the data tree at `data` names the row of `primary`; none for a long primary key that never got a number.
pub(crate) fn row_ref<'k, S: PageSource + ?Sized>(
    tx: &S,
    data: u64,
    primary: &'k [u8],
) -> Result<Option<RowRef<'k>>> {
    if primary.len() <= INLINE_KEY {
        return Ok(Some(RowRef::Key(primary)));
    }
    let bucket = krummi_tree::get(tx, data, &bucket_key(primary))?;
    Ok(bucket
        .and_then(|bytes| in_bucket(bytes, primary))
        .map(RowRef::Long))
}

/// The primary key of the long row `number` in the data tree at `data`; a missing one is damage of its page.
pub(crate) fn long_primary<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    data: u64,
    number: &[u8],
) -> Result<&'t [u8]> {
    let mut key = vec![tag::LONG_KEY];
    key.extend(number);
    krummi_tree::get(tx, data, &key)?.ok_or_else(|| match krummi_tree::leaf_of(tx, data, &key) {
        Ok(page) => tx.damaged(page),
        Err(error) => error,
    })
}

/// The counters of a generation: its entries and its codes with a saturated component.
pub(crate) fn read_counters<S: PageSource + ?Sized>(
    tx: &S,
    data: u64,
    generation: u32,
) -> Result<(u64, u64)> {
    let key = generation_key(tag::COUNTERS, generation);
    Ok(match krummi_tree::get(tx, data, &key)? {
        Some(bytes) if bytes.len() == 16 => (
            u64::from_le_bytes(bytes[..8].try_into().expect("8 bytes")),
            u64::from_le_bytes(bytes[8..].try_into().expect("8 bytes")),
        ),
        _ => (0, 0),
    })
}

/// The generation `generation` of the index of `roots` over vectors of `vector`, through the store's cache.
pub(crate) fn load_generation<S: PageSource + ?Sized>(
    shared: &Shared,
    tx: &S,
    index: u64,
    roots: &VectorRoots,
    vector: &VectorType,
    generation: u32,
) -> Result<Arc<Generation>> {
    shared
        .vectors
        .generation((index, roots.instance, generation), || {
            let key = generation_key(tag::HEADER, generation);
            let bytes = krummi_tree::get(tx, roots.data, &key)?;
            bytes
                .and_then(|b| Generation::from_bytes(b, usize::from(vector.dimension)))
                .ok_or_else(|| match krummi_tree::leaf_of(tx, roots.data, &key) {
                    Ok(page) => tx.damaged(page),
                    Err(error) => error,
                })
        })
}

// ---- Training ---------------------------------------------------------------------------------------------------

/// Trains the generation `state.next` of `target` from a sample of its rows read in short reads, and saves it, the
/// index then building it (VI4, VI9). Whether it trained; false with fewer than 10,000 rows with a vector, when a
/// rebuild is then cancelled.
fn train(shared: &Shared, target: &Target, state: &IndexState) -> Result<bool> {
    let work = &shared.vectors;
    let key = (target.index, target.roots.instance);
    if target.rows < MIN_TRAINING_ROWS && work.training_rows().is_none() {
        return cancel_rebuild(shared, target, state);
    }
    if lock(&work.state).untrainable.get(&key) == Some(&target.counter) {
        // Still too few rows: a rebuild that waits for them ends, as it would have when they were counted (VI9).
        return cancel_rebuild(shared, target, state);
    }
    let seed = target.index ^ u64::from(state.next).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let capacity = SAMPLE_PER_LIST * lists_for(target.rows.max(MIN_TRAINING_ROWS)) as usize;
    let Some((mut sample, counted)) = sample(shared, target, capacity, seed)? else {
        return Ok(false);
    };
    let rows = work.training_rows().unwrap_or(counted);
    if rows < MIN_TRAINING_ROWS || sample.is_empty() {
        lock(&work.state).untrainable.insert(key, target.counter);
        return cancel_rebuild(shared, target, state);
    }
    let lists = lists_for(rows);
    // The sample is uniform but kept in arrival order: shuffled before it is cut to its lists.
    let mut rng = Rng::new(seed ^ 1);
    for i in (1..sample.len()).rev() {
        sample.swap(i, rng.below(i as u64 + 1) as usize);
    }
    sample.truncate(SAMPLE_PER_LIST * lists as usize);
    work.pass(VectorPoint::Sampled);
    let dim = usize::from(target.vector.dimension);
    let stop = || work.stopping();
    let Some(generation) = Generation::train(&sample, dim, lists, rows, seed, &stop) else {
        return Ok(false);
    };
    drop(sample);
    let Some((mut write, started)) = turn(shared)? else {
        return Ok(false);
    };
    let Some(handle) = write.same_index(target, state)? else {
        write.abort();
        return Ok(true);
    };
    let g = state.next;
    let mut next = write.index_state(&handle, target.at)?;
    next.building = g;
    next.next = g + 1;
    next.cursor = None;
    next.end = write.greatest_key(&handle)?;
    let header = generation.to_bytes();
    write.set_data(&handle, target.at, &generation_key(tag::HEADER, g), &header)?;
    write.set_data(
        &handle,
        target.at,
        &generation_key(tag::COUNTERS, g),
        &[0; 16],
    )?;
    write.set_state(&handle, target.at, &next)?;
    finish_batch(shared, write, started)?;
    Ok(true)
}

/// Clears the rebuild that `state` has in progress, when it cannot train: one with fewer than 10,000 rows with a vector
/// does nothing (VI9). Whether it wrote.
fn cancel_rebuild(shared: &Shared, target: &Target, state: &IndexState) -> Result<bool> {
    if !state.rebuilding {
        return Ok(false);
    }
    let Some((mut write, started)) = turn(shared)? else {
        return Ok(false);
    };
    let Some(handle) = write.same_index(target, state)? else {
        write.abort();
        return Ok(true);
    };
    let mut next = write.index_state(&handle, target.at)?;
    next.rebuilding = false;
    write.set_state(&handle, target.at, &next)?;
    finish_batch(shared, write, started)?;
    Ok(true)
}

/// A uniform sample of up to `capacity` work vectors of the rows of `target` and the rows with a vector it counted,
/// read in short reads of [`SAMPLE_CHUNK`] rows; `None` when the store closes or the table goes.
fn sample(
    shared: &Shared,
    target: &Target,
    capacity: usize,
    seed: u64,
) -> Result<Option<(Vec<Vec<f32>>, u64)>> {
    let mut rng = Rng::new(seed);
    let mut reservoir: Vec<Vec<f32>> = Vec::new();
    let mut seen = 0u64;
    let mut after: Option<Vec<u8>> = None;
    loop {
        if shared.vectors.stopping() {
            return Ok(None);
        }
        let read = shared.read()?;
        let Some(table) = find(&read, target)? else {
            return Ok(None);
        };
        let mut taken = 0;
        let mut last = None;
        scan_rows(
            &read,
            &table,
            after.as_deref(),
            SAMPLE_CHUNK,
            |key, vector| {
                taken += 1;
                last = Some(key.to_vec());
                if let Some(vector) = vector {
                    seen += 1;
                    let mut work = Vec::new();
                    work_vector(&target.vector, vector, &mut work);
                    if reservoir.len() < capacity {
                        reservoir.push(work);
                    } else {
                        let j = rng.below(seen) as usize;
                        if j < capacity {
                            reservoir[j] = work;
                        }
                    }
                }
            },
            place_of(&table.schema, target.column),
        )?;
        if taken < SAMPLE_CHUNK {
            return Ok(Some((reservoir, seen)));
        }
        after = last;
    }
}

/// The table of `target` that `read` sees, by its identity, with the index of `target`; `None` when it is gone.
fn find<'r>(read: &'r ReadTransaction<'_>, target: &Target) -> Result<Option<ReadTable<'r>>> {
    let Some(name) = super::worker::current_name(read, target.table, &target.name)? else {
        return Ok(None);
    };
    let table = read.open_table(&name)?;
    let same = (table.schema.vector_indexes().get(target.at)).is_some_and(|i| i.id == target.index)
        && table.vector_roots[target.at].instance == target.roots.instance;
    Ok(same.then_some(table))
}

/// Calls `visit` with the key and the vector (none when null) of up to `limit` rows of `table` after `after`, in key
/// order; the vector is the live column at `place`.
fn scan_rows<'t>(
    read: &'t ReadTransaction<'_>,
    table: &'t ReadTable<'_>,
    after: Option<&[u8]>,
    limit: usize,
    mut visit: impl FnMut(&[u8], Option<&[u8]>),
    place: usize,
) -> Result<()> {
    let interval = Interval {
        prefix: &[],
        lower: after.map_or(Bound::Unbounded, Bound::Excluded),
        upper: Bound::Unbounded,
    };
    let mut versions = Versions::default();
    let at = table.at();
    for entry in krummi_tree::Range::new(&read.inner, table.rows_root, interval)?.take(limit) {
        let (key, bytes) = entry?;
        let row = table.reader.decode(
            &read.inner,
            at,
            &mut versions,
            std::borrow::Cow::Borrowed(key),
            bytes,
            false,
        )?;
        match row.value(place) {
            Some(Value::Vector(vector)) => visit(key, Some(&vector)),
            _ => visit(key, None),
        }
    }
    Ok(())
}

// ---- Batches ----------------------------------------------------------------------------------------------------

/// What a row is in a generation: its list, its code and whether a component saturated; none when the row has no
/// vector or is gone.
type Entry = Option<(u32, Vec<u8>, bool)>;

/// A key of a batch, as the read computed it: the row's vector there and its entries in the active and the building
/// generations.
struct Item {
    key: Vec<u8>,
    backfill: bool,
    vector: Option<Vec<u8>>,
    active: Entry,
    building: Entry,
}

/// The entry of `vector` (a value of `ty`) in `generation`.
fn entry_of(
    generation: &Generation,
    ty: &VectorType,
    vector: Option<&[u8]>,
    work: &mut Vec<f32>,
) -> Entry {
    let vector = vector?;
    work_vector(ty, vector, work);
    let list = generation.assign(work);
    let mut code = Vec::with_capacity(work.len());
    let saturated = generation.encode(work, &mut code);
    Some((list, code, saturated))
}

/// A batch of `target`: its pending keys and, while a generation is building, the next keys of the build, reconciled
/// in one write (document 12, *Reconcile*, *The worker*). Always true: it did work or found the index changed.
fn batch(shared: &Shared, target: &Target, state: &IndexState) -> Result<bool> {
    let work = &shared.vectors;
    let size = {
        let mut state = lock(&work.state);
        if state.batch == 0 {
            state.batch = FIRST_BATCH;
        }
        state.batch
    };
    let (items, end, active, building) = {
        let read = shared.read()?;
        let Some(table) = find(&read, target)? else {
            return Ok(true);
        };
        let now = read_state(&read.inner, &table.vector_roots[target.at])?;
        if now != *state {
            return Ok(true);
        }
        let roots = table.vector_roots[target.at];
        let generation = |g: u32| -> Result<Option<Arc<Generation>>> {
            if g == 0 {
                return Ok(None);
            }
            load_generation(shared, &read.inner, target.index, &roots, &target.vector, g).map(Some)
        };
        let (active, building) = (generation(state.active)?, generation(state.building)?);
        let mut keys: Vec<(Vec<u8>, bool)> = Vec::new();
        let every = Interval {
            prefix: &[],
            lower: Bound::Unbounded,
            upper: Bound::Unbounded,
        };
        for entry in krummi_tree::Range::new(&read.inner, roots.pending, every)?.take(size) {
            keys.push((entry?.0.to_vec(), false));
        }
        let mut end = false;
        if state.building != 0 {
            let want = size.saturating_sub(keys.len()).max(size / 4).max(1);
            let mut backfill = Vec::new();
            if let Some(last) = &state.end {
                scan_rows(
                    &read,
                    &table,
                    state.cursor.as_deref(),
                    want,
                    |key, _| {
                        if key <= last.as_slice() {
                            backfill.push(key.to_vec());
                        }
                    },
                    place_of(&table.schema, target.column),
                )?;
            }
            end = backfill.len() < want;
            keys.extend(backfill.into_iter().map(|key| (key, true)));
        }
        let mut versions = Versions::default();
        let mut scratch = Vec::new();
        let at = table.at();
        let mut items = Vec::with_capacity(keys.len());
        for (key, backfill) in keys {
            let vector = row_vector(
                &read.inner,
                &table.reader,
                at,
                &mut versions,
                table.rows_root,
                &key,
                place_of(&table.schema, target.column),
            )?;
            let active_entry = match &active {
                Some(g) if !backfill => {
                    entry_of(g, &target.vector, vector.as_deref(), &mut scratch)
                }
                _ => None,
            };
            let building_entry = match &building {
                Some(g) => entry_of(g, &target.vector, vector.as_deref(), &mut scratch),
                None => None,
            };
            items.push(Item {
                key,
                backfill,
                vector,
                active: active_entry,
                building: building_entry,
            });
        }
        (items, end, active, building)
    };
    let Some((mut write, started)) = turn(shared)? else {
        return Ok(false);
    };
    let Some(handle) = write.same_index(target, state)? else {
        write.abort();
        return Ok(true);
    };
    let mut next = write.index_state(&handle, target.at)?;
    let mut deltas = Deltas::default();
    let mut scratch = Vec::new();
    let mut applied = 0usize;
    let mut cut = false;
    let apply_start = Instant::now();
    for item in &items {
        if apply_start.elapsed() > APPLY_BUDGET {
            cut = true;
            break;
        }
        let (rows_root, reader, at) = write.rows_of(&handle)?;
        let place = place_of(&write.live_table(&handle)?.schema, target.column);
        let vector = {
            let mut versions = Versions::default();
            let at = At {
                name: &at.0,
                rows_root,
                history: at.1,
            };
            row_vector(
                &write.inner,
                &reader,
                at,
                &mut versions,
                rows_root,
                &item.key,
                place,
            )?
        };
        // The entries computed in the read hold when the row's vector is the same; otherwise they are computed again.
        let same = vector == item.vector;
        let mut fresh = |generation: &Option<Arc<Generation>>, computed: &Entry| -> Entry {
            match generation {
                Some(g) if !same => entry_of(g, &target.vector, vector.as_deref(), &mut scratch),
                Some(_) => computed.clone(),
                None => None,
            }
        };
        if item.backfill {
            let entry = fresh(&building, &item.building);
            write.reconcile(
                &handle,
                target.at,
                next.building,
                &item.key,
                entry,
                &mut deltas,
            )?;
            next.cursor = Some(item.key.clone());
        } else {
            let covered = next.building != 0 && next.covers(&item.key);
            if next.active != 0 {
                let entry = fresh(&active, &item.active);
                write.reconcile(
                    &handle,
                    target.at,
                    next.active,
                    &item.key,
                    entry,
                    &mut deltas,
                )?;
            }
            if covered {
                let entry = fresh(&building, &item.building);
                write.reconcile(
                    &handle,
                    target.at,
                    next.building,
                    &item.key,
                    entry,
                    &mut deltas,
                )?;
            }
            write.remove_pending(&handle, target.at, &item.key)?;
        }
        applied += 1;
    }
    write.apply_deltas(&handle, target.at, &deltas)?;
    if next.building != 0 && end && !cut {
        // The build passed the table's last key: the building generation is published (VI4, VI9).
        if next.active != 0 {
            write.set_data(
                &handle,
                target.at,
                &generation_key(tag::RETIRED, next.active),
                &[],
            )?;
        }
        // Its indicators when published, against which an automatic rebuild measures their growth (VI9).
        if let Some(generation) = &building {
            let data = write.roots(&handle, target.at)?.data;
            let baseline = Indicators::of(&write.inner, data, next.building, generation)?;
            write.set_data(
                &handle,
                target.at,
                &generation_key(tag::BASELINE, next.building),
                &baseline.to_bytes(),
            )?;
        }
        next.active = next.building;
        next.building = 0;
        next.rebuilding = false;
        next.cursor = None;
        next.end = None;
    }
    write.set_state(&handle, target.at, &next)?;
    shared.vectors.pass(VectorPoint::BatchTurn);
    let held = finish_batch(shared, write, started)?;
    // The next batch takes as many rows as fit in the budget at this batch's pace, its commit included: the commit's
    // share of the turn grows with the pages a batch changes.
    if applied > 0 {
        let per_item = held.as_secs_f64() / applied as f64;
        let fit = (TURN_TARGET.as_secs_f64() / per_item.max(1e-7)) as usize;
        // At most twice the last batch: a batch's commit costs more, and less evenly, than its apply.
        let mut state = lock(&work.state);
        state.batch = fit.min(2 * size).clamp(16, 4096);
    }
    Ok(true)
}

/// The rows with a vector of an index whose active generation is `active`: those the generation reflects, less the
/// pending ones it holds, plus the pending rows that have a vector (the live column at `place`).
fn rows_with_vector<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    roots: &VectorRoots,
    active: u32,
    reader: &'t Arc<Reader>,
    at: At<'_>,
    rows_root: u64,
    place: usize,
) -> Result<u64> {
    let (entries, _) = read_counters(tx, roots.data, active)?;
    let mut rows = entries;
    let all = Interval {
        prefix: &[],
        lower: Bound::Unbounded,
        upper: Bound::Unbounded,
    };
    let mut versions = Versions::default();
    for entry in krummi_tree::Range::new(tx, roots.pending, all)? {
        let (key, _) = entry?;
        let held = match row_ref(tx, roots.data, key)? {
            Some(reference) => {
                krummi_tree::get(tx, roots.data, &reference.locator(active))?.is_some()
            }
            None => false,
        };
        rows -= u64::from(held).min(rows);
        let vector = row_vector(tx, reader, at, &mut versions, rows_root, key, place)?;
        rows += u64::from(vector.is_some());
    }
    Ok(rows)
}

/// The vector of the row `key` of the rows tree at `rows_root` (the live column at `place`), none when the row is gone
/// or its vector is null.
fn row_vector<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    reader: &'t Arc<Reader>,
    at: At<'_>,
    versions: &mut Versions<'t>,
    rows_root: u64,
    key: &[u8],
    place: usize,
) -> Result<Option<Vec<u8>>> {
    let Some(bytes) = krummi_tree::get(tx, rows_root, key)? else {
        return Ok(None);
    };
    let row = reader.decode(
        tx,
        at,
        versions,
        std::borrow::Cow::Owned(key.to_vec()),
        bytes,
        false,
    )?;
    Ok(match row.value(place) {
        Some(Value::Vector(vector)) => Some(vector.into_owned()),
        _ => None,
    })
}

/// The changes a batch makes to the counters and list sizes of each generation.
#[derive(Default)]
pub(crate) struct Deltas {
    counters: HashMap<u32, (i64, i64)>,
    lists: HashMap<(u32, u32), i64>,
}

/// Deletes up to [`CLEANUP_BATCH`] keys of the first generation a publication retired, and its mark once it has none.
/// Whether it did.
fn cleanup(shared: &Shared, target: &Target) -> Result<bool> {
    let retired = {
        let read = shared.read()?;
        let Some(table) = find(&read, target)? else {
            return Ok(false);
        };
        let prefix = [tag::RETIRED];
        let found = krummi_tree::Range::new(
            &read.inner,
            table.vector_roots[target.at].data,
            Interval {
                prefix: &prefix,
                lower: Bound::Unbounded,
                upper: Bound::Unbounded,
            },
        )?
        .next()
        .transpose()?;
        match found {
            Some((key, _)) if key.len() == 5 => {
                u32::from_be_bytes(key[1..5].try_into().expect("4 bytes"))
            }
            _ => return Ok(false),
        }
    };
    let Some((mut write, started)) = turn(shared)? else {
        return Ok(false);
    };
    let Some(handle) = write.same_instance(target)? else {
        write.abort();
        return Ok(true);
    };
    let mut left = CLEANUP_BATCH;
    for tag in [
        tag::POSTING,
        tag::LOCATOR,
        tag::LONG_POSTING,
        tag::LONG_LOCATOR,
        tag::LIST_SIZE,
        tag::COUNTERS,
        tag::BASELINE,
        tag::HEADER,
    ] {
        let prefix = generation_key(tag, retired);
        left -= write.remove_some(&handle, target.at, &prefix, left)?;
        if left == 0 {
            break;
        }
    }
    if left > 0 {
        write.remove_data(&handle, target.at, &generation_key(tag::RETIRED, retired))?;
    }
    finish_batch(shared, write, started)?;
    Ok(true)
}

/// Starts a rebuild of the ready index of `target` when an indicator passes its threshold (VI9): it marks the index as
/// rebuilding, and the next steps train and build the new generation. Whether it did.
fn start_rebuild_if_due(shared: &Shared, target: &Target, state: &IndexState) -> Result<bool> {
    let (due, vector_rows) = {
        let read = shared.read()?;
        let Some(table) = find(&read, target)? else {
            return Ok(false);
        };
        let roots = table.vector_roots[target.at];
        if read_state(&read.inner, &roots)? != *state {
            return Ok(false);
        }
        let info = super::vector_index::read_info_with(
            shared,
            &read.inner,
            target.index,
            &roots,
            &target.vector,
        )?;
        let thresholds = shared.vectors.thresholds();
        let key = generation_key(tag::BASELINE, state.active);
        let baseline = krummi_tree::get(&read.inner, roots.data, &key)?
            .and_then(Indicators::from_bytes)
            .unwrap_or(Indicators {
                saturated: None,
                largest_list: None,
                rows: 1.0,
            });
        let grew = |now: Option<f64>, then: Option<f64>, threshold: f64| {
            now.is_some_and(|now| now > threshold && now > then.unwrap_or(0.0) * thresholds.growth)
        };
        let rows = info.rows_against_trained.unwrap_or(1.0) / baseline.rows.max(f64::MIN_POSITIVE);
        let rows_due = rows > thresholds.rows || rows < 1.0 / thresholds.rows;
        let grown = grew(info.saturated, baseline.saturated, thresholds.saturated)
            || grew(
                info.largest_list,
                baseline.largest_list,
                thresholds.largest_list,
            );
        if !grown && !rows_due {
            return Ok(false);
        }
        // The rows with a vector decide, not the table's rows: rows without one are never in the index (VI1, VI9).
        // Counting them reads the pending rows, so only an indicator past its threshold pays for it.
        let vector_rows = rows_with_vector(
            &read.inner,
            &roots,
            state.active,
            &table.reader,
            table.at(),
            table.rows_root,
            place_of(&table.schema, target.column),
        )?;
        let lists_change = rows_due
            && info
                .lists
                .is_some_and(|lists| lists_for(vector_rows) != lists as u32);
        (grown || lists_change, vector_rows)
    };
    if !due || vector_rows < MIN_TRAINING_ROWS {
        return Ok(false);
    }
    let Some((mut write, started)) = turn(shared)? else {
        return Ok(false);
    };
    let Some(handle) = write.same_index(target, state)? else {
        write.abort();
        return Ok(true);
    };
    let mut next = write.index_state(&handle, target.at)?;
    next.rebuilding = true;
    write.set_state(&handle, target.at, &next)?;
    finish_batch(shared, write, started)?;
    Ok(true)
}

/// The write turn for a batch, waiting in the queue until it comes or the store closes; `None` when it closes.
fn turn(shared: &Shared) -> Result<Option<(WriteTransaction<'_>, Instant)>> {
    match shared.open_write_unless(&|| shared.vectors.stopping()) {
        Ok(write) => Ok(Some((write, Instant::now()))),
        Err(error) if error.kind() == ErrorKind::Timeout => Ok(None),
        Err(error) => Err(error),
    }
}

/// Commits a batch without waiting for the disk (VI3) and records how long it held the turn, then, without the turn,
/// waits until it is durable; a batch of pending or build keys passes [`VectorPoint::BatchTurn`] first.
fn finish_batch(
    shared: &Shared,
    write: WriteTransaction<'_>,
    started: Instant,
) -> Result<Duration> {
    let result = write.commit_deferred();
    let held = started.elapsed();
    shared.vectors.record_turn(held);
    result?;
    // Outside the turn, the worker waits until its batch is durable: a read sees a deferred commit only then, and the
    // next batch must not read again the pending rows this one reconciled (VI3: the commit itself does not wait).
    shared.engine.make_durable()?;
    Ok(held)
}

impl<'s> WriteTransaction<'s> {
    /// The table of `target` in this write when it is still the same table, with the same index of the same instance
    /// in the same state (VI3): otherwise the batch is dropped.
    fn same_index(&self, target: &Target, state: &IndexState) -> Result<Option<WriteTable<'s>>> {
        let Some(handle) = self.same_instance(target)? else {
            return Ok(None);
        };
        let now = self.index_state(&handle, target.at)?;
        let same = now.active == state.active
            && now.building == state.building
            && now.cursor == state.cursor
            && now.end == state.end
            && now.next == state.next;
        Ok(same.then_some(handle))
    }

    /// The table of `target` in this write when it is still the same table with the same index of the same instance.
    fn same_instance(&self, target: &Target) -> Result<Option<WriteTable<'s>>> {
        let Some(handle) = super::worker::find_write_table(self, target.table, &target.name)?
        else {
            return Ok(None);
        };
        let state = self.live_table(&handle)?;
        let same = (state.schema.vector_indexes().get(target.at))
            .is_some_and(|i| i.id == target.index)
            && state.vector_roots[target.at].instance == target.roots.instance;
        drop(state);
        Ok(same.then_some(handle))
    }

    fn roots(&self, table: &WriteTable<'_>, at: usize) -> Result<VectorRoots> {
        Ok(self.live_table(table)?.vector_roots[at])
    }

    fn set_roots(
        &mut self,
        table: &WriteTable<'_>,
        at: usize,
        change: impl FnOnce(&mut VectorRoots),
    ) {
        let state = self
            .tables
            .get_mut()
            .get_mut(table.id)
            .expect("a live handle");
        change(&mut state.vector_roots[at]);
        state.dirty = true;
    }

    /// The greatest primary key of the table of `table`, none when it has no rows.
    fn greatest_key(&self, table: &WriteTable<'_>) -> Result<Option<Vec<u8>>> {
        let rows_root = self.live_table(table)?.rows_root;
        let all = Interval {
            prefix: &[],
            lower: Bound::Unbounded,
            upper: Bound::Unbounded,
        };
        let last = krummi_tree::Range::new(&self.inner, rows_root, all)?.next_back();
        Ok(last.transpose()?.map(|(key, _)| key.to_vec()))
    }

    /// The rows tree, the reader, and the name and history of the table of `table`.
    fn rows_of(&self, table: &WriteTable<'_>) -> Result<(u64, Arc<Reader>, (String, u64))> {
        let state = self.live_table(table)?;
        Ok((
            state.rows_root,
            Arc::clone(&state.reader),
            (state.name.clone(), state.history),
        ))
    }

    fn index_state(&self, table: &WriteTable<'_>, at: usize) -> Result<IndexState> {
        read_state(&self.inner, &self.roots(table, at)?)
    }

    fn set_data(
        &mut self,
        table: &WriteTable<'_>,
        at: usize,
        key: &[u8],
        value: &[u8],
    ) -> Result<()> {
        let root = self.roots(table, at)?.data;
        let root = krummi_tree::set(&mut self.inner, root, key, value)?.root;
        self.set_roots(table, at, |roots| roots.data = root);
        Ok(())
    }

    fn remove_data(&mut self, table: &WriteTable<'_>, at: usize, key: &[u8]) -> Result<bool> {
        let root = self.roots(table, at)?.data;
        let update = krummi_tree::remove(&mut self.inner, root, key)?;
        self.set_roots(table, at, |roots| roots.data = update.root);
        Ok(update.count_changed)
    }

    fn set_state(&mut self, table: &WriteTable<'_>, at: usize, state: &IndexState) -> Result<()> {
        self.set_data(table, at, &[tag::STATE], &state.encode())
    }

    fn remove_pending(&mut self, table: &WriteTable<'_>, at: usize, key: &[u8]) -> Result<()> {
        let root = self.roots(table, at)?.pending;
        let update = krummi_tree::remove(&mut self.inner, root, key)?;
        self.set_roots(table, at, |roots| {
            roots.pending = update.root;
            roots.pending_count -= u64::from(update.count_changed);
        });
        Ok(())
    }

    /// Makes the entry of the row `key` in `generation` be `entry` (document 12, *Reconcile*): it removes the entry the
    /// locator names, if any, and adds `entry`, if any.
    fn reconcile(
        &mut self,
        table: &WriteTable<'_>,
        at: usize,
        generation: u32,
        key: &[u8],
        entry: Entry,
        deltas: &mut Deltas,
    ) -> Result<()> {
        let reference = match &entry {
            Some(_) => self.row_ref_or_new(table, at, key)?,
            None => match row_ref(&self.inner, self.roots(table, at)?.data, key)? {
                Some(reference) => reference,
                // A long primary key that never got a number has no entry to remove.
                None => return Ok(()),
            },
        };
        let data = self.roots(table, at)?.data;
        let locator = reference.locator(generation);
        let old = krummi_tree::get(&self.inner, data, &locator)?.map(|v| v.to_vec());
        let mut root = data;
        if let Some(old) = old {
            let (list, saturated) =
                decode_locator(&old).ok_or_else(|| self.data_damage(data, &locator))?;
            match &entry {
                Some((new_list, _, _)) if *new_list == list => {}
                _ => {
                    root = krummi_tree::remove(
                        &mut self.inner,
                        root,
                        &reference.posting(generation, list),
                    )?
                    .root;
                    *deltas.lists.entry((generation, list)).or_default() -= 1;
                }
            }
            if entry.is_none() {
                root = krummi_tree::remove(&mut self.inner, root, &locator)?.root;
            }
            let counters = deltas.counters.entry(generation).or_default();
            counters.0 -= 1;
            counters.1 -= i64::from(saturated);
        }
        if let Some((list, code, saturated)) = entry {
            let moved = old_list_differs(&self.inner, root, &locator, list)?;
            root = krummi_tree::set(
                &mut self.inner,
                root,
                &reference.posting(generation, list),
                &code,
            )?
            .root;
            let mut value = list.to_be_bytes().to_vec();
            value.push(u8::from(saturated));
            root = krummi_tree::set(&mut self.inner, root, &locator, &value)?.root;
            if moved {
                *deltas.lists.entry((generation, list)).or_default() += 1;
            }
            let counters = deltas.counters.entry(generation).or_default();
            counters.0 += 1;
            counters.1 += i64::from(saturated);
        }
        self.set_roots(table, at, |roots| roots.data = root);
        Ok(())
    }

    /// How the data tree names the row of `primary`, giving a long primary key its number on its first entry.
    fn row_ref_or_new<'k>(
        &mut self,
        table: &WriteTable<'_>,
        at: usize,
        primary: &'k [u8],
    ) -> Result<RowRef<'k>> {
        let data = self.roots(table, at)?.data;
        if let Some(reference) = row_ref(&self.inner, data, primary)? {
            return Ok(reference);
        }
        let last = match krummi_tree::get(&self.inner, data, &[tag::LONG_NEXT])? {
            Some(bytes) if bytes.len() == 8 => {
                u64::from_be_bytes(bytes.try_into().expect("8 bytes"))
            }
            _ => 0,
        };
        let number = last + 1;
        self.set_data(table, at, &[tag::LONG_NEXT], &number.to_be_bytes())?;
        let mut key = vec![tag::LONG_KEY];
        key.extend(number.to_be_bytes());
        self.set_data(table, at, &key, primary)?;
        let bucket = bucket_key(primary);
        let data = self.roots(table, at)?.data;
        let mut entries =
            krummi_tree::get(&self.inner, data, &bucket)?.map_or_else(Vec::new, <[u8]>::to_vec);
        entries.extend(number.to_be_bytes());
        entries.extend(u32::try_from(primary.len()).expect("a key").to_be_bytes());
        entries.extend(primary);
        self.set_data(table, at, &bucket, &entries)?;
        Ok(RowRef::Long(number))
    }

    /// Adds the counters and list sizes of `deltas` to those the data tree holds.
    fn apply_deltas(&mut self, table: &WriteTable<'_>, at: usize, deltas: &Deltas) -> Result<()> {
        for (&generation, &(entries, saturated)) in &deltas.counters {
            let data = self.roots(table, at)?.data;
            let (old_entries, old_saturated) = read_counters(&self.inner, data, generation)?;
            let mut value = (old_entries as i64 + entries).max(0).to_le_bytes().to_vec();
            value.extend((old_saturated as i64 + saturated).max(0).to_le_bytes());
            self.set_data(
                table,
                at,
                &generation_key(tag::COUNTERS, generation),
                &value,
            )?;
        }
        for (&(generation, list), &delta) in &deltas.lists {
            if delta == 0 {
                continue;
            }
            let data = self.roots(table, at)?.data;
            let key = list_key(tag::LIST_SIZE, generation, list);
            let old = match krummi_tree::get(&self.inner, data, &key)? {
                Some(bytes) if bytes.len() == 8 => {
                    i64::from_le_bytes(bytes.try_into().expect("8 bytes"))
                }
                _ => 0,
            };
            let size = (old + delta).max(0);
            if size == 0 {
                self.remove_data(table, at, &key)?;
            } else {
                self.set_data(table, at, &key, &size.to_le_bytes())?;
            }
        }
        Ok(())
    }

    /// Removes up to `limit` keys of the data tree under `prefix`, and returns how many.
    fn remove_some(
        &mut self,
        table: &WriteTable<'_>,
        at: usize,
        prefix: &[u8],
        limit: usize,
    ) -> Result<usize> {
        let data = self.roots(table, at)?.data;
        let all = Interval {
            prefix,
            lower: Bound::Unbounded,
            upper: Bound::Unbounded,
        };
        let mut last = None;
        let mut count = 0;
        for entry in krummi_tree::Range::new(&self.inner, data, all)?.take(limit) {
            last = Some(entry?.0.to_vec());
            count += 1;
        }
        let Some(last) = last else {
            return Ok(0);
        };
        let upto = Interval {
            prefix: &[],
            lower: Bound::Included(prefix),
            upper: Bound::Included(&last),
        };
        let update = krummi_tree::remove_range(&mut self.inner, data, upto)?;
        self.set_roots(table, at, |roots| roots.data = update.root);
        Ok(count)
    }

    fn data_damage(&self, root: u64, key: &[u8]) -> krummi_pager::Error {
        match krummi_tree::leaf_of(&self.inner, root, key) {
            Ok(page) => self.inner.damaged(page),
            Err(error) => error,
        }
    }
}

/// Whether the locator at `locator` does not already name `list` (so that the posting is a new one of `list`).
fn old_list_differs<S: PageSource + ?Sized>(
    tx: &S,
    root: u64,
    locator: &[u8],
    list: u32,
) -> Result<bool> {
    Ok(match krummi_tree::get(tx, root, locator)? {
        Some(value) => decode_locator(value).is_none_or(|(old, _)| old != list),
        None => true,
    })
}

/// The list and the saturation of a locator's value.
pub(crate) fn decode_locator(value: &[u8]) -> Option<(u32, bool)> {
    if value.len() != 5 || value[4] > 1 {
        return None;
    }
    Some((
        u32::from_be_bytes(value[..4].try_into().ok()?),
        value[4] == 1,
    ))
}

impl crate::Store {
    /// A test hook, not part of the interface: pauses the vector indexes' worker, which takes no batch while paused
    /// (the batch it holds goes on), or resumes it.
    #[doc(hidden)]
    pub fn pause_vector_work(&self, paused: bool) {
        self.shared().vectors.pause(paused);
    }

    /// A test hook, not part of the interface: waits until the vector indexes' worker has nothing to do, then makes
    /// its commits durable so that new reads see them.
    #[doc(hidden)]
    pub fn wait_vector_idle(&self) {
        self.wait_vector_idle_until(None);
    }

    /// As [`Store::wait_vector_idle`], for at most `limit`: whether the worker became idle. A worker that retries a
    /// failure, as with a damaged catalog, never does.
    #[cfg(test)]
    pub(crate) fn wait_vector_idle_for(&self, limit: Duration) -> bool {
        self.wait_vector_idle_until(Some(Instant::now() + limit))
    }

    fn wait_vector_idle_until(&self, deadline: Option<Instant>) -> bool {
        let shared = self.shared();
        let work = &shared.vectors;
        {
            let mut state = lock(&work.state);
            while (state.busy || state.kicked) && !work.stopping() && !work.paused() {
                if lock(&work.thread)
                    .as_ref()
                    .is_none_or(JoinHandle::is_finished)
                {
                    break;
                }
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    return false;
                }
                // Bounded, so a worker that ends without notifying is seen at the next check.
                state = work
                    .wake
                    .wait_timeout(state, Duration::from_millis(50))
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
        }
        let _ = shared.engine.make_durable();
        true
    }

    /// A test hook, not part of the interface: the longest a batch of the worker held the write turn and the batches
    /// since the last call, which resets both (AC-VI3).
    #[doc(hidden)]
    pub fn take_vector_turns(&self) -> (Duration, u64) {
        let mut hooks = lock(&self.shared().vectors.hooks);
        let taken = (hooks.longest_turn, hooks.batches);
        hooks.longest_turn = Duration::ZERO;
        hooks.batches = 0;
        taken
    }

    /// A test hook, not part of the interface: waits until the worker waits at the held `point` (at most a minute), and
    /// whether it does.
    #[doc(hidden)]
    pub fn wait_vector_held(&self, point: VectorPoint) -> bool {
        let work = &self.shared().vectors;
        let start = std::time::Instant::now();
        let mut held = lock(&work.held);
        while *lock(&work.waiting) != Some(point) {
            if start.elapsed() > Duration::from_secs(60) {
                return false;
            }
            held = work
                .released
                .wait_timeout(held, Duration::from_millis(10))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }

    /// A test hook, not part of the interface: whether the vector indexes' worker thread is running.
    #[doc(hidden)]
    pub fn vector_work_alive(&self) -> bool {
        lock(&self.shared().vectors.thread)
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    /// A test hook, not part of the interface: holds a point of the worker or releases it.
    #[doc(hidden)]
    pub fn hold_vector_work(&self, point: VectorPoint, held: bool) {
        let work = &self.shared().vectors;
        let mut points = lock(&work.held);
        points.retain(|p| *p != point);
        if held {
            points.push(point);
        }
        drop(points);
        work.released.notify_all();
    }

    /// A test hook, not part of the interface: the threshold of rows that pass of the searches through an index, instead
    /// of the measured one (AC-VI6).
    #[doc(hidden)]
    pub fn set_vector_search_threshold(&self, rows: Option<u64>) {
        lock(&self.shared().vectors.hooks).search.threshold = rows;
    }

    /// A test hook, not part of the interface: whether a search through an index reranks every candidate of the lists it
    /// probes (AC-VI5).
    #[doc(hidden)]
    pub fn set_vector_rerank_all(&self, all: bool) {
        lock(&self.shared().vectors.hooks).search.rerank_all = all;
    }

    /// A test hook, not part of the interface: the rows a training counts, instead of those it reads (AC-VI15).
    #[doc(hidden)]
    pub fn set_vector_training_rows(&self, rows: Option<u64>) {
        lock(&self.shared().vectors.hooks).training_rows = rows;
    }

    /// A test hook, not part of the interface: the thresholds of an automatic rebuild instead of the defaults (AC-VI16).
    #[doc(hidden)]
    pub fn set_vector_rebuild_thresholds(&self, thresholds: Option<RebuildThresholds>) {
        lock(&self.shared().vectors.hooks).thresholds = thresholds;
    }
}

impl ReadTransaction<'_> {
    /// A test hook, not part of the interface: how the vector index `index` of `table` differs from the rows, as the
    /// snapshot sees them (AC-VI3, AC-VI4): for its active generation, and for its building one up to the cursor, each
    /// row that is not pending must have the locator, the posting and the code of its vector (none without one), every
    /// locator must name a row that is not pending, and the counters and list sizes must count them. Empty when it
    /// holds.
    #[doc(hidden)]
    pub fn check_vector_index(&self, table: &ReadTable<'_>, index: &str) -> Result<Vec<String>> {
        self.check_table(table)?;
        let at = (table.schema.vector_index_position(index))
            .ok_or_else(|| self.folder.missing_vector_index(&table.name, index))?;
        let (id, vector) = super::vector_index::vector_of(&table.schema, at);
        let place = (table.schema)
            .column_position(table.schema.vector_indexes()[at].column.id)
            .expect("a live column");
        let roots = table.vector_roots[at];
        let state = read_state(&self.inner, &roots)?;
        let tx = &self.inner;
        let pending = |key: &[u8]| -> Result<bool> {
            Ok(krummi_tree::get(tx, roots.pending, key)?.is_some())
        };
        let mut problems = Vec::new();
        let mut counted_pending = 0u64;
        let all = Interval {
            prefix: &[],
            lower: Bound::Unbounded,
            upper: Bound::Unbounded,
        };
        for entry in krummi_tree::Range::new(tx, roots.pending, all)? {
            entry?;
            counted_pending += 1;
        }
        if counted_pending != roots.pending_count {
            problems.push(format!(
                "pending count {} but {counted_pending} keys",
                roots.pending_count
            ));
        }
        for (g, cursor) in [(state.active, None), (state.building, Some(()))] {
            if g == 0 {
                continue;
            }
            let generation = load_generation(self.store, tx, id, &roots, &vector, g)?;
            let covers = |key: &[u8]| cursor.is_none() || state.covers(key);
            let mut work = Vec::new();
            let mut expected_entries = 0u64;
            let mut expected_saturated = 0u64;
            let mut sizes: HashMap<u32, u64> = HashMap::new();
            let mut versions = Versions::default();
            let rows_all = Interval {
                prefix: &[],
                lower: Bound::Unbounded,
                upper: Bound::Unbounded,
            };
            for entry in krummi_tree::Range::new(tx, table.rows_root, rows_all)? {
                let (key, bytes) = entry?;
                if !covers(key) || pending(key)? {
                    continue;
                }
                let row = table.reader.decode(
                    tx,
                    table.at(),
                    &mut versions,
                    std::borrow::Cow::Borrowed(key),
                    bytes,
                    false,
                )?;
                let vector_value = match row.value(place) {
                    Some(Value::Vector(v)) => Some(v.into_owned()),
                    _ => None,
                };
                let expected = entry_of(&generation, &vector, vector_value.as_deref(), &mut work);
                let reference = row_ref(tx, roots.data, key)?;
                let locator = match &reference {
                    Some(reference) => krummi_tree::get(tx, roots.data, &reference.locator(g))?
                        .and_then(decode_locator),
                    None => None,
                };
                match (&expected, locator) {
                    (None, None) => {}
                    (Some((list, code, saturated)), Some((found, found_saturated))) => {
                        if *list != found || *saturated != found_saturated {
                            problems.push(format!(
                                "g{g}: locator of {key:?} names list {found}, not {list}"
                            ));
                        }
                        let posting = match &reference {
                            Some(reference) => {
                                krummi_tree::get(tx, roots.data, &reference.posting(g, *list))?
                            }
                            None => None,
                        };
                        if posting != Some(code.as_slice()) {
                            problems.push(format!(
                                "g{g}: posting of {key:?} in list {list} is not its code"
                            ));
                        }
                        expected_entries += 1;
                        expected_saturated += u64::from(*saturated);
                        *sizes.entry(*list).or_default() += 1;
                    }
                    (Some(_), None) => problems.push(format!("g{g}: row {key:?} has no entry")),
                    (None, Some(_)) => {
                        problems.push(format!("g{g}: row {key:?} without a vector has an entry"))
                    }
                }
            }
            // Every locator and posting belongs to a row the check counted.
            let mut locators = 0u64;
            let mut postings = 0u64;
            for (locator, posting, long) in [
                (tag::LOCATOR, tag::POSTING, false),
                (tag::LONG_LOCATOR, tag::LONG_POSTING, true),
            ] {
                for (counted, tag, skip) in
                    [(&mut locators, locator, 0), (&mut postings, posting, 4)]
                {
                    let prefix = generation_key(tag, g);
                    for entry in krummi_tree::Range::new(
                        tx,
                        roots.data,
                        Interval {
                            prefix: &prefix,
                            lower: Bound::Unbounded,
                            upper: Bound::Unbounded,
                        },
                    )? {
                        let (key, _) = entry?;
                        let mut primary = &key[prefix.len() + skip..];
                        if long {
                            primary = long_primary(tx, roots.data, primary)?;
                        }
                        if covers(primary) && !pending(primary)? {
                            *counted += 1;
                        }
                    }
                }
            }
            if locators != expected_entries || postings != expected_entries {
                problems.push(format!(
                    "g{g}: {locators} locators and {postings} postings for {expected_entries} rows"
                ));
            }
            if counted_pending == 0 && cursor.is_none() {
                let (entries, saturated) = read_counters(tx, roots.data, g)?;
                if (entries, saturated) != (expected_entries, expected_saturated) {
                    problems.push(format!("g{g}: counters ({entries}, {saturated}) for ({expected_entries}, {expected_saturated})"));
                }
                for list in 0..generation.lists {
                    let stored =
                        match krummi_tree::get(tx, roots.data, &list_key(tag::LIST_SIZE, g, list))?
                        {
                            Some(bytes) => {
                                u64::from_le_bytes(bytes.try_into().unwrap_or([0xFF; 8]))
                            }
                            None => 0,
                        };
                    let expected = sizes.get(&list).copied().unwrap_or(0);
                    if stored != expected {
                        problems.push(format!("g{g}: list {list} size {stored} for {expected}"));
                    }
                }
            }
        }
        Ok(problems)
    }
}

impl WriteTransaction<'_> {
    /// Asks for a rebuild of the vector index `index` of `table` (VI9): when it commits, the index is ready and
    /// rebuilding, and the store's worker trains and builds a new generation while the searches use the current one,
    /// then publishes it at once. The request survives closing the store. It does nothing with a rebuild in progress,
    /// with the index training or building, or with fewer than 10,000 rows with a vector. Fails with
    /// [`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize) for a name without 1 to 1,024 bytes, with
    /// [`ErrorKind::Misuse`](crate::ErrorKind::Misuse) for a handle of another transaction, with
    /// [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound) for a table this write deleted, with
    /// [`ErrorKind::DifferentSchema`](crate::ErrorKind::DifferentSchema) for a vector index the table does not have, and
    /// with damage of a page it reads. It is a change for T10.
    pub fn rebuild_vector_index(&mut self, table: &WriteTable<'_>, index: &str) -> Result<()> {
        self.folder.check_index_name(index)?;
        let (at, roots, rows_root, reader, at_of, id, vector, place) = {
            let state = self.live_table(table)?;
            let at = (state.schema.vector_index_position(index))
                .ok_or_else(|| self.folder.missing_vector_index(&state.name, index))?;
            let (id, vector) = super::vector_index::vector_of(&state.schema, at);
            let place = (state.schema)
                .column_position(state.schema.vector_indexes()[at].column.id)
                .expect("a live column");
            (
                at,
                state.vector_roots[at],
                state.rows_root,
                Arc::clone(&state.reader),
                (state.name.clone(), state.history),
                id,
                vector,
                place,
            )
        };
        let _ = (id, vector);
        let mut current = read_state(&self.inner, &roots)?;
        if current.public() != super::vector_index::VectorIndexState::Ready {
            return Ok(());
        }
        let at_rows = At {
            name: &at_of.0,
            rows_root,
            history: at_of.1,
        };
        let rows = rows_with_vector(
            &self.inner,
            &roots,
            current.active,
            &reader,
            at_rows,
            rows_root,
            place,
        )?;
        if rows < MIN_TRAINING_ROWS {
            return Ok(());
        }
        current.rebuilding = true;
        self.set_state(table, at, &current)?;
        let state = self
            .tables
            .get_mut()
            .get_mut(table.id)
            .expect("a live handle");
        state.vector_changed = true;
        Ok(())
    }
}
