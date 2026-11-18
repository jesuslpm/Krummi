//! The sort of the entries of an index being created (IX2 of the secondary indexes PRD; document 2 of the table design,
//! H15).
//!
//! The entries are kept in memory up to [`SORT_MEMORY`] bytes, counting each entry's key and value and its place in
//! the list. In a store on disk, an entry past that budget first has the entries in memory sorted and written as a run
//! into a file of `krummi.sort` ([`krummi_pager::SortFiles`]), and the runs are merged at the end: in groups of
//! [`MERGE_WAYS`] into longer runs while there are more, so that no merge holds more files open, and then by every
//! read of the sorted entries, which [`Sorted::cursor`] gives as often as asked. A temporary store has no files and
//! keeps every entry in memory (G9), failing with out of memory when the system does not give it (P13).
//!
//! A run file holds its entries in the order of their keys, each as a header of three little-endian `u32` (the length
//! of its key, the length of its value, and the length of its uniqueness prefix, `u32::MAX` for none) followed by its
//! key and its value. Nothing else reads them, and they never outlive the sort ([`krummi_pager::SortFiles`] removes
//! them).
//!
//! The failures of the files and of the memory of the sort are the transaction's (T11): [`Sorter`] marks them so, and
//! the caller marks those of a [`Cursor`].

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

use krummi_pager::{Result, SortFiles, SortReader, SortRun, SortWriter};
use krummi_tree::PageSource;
use krummi_tx::try_reserve;

use super::index::RowEntry;

/// Bytes of entries the sort keeps in memory before it writes them into a file of `krummi.sort`: 64 MiB, a few times
/// the 32 MiB the store keeps for the page copies and value runs of later commits, and few enough for an embedded
/// engine. Tests reduce it with [`testing::with_sort_memory`].
pub(super) const SORT_MEMORY: usize = 64 << 20;

/// The most runs one merge reads at once, each through a file of the sort and its buffer.
const MERGE_WAYS: usize = 64;

/// Bytes of the header of an entry in a run file.
const HEADER: usize = 12;

/// The prefix length of an entry without one (a null in its key columns).
const NO_PREFIX: u32 = u32::MAX;

thread_local! {
    /// The memory of the sorts of this thread, when a test reduced it.
    static MEMORY: Cell<Option<usize>> = const { Cell::new(None) };
    /// What a test runs once a sort of this thread wrote its first file.
    static PAUSE: RefCell<Option<Box<dyn FnMut()>>> = RefCell::new(None);
}

/// The sort of the entries of an index being created.
pub(super) struct Sorter<'p> {
    /// The entries in memory, in the order they came.
    entries: Vec<RowEntry>,
    /// Their bytes, as [`cost`] counts them.
    bytes: usize,
    budget: usize,
    runs: Vec<SortRun>,
    /// The files of the runs, `None` for a temporary store. Last, so that dropping the sorter removes them after
    /// everything else.
    files: Option<SortFiles<'p>>,
}

impl<'p> Sorter<'p> {
    /// A sort with the files `files`, or in memory only.
    pub(super) fn new(files: Option<SortFiles<'p>>) -> Sorter<'p> {
        Sorter {
            entries: Vec::new(),
            bytes: 0,
            budget: MEMORY.get().unwrap_or(SORT_MEMORY),
            runs: Vec::new(),
            files,
        }
    }

    /// Adds `entry`, first writing the entries in memory into a run file when it would pass the budget.
    pub(super) fn push<S: PageSource + ?Sized>(&mut self, tx: &S, entry: RowEntry) -> Result<()> {
        let cost = cost(&entry);
        if self.files.is_some() && !self.entries.is_empty() && self.bytes + cost > self.budget {
            self.spill(tx)?;
        }
        krummi_tree::reserve(tx, &mut self.entries, 1)?;
        self.bytes += cost;
        self.entries.push(entry);
        Ok(())
    }

    /// The entries, sorted by their keys, which are distinct: each ends with its row's primary key.
    pub(super) fn sorted<S: PageSource + ?Sized>(mut self, tx: &S) -> Result<Sorted<'p>> {
        if self.runs.is_empty() {
            self.entries.sort_unstable_by(by_key);
            return Ok(Sorted {
                entries: self.entries,
                runs: self.runs,
                files: self.files,
            });
        }
        if !self.entries.is_empty() {
            self.spill(tx)?;
        }
        self.entries = Vec::new();
        let files = self.files.as_mut().expect("runs only in a store on disk");
        while self.runs.len() > MERGE_WAYS {
            let group: Vec<SortRun> = self.runs.drain(..MERGE_WAYS).collect();
            let run = merge_runs(files, &group).map_err(|e| tx.fail(e))?;
            group.into_iter().for_each(|done| files.remove(done));
            self.runs.push(run);
        }
        Ok(Sorted {
            entries: self.entries,
            runs: self.runs,
            files: self.files,
        })
    }

    /// Sorts the entries in memory and writes them into a new run file, emptying the memory.
    fn spill<S: PageSource + ?Sized>(&mut self, tx: &S) -> Result<()> {
        self.entries.sort_unstable_by(by_key);
        let files = self
            .files
            .as_mut()
            .expect("a spill only in a store on disk");
        let run = write_run(files, &self.entries).map_err(|e| tx.fail(e))?;
        krummi_tree::reserve(tx, &mut self.runs, 1)?;
        self.runs.push(run);
        self.entries.clear();
        self.bytes = 0;
        if self.runs.len() == 1 {
            pause();
        }
        Ok(())
    }
}

/// The sorted entries of an index being created, in memory or in the run files they are merged from. Dropping them
/// removes the files.
pub(super) struct Sorted<'p> {
    entries: Vec<RowEntry>,
    runs: Vec<SortRun>,
    files: Option<SortFiles<'p>>,
}

impl Sorted<'_> {
    /// A read of the entries in order, from the first.
    pub(super) fn cursor(&self) -> Result<Cursor<'_>> {
        match &self.files {
            Some(files) if !self.runs.is_empty() => {
                Ok(Cursor::Merge(Merge::new(files, &self.runs)?))
            }
            _ => Ok(Cursor::Memory(self.entries.iter())),
        }
    }
}

/// A read of sorted entries, in the order of their keys.
pub(super) enum Cursor<'a> {
    Memory(std::slice::Iter<'a, RowEntry>),
    Merge(Merge),
}

impl Cursor<'_> {
    /// The next entry, or `None` after the last.
    pub(super) fn next(&mut self) -> Result<Option<&RowEntry>> {
        match self {
            Cursor::Memory(entries) => Ok(entries.next()),
            Cursor::Merge(merge) => merge.next(),
        }
    }
}

/// A merge of sorted runs: the next entry of each run, in a heap whose top is the smallest.
pub(super) struct Merge {
    readers: Vec<SortReader>,
    heap: BinaryHeap<Head>,
    /// The entry the merge last gave, until the next one is asked for.
    current: Option<Head>,
}

/// The next entry of the run that `source` reads.
struct Head {
    entry: RowEntry,
    source: usize,
}

impl Ord for Head {
    /// Reversed, so that the heap's top is the entry with the smallest key.
    fn cmp(&self, other: &Head) -> Ordering {
        other.entry.key.cmp(&self.entry.key)
    }
}

impl PartialOrd for Head {
    fn partial_cmp(&self, other: &Head) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Head {
    fn eq(&self, other: &Head) -> bool {
        self.entry.key == other.entry.key
    }
}

impl Eq for Head {}

impl Merge {
    fn new(files: &SortFiles<'_>, runs: &[SortRun]) -> Result<Merge> {
        let mut readers = Vec::new();
        try_reserve(&mut readers, runs.len())?;
        let mut heads = Vec::new();
        try_reserve(&mut heads, runs.len())?;
        for (source, run) in runs.iter().enumerate() {
            let mut reader = files.open(run)?;
            let mut entry = RowEntry {
                key: Vec::new(),
                prefix: None,
                value: Vec::new(),
            };
            if read_entry(&mut reader, &mut entry)? {
                heads.push(Head { entry, source });
            }
            readers.push(reader);
        }
        Ok(Merge {
            readers,
            heap: BinaryHeap::from(heads),
            current: None,
        })
    }

    fn next(&mut self) -> Result<Option<&RowEntry>> {
        if let Some(mut head) = self.current.take()
            && read_entry(&mut self.readers[head.source], &mut head.entry)?
        {
            // Within the heap's capacity: it held this head before.
            self.heap.push(head);
        }
        self.current = self.heap.pop();
        Ok(self.current.as_ref().map(|head| &head.entry))
    }
}

/// The bytes an entry in memory counts for the budget.
fn cost(entry: &RowEntry) -> usize {
    size_of::<RowEntry>() + entry.key.len() + entry.value.len()
}

fn by_key(a: &RowEntry, b: &RowEntry) -> Ordering {
    a.key.cmp(&b.key)
}

/// Writes the sorted `entries` into a new run file of `files`.
fn write_run(files: &mut SortFiles<'_>, entries: &[RowEntry]) -> Result<SortRun> {
    let mut writer = files.create()?;
    for entry in entries {
        write_entry(&mut writer, entry)?;
    }
    writer.finish()
}

/// Merges `runs` into a new run file of `files`.
fn merge_runs(files: &mut SortFiles<'_>, runs: &[SortRun]) -> Result<SortRun> {
    let mut merge = Merge::new(files, runs)?;
    let mut writer = files.create()?;
    while let Some(entry) = merge.next()? {
        write_entry(&mut writer, entry)?;
    }
    writer.finish()
}

fn write_entry(writer: &mut SortWriter<'_>, entry: &RowEntry) -> Result<()> {
    let len = |len: usize| u32::try_from(len).expect("an entry's parts are within a row's maximum");
    let prefix = entry.prefix.map_or(NO_PREFIX, len);
    let mut header = [0; HEADER];
    header[..4].copy_from_slice(&len(entry.key.len()).to_le_bytes());
    header[4..8].copy_from_slice(&len(entry.value.len()).to_le_bytes());
    header[8..].copy_from_slice(&prefix.to_le_bytes());
    writer.write(&header)?;
    writer.write(&entry.key)?;
    writer.write(&entry.value)
}

/// Reads the next entry of a run into `entry`, reusing its memory, or returns `false` after the last.
fn read_entry(reader: &mut SortReader, entry: &mut RowEntry) -> Result<bool> {
    let mut header = [0; HEADER];
    if !reader.read(&mut header)? {
        return Ok(false);
    }
    let part = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().expect("4 bytes"));
    read_part(reader, &mut entry.key, part(0) as usize)?;
    read_part(reader, &mut entry.value, part(4) as usize)?;
    let prefix = part(8);
    entry.prefix = (prefix != NO_PREFIX).then_some(prefix as usize);
    Ok(true)
}

fn read_part(reader: &mut SortReader, part: &mut Vec<u8>, len: usize) -> Result<()> {
    part.clear();
    try_reserve(part, len)?;
    part.resize(len, 0);
    reader.read_rest(part)
}

/// Runs the test's pause once a sort wrote its first file, if a test set one.
fn pause() {
    let hook = PAUSE.with(|p| p.borrow_mut().take());
    if let Some(mut hook) = hook {
        hook();
        PAUSE.with(|p| *p.borrow_mut() = Some(hook));
    }
}

/// Test hooks of the sort: the memory it keeps and a pause once it wrote its first file (AC-IX14). They are test
/// mechanisms, not public contracts.
pub mod testing {
    use super::*;

    /// Runs `f` with the memory of the sorts of this thread reduced to `bytes`, so that a few entries go to the files of
    /// `krummi.sort`.
    pub fn with_sort_memory<R>(bytes: usize, f: impl FnOnce() -> R) -> R {
        struct Restore(Option<usize>);
        impl Drop for Restore {
            fn drop(&mut self) {
                MEMORY.set(self.0);
            }
        }
        let _restore = Restore(MEMORY.replace(Some(bytes)));
        f()
    }

    /// Runs `f` with `pause` run once by each sort of this thread, right after it wrote its first file, while
    /// `krummi.sort` exists; a pause that panics interrupts the creation of the index.
    pub fn with_sort_pause<R>(pause: impl FnMut() + 'static, f: impl FnOnce() -> R) -> R {
        struct Restore(Option<Box<dyn FnMut()>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                let previous = self.0.take();
                PAUSE.with(|p| *p.borrow_mut() = previous);
            }
        }
        let previous = PAUSE.with(|p| p.borrow_mut().replace(Box::new(pause)));
        let _restore = Restore(previous);
        f()
    }
}
