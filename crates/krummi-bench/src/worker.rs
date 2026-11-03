//! The Rust worker (task T2): runs one load of one Rust engine and prints the protocol lines of
//! [`crate::protocol`]. The loads follow design D10 of the execution document; they are generic over
//! [`engines::Store`], so the five engines run the same code.
//!
//! Every load generates its data, its random orders, its expected sums and its digest before the first repetition,
//! so no timer covers them (K8). A timer covers the operations of a repetition, from the first to the last commit or
//! read; opening, closing and copying stores are outside it, except in W16, which measures opening and closing.

use std::ffi::OsString;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::data::{
    Digest, byte_sum, fixed_key, index_base, key, permutation, read_order, seed, value_into,
};
use crate::engines::{self, Reader as _, Result, Store, Tree, Writer as _};
use crate::protocol::Line;
use crate::workload::{self, BATCH, Engine, KEY_LEN, Load, Scale, Start, VALUE_LEN, params};

/// The environment variable that makes the executable a worker (design D1).
pub const ROLE_VARIABLE: &str = "KRUMMI_BENCH_ROLE";

/// The environment variable of the simulated failures (D10, AC-K10): `<engine>:<load>:<kind>`.
pub const FAIL_VARIABLE: &str = "KRUMMI_BENCH_FAIL";

/// The exit code of the simulated failure `exit`: the process ends without finishing the load or saying why.
const EXIT_CODE: i32 = 3;

/// Runs the worker command in `args` and returns the exit code: `0`, or `1` after one line with the cause on
/// standard error.
pub fn main(args: Vec<OsString>) -> i32 {
    match command(args) {
        Ok(()) => 0,
        Err(cause) => {
            eprintln!("{}", cause.replace(['\r', '\n'], " "));
            1
        }
    }
}

fn command(args: Vec<OsString>) -> Result<()> {
    let args: Vec<String> = args
        .into_iter()
        .map(|a| {
            a.into_string()
                .map_err(|a| format!("argumento no válido: {}", a.to_string_lossy()))
        })
        .collect::<Result<_>>()?;
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["versions"] => print(&format!("versions lmdb {}", engines::lmdb::version())),
        ["prepare", engine, scale, store] => {
            let engine = rust_engine(engine)?;
            let scale = parse_scale(scale)?;
            let store = new_folder(store)?;
            dispatch(engine, Job::Prepare { scale, store })
        }
        ["run", engine, load, scale, store, source @ ..] if source.len() <= 1 => {
            let engine = rust_engine(engine)?;
            let load = Load::from_id(load)
                .filter(|l| l.has_process())
                .ok_or_else(|| format!("carga no válida: {load}"))?;
            if !workload::runs(engine, load) {
                return Err(format!(
                    "{} no ejecuta la carga {} (K6)",
                    engine.name(),
                    load.name()
                ));
            }
            let scale = parse_scale(scale)?;
            let source = match (load.start(), source) {
                (Start::Empty, []) => None,
                (Start::Base | Start::After(_), [source]) => {
                    let source = PathBuf::from(source);
                    if !source.is_dir() {
                        return Err(format!(
                            "no existe el almacén de partida {}",
                            source.display()
                        ));
                    }
                    Some(source)
                }
                (Start::Empty, _) => {
                    return Err(format!("la carga {} empieza vacía", load.id()));
                }
                (_, _) => {
                    return Err(format!(
                        "la carga {} necesita su almacén de partida",
                        load.id()
                    ));
                }
            };
            let fault = Fault::from_environment(engine, load)?;
            let store = new_folder(store)?;
            dispatch(
                engine,
                Job::Run(Context {
                    load,
                    scale,
                    store,
                    source,
                    fault,
                }),
            )
        }
        _ => Err(format!("orden no válida: {}", args.join(" "))),
    }
}

fn rust_engine(id: &str) -> Result<Engine> {
    match Engine::from_id(id) {
        Some(Engine::Voron) | None => Err(format!("motor no válido: {id}")),
        Some(engine) => Ok(engine),
    }
}

fn parse_scale(id: &str) -> Result<Scale> {
    Scale::from_id(id).ok_or_else(|| format!("escala no válida: {id}"))
}

/// Creates the folder `path`, which must not exist yet (protocol).
fn new_folder(path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if path.exists() {
        return Err(format!("la carpeta {} ya existe", path.display()));
    }
    fs::create_dir_all(&path)
        .map_err(|e| format!("no se puede crear la carpeta {}: {e}", path.display()))?;
    Ok(path)
}

enum Job {
    Prepare { scale: Scale, store: PathBuf },
    Run(Context),
}

fn dispatch(engine: Engine, job: Job) -> Result<()> {
    match engine {
        Engine::Krummi => execute::<engines::krummi::Store>(job),
        Engine::Lmdb => execute::<engines::lmdb::Store>(job),
        Engine::Redb => execute::<engines::redb::Store>(job),
        Engine::Canopydb => execute::<engines::canopydb::Store>(job),
        Engine::Fjall => execute::<engines::fjall::Store>(job),
        Engine::Voron => unreachable!("rust_engine refuses Voron"),
    }
}

fn execute<E: Store>(job: Job) -> Result<()> {
    match job {
        Job::Prepare { scale, store } => prepare::<E>(scale, &store),
        Job::Run(context) => run::<E>(&context),
    }
}

fn print(line: &str) -> Result<()> {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{line}")
        .and_then(|()| out.flush())
        .map_err(|e| format!("no se puede escribir la salida estándar: {e}"))
}

// ---------------------------------------------------------------------------------------------------------------
// Simulated failures (AC-K10).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    /// An engine error.
    Error,
    /// A key not found.
    Missing,
    /// A sum that differs.
    Checksum,
    /// The process ends with code 3 and no message.
    Exit,
}

impl Fault {
    /// The failure `KRUMMI_BENCH_FAIL` asks of `engine` in `load`, if any; W13 is W12's process.
    fn from_environment(engine: Engine, load: Load) -> Result<Option<Fault>> {
        match std::env::var(FAIL_VARIABLE) {
            Ok(value) => Fault::parse(&value, engine, load),
            Err(_) => Ok(None),
        }
    }

    fn parse(value: &str, engine: Engine, load: Load) -> Result<Option<Fault>> {
        let invalid = || format!("{FAIL_VARIABLE} no válida: {value}");
        let [e, l, kind] = value.split(':').collect::<Vec<_>>()[..] else {
            return Err(invalid());
        };
        let (Some(e), Some(l)) = (Engine::from_id(e), Load::from_id(l)) else {
            return Err(invalid());
        };
        let fault = match kind {
            "error" => Fault::Error,
            "missing" => Fault::Missing,
            "checksum" => Fault::Checksum,
            "exit" => Fault::Exit,
            _ => return Err(invalid()),
        };
        let same_process = l == load || (l == Load::W13 && load == Load::W12);
        Ok((e == engine && same_process).then_some(fault))
    }

    /// Fails as the engine would: through the same causes as a real failure.
    fn inject(self) -> Result<()> {
        match self {
            Fault::Error => Err("error simulado del motor".to_string()),
            Fault::Missing => Err(missing(&key(u64::MAX))),
            Fault::Checksum => check_sum(0, 1),
            Fault::Exit => std::process::exit(EXIT_CODE),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Checks of what an engine read (K8).

fn missing(key: &[u8]) -> String {
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    format!("no se encuentra la clave {hex}")
}

fn missing_fixed(key: u64) -> String {
    format!("no se encuentra la clave entera {key}")
}

fn check_sum(expected: u64, actual: u64) -> Result<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "la suma de comprobación es {actual} y se esperaba {expected}"
        ))
    }
}

fn check_count(expected: u64, actual: u64) -> Result<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "se leyeron {actual} entradas y se esperaban {expected}"
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The data of a load, generated before any timer.

/// The keys of `indices`.
fn keys(indices: impl IntoIterator<Item = u64>) -> Vec<[u8; KEY_LEN]> {
    indices.into_iter().map(key).collect()
}

/// The values of `indices`, `len` bytes each, in one buffer.
struct Values {
    bytes: Vec<u8>,
    len: usize,
}

impl Values {
    fn new(indices: impl IntoIterator<Item = u64>, len: usize) -> Values {
        let indices = indices.into_iter();
        let mut bytes = Vec::with_capacity(indices.size_hint().0 * len);
        for i in indices {
            let start = bytes.len();
            bytes.resize(start + len, 0);
            value_into(i, &mut bytes[start..]);
        }
        Values { bytes, len }
    }

    fn get(&self, j: usize) -> &[u8] {
        &self.bytes[j * self.len..(j + 1) * self.len]
    }
}

/// Entries to write: `key(i)` with `value(i, len)` for every index.
struct Entries {
    keys: Vec<[u8; KEY_LEN]>,
    values: Values,
}

impl Entries {
    fn new(indices: &[u64], len: usize) -> Entries {
        Entries {
            keys: keys(indices.iter().copied()),
            values: Values::new(indices.iter().copied(), len),
        }
    }

    fn len(&self) -> usize {
        self.keys.len()
    }

    /// Feeds the key and the value of every entry.
    fn feed(&self, digest: &mut Digest) {
        for (j, key) in self.keys.iter().enumerate() {
            digest.feed(key);
            digest.feed(self.values.get(j));
        }
    }
}

/// The byte sum of `value(i, len)`, what reading it adds.
fn value_sum(i: u64, buffer: &mut Vec<u8>, len: usize) -> u64 {
    buffer.resize(len, 0);
    value_into(i, buffer);
    byte_sum(buffer)
}

/// Keys to read and the sum of their values, `value(i, len)` for the key of index `i`.
struct Reads {
    keys: Vec<[u8; KEY_LEN]>,
    sum: u64,
}

impl Reads {
    fn new(indices: impl IntoIterator<Item = u64>, len: usize) -> Reads {
        let mut buffer = Vec::new();
        let mut sum = 0;
        let keys = indices
            .into_iter()
            .map(|i| {
                sum += value_sum(i, &mut buffer, len);
                key(i)
            })
            .collect();
        Reads { keys, sum }
    }
}

fn feed_keys<'k>(digest: &mut Digest, keys: impl IntoIterator<Item = &'k [u8; KEY_LEN]>) {
    for key in keys {
        digest.feed(key);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Repetitions.

struct Context {
    load: Load,
    scale: Scale,
    /// The folder of the load's stores, `<store>/rep-<i>`.
    store: PathBuf,
    /// The closed store the load starts from, never modified.
    source: Option<PathBuf>,
    fault: Option<Fault>,
}

/// What a repetition measured: for every load it counts (W12 and W13 in one), its operations and its time.
type Measured = Vec<(Load, u64, Duration)>;

impl Context {
    fn repetitions(&self) -> u32 {
        let (warmups, measured) = self.scale.repetitions();
        warmups + measured
    }

    fn folder(&self, repetition: u32) -> PathBuf {
        self.store.join(format!("rep-{repetition}"))
    }

    /// The store of `repetition`, fresh: a copy of the source, or empty (not measured).
    fn fresh(&self, repetition: u32) -> Result<PathBuf> {
        let folder = self.folder(repetition);
        match &self.source {
            Some(source) => copy_folder(source, &folder)?,
            None => fs::create_dir(&folder)
                .map_err(|e| format!("no se puede crear la carpeta {}: {e}", folder.display()))?,
        }
        Ok(folder)
    }

    /// Removes the store of an earlier repetition, which nothing needs.
    fn discard(&self, repetition: u32) -> Result<()> {
        let folder = self.folder(repetition);
        fs::remove_dir_all(&folder)
            .map_err(|e| format!("no se puede borrar la carpeta {}: {e}", folder.display()))
    }

    /// The simulated failure of this load, at the start of every repetition with its store open (W16: copied).
    fn inject(&self) -> Result<()> {
        self.fault.map_or(Ok(()), Fault::inject)
    }

    fn data(&self, load: Load, digest: Digest) -> Result<()> {
        print(&Line::data(load, digest))
    }

    fn report(&self, repetition: u32, measured: &[(Load, u64, Duration)]) -> Result<()> {
        for &(load, operations, time) in measured {
            let nanos = u64::try_from(time.as_nanos()).unwrap_or(u64::MAX);
            print(&Line::rep(load, repetition, operations, nanos))?;
        }
        Ok(())
    }

    /// Runs a modifying load: every repetition on a fresh store, opened before and closed after `body`.
    fn modifying<E: Store>(
        &self,
        tree: Tree,
        mut body: impl FnMut(&E) -> Result<Measured>,
    ) -> Result<()> {
        for i in 0..self.repetitions() {
            let folder = self.fresh(i)?;
            let store = E::open(&folder, tree)?;
            self.inject()?;
            let measured = body(&store)?;
            store.close()?;
            self.report(i, &measured)?;
            if i > 0 {
                self.discard(i - 1)?;
            }
        }
        Ok(())
    }

    /// Runs a read-only load: one store, opened once, for every repetition (K4).
    fn read_only<E: Store>(
        &self,
        tree: Tree,
        mut body: impl FnMut(&E) -> Result<Measured>,
    ) -> Result<()> {
        let folder = self.fresh(0)?;
        let store = E::open(&folder, tree)?;
        for i in 0..self.repetitions() {
            self.inject()?;
            let measured = body(&store)?;
            self.report(i, &measured)?;
        }
        store.close()
    }
}

/// Copies the folder `from` with its subfolders into the new folder `to`.
fn copy_folder(from: &Path, to: &Path) -> Result<()> {
    let fail =
        |path: &Path, e: std::io::Error| format!("no se puede copiar {}: {e}", path.display());
    fs::create_dir(to).map_err(|e| fail(to, e))?;
    for entry in fs::read_dir(from).map_err(|e| fail(from, e))? {
        let entry = entry.map_err(|e| fail(from, e))?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type().map_err(|e| fail(&path, e))?.is_dir() {
            copy_folder(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|e| fail(&path, e))?;
        }
    }
    Ok(())
}

/// Runs `f` and measures it.
fn timed<R>(f: impl FnOnce() -> Result<R>) -> Result<(R, Duration)> {
    let start = Instant::now();
    let result = f()?;
    Ok((result, start.elapsed()))
}

/// What one thread of [`parallel`] returned, with when it started and ended.
struct Finished<R> {
    result: R,
    start: Instant,
    end: Instant,
}

/// Runs `body(t)` for `t` in `0..threads`, each in its own thread, created before and released together (K8).
fn parallel<R: Send>(
    threads: usize,
    body: impl Fn(usize) -> Result<R> + Sync,
) -> Result<Vec<Finished<R>>> {
    let barrier = Barrier::new(threads);
    let outcomes: Vec<_> = thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let (barrier, body) = (&barrier, &body);
                scope.spawn(move || {
                    barrier.wait();
                    let start = Instant::now();
                    let result = body(t);
                    (result, start, Instant::now())
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join()).collect()
    });
    outcomes
        .into_iter()
        .map(|outcome| {
            let (result, start, end) =
                outcome.map_err(|_| "un hilo de la carga terminó con un pánico".to_string())?;
            Ok(Finished {
                result: result?,
                start,
                end,
            })
        })
        .collect()
}

/// The time from the first thread's start to the last one's end.
fn span<R>(finished: &[Finished<R>]) -> Duration {
    let start = finished.iter().map(|f| f.start).min();
    let end = finished.iter().map(|f| f.end).max();
    match (start, end) {
        (Some(start), Some(end)) => end - start,
        _ => Duration::ZERO,
    }
}

/// Writes `n` items in write transactions of `batch`, `op(writer, j)` for item `j`.
fn write_batches<E: Store>(
    store: &E,
    n: usize,
    batch: u64,
    mut op: impl FnMut(&mut E::Writer<'_>, usize) -> Result<()>,
) -> Result<()> {
    let batch = usize::try_from(batch).unwrap_or(usize::MAX).max(1);
    let mut start = 0;
    while start < n {
        let end = n.min(start + batch);
        store.write(|w| (start..end).try_for_each(|j| op(w, j)))?;
        start = end;
    }
    Ok(())
}

fn put_batches<E: Store>(store: &E, entries: &Entries, batch: u64) -> Result<()> {
    write_batches(store, entries.len(), batch, |w, j| {
        w.put(&entries.keys[j], entries.values.get(j))
    })
}

fn remove_batches<E: Store>(store: &E, keys: &[[u8; KEY_LEN]], batch: u64) -> Result<()> {
    write_batches(store, keys.len(), batch, |w, j| w.remove(&keys[j]))
}

/// Reads every key of `reads` in one read transaction; returns the sum of the values.
fn get_all<E: Store>(store: &E, reads: &Reads) -> Result<u64> {
    store.read(|r| {
        let mut sum = 0;
        for key in &reads.keys {
            sum += r.get(key)?.ok_or_else(|| missing(key))?;
        }
        Ok(sum)
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The base store and the loads (D10).

/// Builds the base store in `folder`: W2's data, `key(i)`/`value(i)` for `i` in `0..N`, writes of 1,000.
fn prepare<E: Store>(scale: Scale, folder: &Path) -> Result<()> {
    let indices: Vec<u64> = (0..scale.base_entries()).collect();
    let entries = Entries::new(&indices, VALUE_LEN);
    let store = E::open(folder, Tree::Normal)?;
    put_batches(&store, &entries, BATCH)?;
    store.close()
}

fn run<E: Store>(cx: &Context) -> Result<()> {
    let load = cx.load;
    let p = params(load, cx.scale);
    let n = cx.scale.base_entries();
    let base = index_base(load);
    let mut digest = Digest::default();
    match load {
        Load::W1 | Load::W2 | Load::W3 => {
            let mut indices: Vec<u64> = (0..n).collect();
            if load == Load::W1 {
                indices.sort_by_key(|&i| key(i));
            }
            let entries = Entries::new(&indices, VALUE_LEN);
            entries.feed(&mut digest);
            cx.data(load, digest)?;
            cx.modifying::<E>(Tree::Normal, |store| {
                let ((), time) = timed(|| put_batches(store, &entries, p.batch))?;
                Ok(vec![(load, n, time)])
            })
        }
        Load::W4 | Load::W5 | Load::W6 => {
            let per = p.entries / p.threads;
            let threads: Vec<Entries> = (0..p.threads)
                .map(|t| {
                    let indices: Vec<u64> = (0..per).map(|j| base + t * per + j).collect();
                    Entries::new(&indices, p.value_len)
                })
                .collect();
            threads.iter().for_each(|e| e.feed(&mut digest));
            cx.data(load, digest)?;
            cx.modifying::<E>(Tree::Normal, |store| {
                let finished = parallel(threads.len(), |t| {
                    let entries = &threads[t];
                    (0..entries.len()).try_for_each(|j| {
                        store.write(|w| w.put(&entries.keys[j], entries.values.get(j)))
                    })
                })?;
                Ok(vec![(load, per * p.threads, span(&finished))])
            })
        }
        Load::W7 | Load::W8 | Load::W9 => {
            let threads: Vec<Reads> = (0..p.threads)
                .map(|t| Reads::new(read_order(load, t, n).into_iter().map(u64::from), VALUE_LEN))
                .collect();
            gets::<E>(cx, &threads, n * p.threads, &mut digest)
        }
        Load::W10 => {
            let mut buffer = Vec::new();
            let expected: u64 = (0..n)
                .map(|i| byte_sum(&key(i)) + value_sum(i, &mut buffer, VALUE_LEN))
                .sum();
            digest.feed_u64(n);
            cx.data(load, digest)?;
            cx.read_only::<E>(Tree::Normal, |store| {
                let ((count, sum), time) = timed(|| store.read(|r| r.scan(None, u64::MAX)))?;
                check_count(n, count)?;
                check_sum(expected, sum)?;
                Ok(vec![(load, n, time)])
            })
        }
        Load::W11 => {
            let scans = Scans::new(n, p.ranges, p.range_len);
            feed_keys(&mut digest, &scans.starts);
            cx.data(load, digest)?;
            cx.read_only::<E>(Tree::Normal, |store| {
                let ((count, sum), time) = timed(|| {
                    store.read(|r| {
                        let (mut count, mut sum) = (0, 0);
                        for start in &scans.starts {
                            let (c, s) = r.scan(Some(start), p.range_len)?;
                            count += c;
                            sum += s;
                        }
                        Ok((count, sum))
                    })
                })?;
                check_count(scans.count, count)?;
                check_sum(scans.sum, sum)?;
                Ok(vec![(load, count, time)])
            })
        }
        Load::W12 => readers_with_writer::<E>(cx),
        Load::W13 => unreachable!("W13 runs in W12's process"),
        Load::W14 => {
            let removed = keys(0..n);
            feed_keys(&mut digest, &removed);
            cx.data(load, digest)?;
            cx.modifying::<E>(Tree::Normal, |store| {
                let ((), time) = timed(|| remove_batches(store, &removed, p.batch))?;
                Ok(vec![(load, n, time)])
            })
        }
        Load::W15 => {
            let half = n / 2;
            let removed = keys(0..half);
            let indices: Vec<u64> = (0..half).map(|j| base + j).collect();
            let inserted = Entries::new(&indices, VALUE_LEN);
            feed_keys(&mut digest, &removed);
            inserted.feed(&mut digest);
            cx.data(load, digest)?;
            cx.modifying::<E>(Tree::Normal, |store| {
                let ((), time) = timed(|| {
                    remove_batches(store, &removed, p.batch)?;
                    put_batches(store, &inserted, p.batch)
                })?;
                Ok(vec![(load, 2 * half, time)])
            })
        }
        Load::W16 => {
            let reads = Reads::new(0..p.opens, VALUE_LEN);
            feed_keys(&mut digest, &reads.keys);
            cx.data(load, digest)?;
            for i in 0..cx.repetitions() {
                let folder = cx.fresh(i)?;
                cx.inject()?;
                let (sum, time) = timed(|| {
                    let mut sum = 0;
                    for key in &reads.keys {
                        let store = E::open(&folder, Tree::Normal)?;
                        let value = store.read(|r| r.get(key))?;
                        store.close()?;
                        sum += value.ok_or_else(|| missing(key))?;
                    }
                    Ok(sum)
                })?;
                check_sum(reads.sum, sum)?;
                cx.report(i, &[(load, p.opens, time)])?;
                if i > 0 {
                    cx.discard(i - 1)?;
                }
            }
            Ok(())
        }
        Load::W17 => {
            let indices: Vec<u64> = (0..p.entries).map(|i| base + i).collect();
            let entries = Entries::new(&indices, p.value_len);
            entries.feed(&mut digest);
            cx.data(load, digest)?;
            cx.modifying::<E>(Tree::Normal, |store| {
                let ((), time) = timed(|| put_batches(store, &entries, p.batch))?;
                Ok(vec![(load, p.entries, time)])
            })
        }
        Load::W18 => {
            let reads = Reads::new(
                (0..p.entries).map(|i| index_base(Load::W17) + i),
                p.value_len,
            );
            gets::<E>(cx, &[reads], p.entries, &mut digest)
        }
        Load::W19 => {
            let adds: Vec<(u64, u64)> = (0..p.values_per_key)
                .flat_map(|v| (0..p.keys).map(move |k| (base + k, base + k * 10 + v)))
                .collect();
            let entries = Entries {
                keys: keys(adds.iter().map(|&(k, _)| k)),
                values: Values::new(adds.iter().map(|&(_, v)| v), p.value_len),
            };
            entries.feed(&mut digest);
            cx.data(load, digest)?;
            cx.modifying::<E>(Tree::Multi, |store| {
                let ((), time) = timed(|| {
                    write_batches(store, entries.len(), p.batch, |w, j| {
                        w.add(&entries.keys[j], entries.values.get(j))
                    })
                })?;
                Ok(vec![(load, p.keys * p.values_per_key, time)])
            })
        }
        Load::W20 => {
            let written = index_base(Load::W19);
            let keys = keys((0..p.keys).map(|k| written + k));
            let mut buffer = Vec::new();
            let expected: u64 = (0..p.keys)
                .map(|k| {
                    let values: u64 = (0..p.values_per_key)
                        .map(|v| value_sum(written + k * 10 + v, &mut buffer, p.value_len))
                        .sum();
                    byte_sum(&key(written + k)) + values
                })
                .sum();
            feed_keys(&mut digest, &keys);
            cx.data(load, digest)?;
            let values = p.keys * p.values_per_key;
            cx.read_only::<E>(Tree::Multi, |store| {
                let (sum, time) = timed(|| {
                    store.read(|r| {
                        let mut sum = 0;
                        for key in &keys {
                            let (count, values) = r.values(key)?;
                            check_count(p.values_per_key, count)?;
                            sum += byte_sum(key) + values;
                        }
                        Ok(sum)
                    })
                })?;
                check_sum(expected, sum)?;
                Ok(vec![(load, values, time)])
            })
        }
        Load::W21 => {
            let keys: Vec<u64> = (0..n).map(fixed_key).collect();
            let values = Values::new((0..n).map(|i| base + i), p.value_len);
            for (j, &key) in keys.iter().enumerate() {
                digest.feed_u64(key);
                digest.feed(values.get(j));
            }
            cx.data(load, digest)?;
            cx.modifying::<E>(Tree::Fixed, |store| {
                let ((), time) = timed(|| {
                    write_batches(store, keys.len(), p.batch, |w, j| {
                        w.put_fixed(keys[j], values.get(j))
                    })
                })?;
                Ok(vec![(load, n, time)])
            })
        }
        Load::W22 => {
            let keys: Vec<u64> = (0..n).map(fixed_key).collect();
            let mut buffer = Vec::new();
            let written = index_base(Load::W21);
            let expected: u64 = (0..n)
                .map(|i| value_sum(written + i, &mut buffer, p.value_len))
                .sum();
            keys.iter().for_each(|&k| digest.feed_u64(k));
            cx.data(load, digest)?;
            cx.read_only::<E>(Tree::Fixed, |store| {
                let (sum, time) = timed(|| {
                    store.read(|r| {
                        let mut sum = 0;
                        for &key in &keys {
                            sum += r.get_fixed(key)?.ok_or_else(|| missing_fixed(key))?;
                        }
                        Ok(sum)
                    })
                })?;
                check_sum(expected, sum)?;
                Ok(vec![(load, n, time)])
            })
        }
    }
}

/// W7–W9 and W18: thread `t` reads every key of `threads[t]` in its own read transaction.
fn gets<E: Store>(
    cx: &Context,
    threads: &[Reads],
    operations: u64,
    digest: &mut Digest,
) -> Result<()> {
    threads.iter().for_each(|r| feed_keys(digest, &r.keys));
    cx.data(cx.load, *digest)?;
    cx.read_only::<E>(Tree::Normal, |store| {
        let finished = parallel(threads.len(), |t| get_all(store, &threads[t]))?;
        for (f, reads) in finished.iter().zip(threads) {
            check_sum(reads.sum, f.result)?;
        }
        Ok(vec![(cx.load, operations, span(&finished))])
    })
}

/// The scans of W11: their start keys, and the entries and sum they read, from the base keys sorted.
struct Scans {
    starts: Vec<[u8; KEY_LEN]>,
    count: u64,
    sum: u64,
}

impl Scans {
    fn new(n: u64, ranges: u64, length: u64) -> Scans {
        let mut buffer = Vec::new();
        let mut sorted: Vec<([u8; KEY_LEN], u64)> = (0..n)
            .map(|i| {
                (
                    key(i),
                    byte_sum(&key(i)) + value_sum(i, &mut buffer, VALUE_LEN),
                )
            })
            .collect();
        sorted.sort_unstable_by_key(|entry| entry.0);
        // prefix[p] is the sum of the first p entries in key order.
        let mut prefix = Vec::with_capacity(sorted.len() + 1);
        prefix.push(0u64);
        for (_, s) in &sorted {
            prefix.push(prefix[prefix.len() - 1] + s);
        }
        let starts: Vec<[u8; KEY_LEN]> = permutation(n, seed(Load::W11, 0))
            .into_iter()
            .take(usize::try_from(ranges).unwrap_or(usize::MAX))
            .map(|i| key(u64::from(i)))
            .collect();
        let (mut count, mut sum) = (0, 0);
        for start in &starts {
            let position = sorted.partition_point(|(k, _)| k < start);
            let end = sorted
                .len()
                .min(position + usize::try_from(length).unwrap_or(usize::MAX));
            count += (end - position) as u64;
            sum += prefix[end] - prefix[position];
        }
        Scans { starts, count, sum }
    }
}

/// W12 and W13: four readers and one writer, released together; the time is the writer's (D10).
fn readers_with_writer<E: Store>(cx: &Context) -> Result<()> {
    let p = params(Load::W12, cx.scale);
    let n = cx.scale.base_entries();
    let readers = usize::try_from(p.threads).unwrap_or(usize::MAX);
    let orders: Vec<Vec<u32>> = (0..p.threads)
        .map(|t| read_order(Load::W12, t, n))
        .collect();
    let reader_keys: Vec<Vec<[u8; KEY_LEN]>> = orders
        .iter()
        .map(|order| keys(order.iter().map(|&i| u64::from(i))))
        .collect();
    let mut buffer = Vec::new();
    let sums: Vec<u64> = (0..n)
        .map(|i| value_sum(i, &mut buffer, VALUE_LEN))
        .collect();
    // The writer's entries are new keys of W13, the load it measures.
    let base = index_base(Load::W13);
    let indices: Vec<u64> = (0..p.writes * p.batch).map(|j| base + j).collect();
    let written = Entries::new(&indices, VALUE_LEN);

    let mut digest = Digest::default();
    reader_keys.iter().for_each(|k| feed_keys(&mut digest, k));
    cx.data(Load::W12, digest)?;
    let mut digest = Digest::default();
    written.feed(&mut digest);
    cx.data(Load::W13, digest)?;

    cx.modifying::<E>(Tree::Normal, |store| {
        let done = AtomicBool::new(false);
        let finished = parallel(readers + 1, |t| {
            if t == readers {
                // The writer: when it ends, also by a failure, the readers stop.
                let _stop = SetOnDrop(&done);
                put_batches(store, &written, p.batch)?;
                Ok(0)
            } else {
                read_until(
                    store,
                    &done,
                    &orders[t],
                    &reader_keys[t],
                    &sums,
                    p.reads_per_transaction,
                )
            }
        })?;
        let writer = &finished[readers];
        let time = writer.end - writer.start;
        let reads = finished[..readers].iter().map(|f| f.result).sum();
        Ok(vec![
            (Load::W12, reads, time),
            (Load::W13, p.writes * p.batch, time),
        ])
    })
}

/// A W12 reader: cycles through its keys, a new read transaction every `per_transaction` reads, until `done`;
/// returns the reads it finished before. A read that ends after `done` is not counted.
fn read_until<E: Store>(
    store: &E,
    done: &AtomicBool,
    order: &[u32],
    keys: &[[u8; KEY_LEN]],
    sums: &[u64],
    per_transaction: u64,
) -> Result<u64> {
    let mut count = 0;
    let mut position = 0;
    loop {
        let stopped = store.read(|r| {
            for _ in 0..per_transaction {
                let key = &keys[position];
                let sum = r.get(key)?.ok_or_else(|| missing(key))?;
                check_sum(sums[order[position] as usize], sum)?;
                if done.load(Ordering::Acquire) {
                    return Ok(true);
                }
                count += 1;
                position = (position + 1) % keys.len();
            }
            Ok(false)
        })?;
        if stopped {
            return Ok(count);
        }
    }
}

/// Sets the flag when dropped.
struct SetOnDrop<'a>(&'a AtomicBool);

impl Drop for SetOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faults_match_their_engine_and_load() {
        let parse = |v| Fault::parse(v, Engine::Lmdb, Load::W12);
        assert_eq!(parse("lmdb:W12:error"), Ok(Some(Fault::Error)));
        assert_eq!(parse("lmdb:W13:exit"), Ok(Some(Fault::Exit)));
        assert_eq!(parse("lmdb:W7:missing"), Ok(None));
        assert_eq!(parse("redb:W12:checksum"), Ok(None));
        assert!(parse("lmdb:W12:boom").is_err());
        assert!(parse("lmdb:W12").is_err());
        assert!(parse("lmdb:W99:error").is_err());
    }

    // The Voron program's self-test digest: key(0) then value(3, 100).
    #[test]
    fn the_digest_agrees_with_the_voron_program() {
        let mut digest = Digest::default();
        digest.feed(&key(0));
        digest.feed(&crate::data::value(3, 100));
        assert_eq!(digest.hex(), "95a8df032a8a94d0");
    }

    #[test]
    fn values_and_entries_are_those_of_the_generator() {
        let entries = Entries::new(&[3, 9], 20);
        assert_eq!(entries.keys, [key(3), key(9)]);
        assert_eq!(entries.values.get(1), crate::data::value(9, 20));
        let reads = Reads::new([3, 9], 20);
        assert_eq!(
            reads.sum,
            byte_sum(&crate::data::value(3, 20)) + byte_sum(&crate::data::value(9, 20))
        );
    }

    // W11's expected entries: from each start, up to `length` entries in key order, fewer at the end.
    #[test]
    fn scans_count_the_entries_from_each_start_to_the_end() {
        let n = 50;
        let scans = Scans::new(n, n, 10);
        let mut sorted: Vec<_> = (0..n).map(key).collect();
        sorted.sort();
        let count: u64 = scans
            .starts
            .iter()
            .map(|s| {
                let p = sorted.iter().position(|k| k == s).unwrap() as u64;
                10.min(n - p)
            })
            .sum();
        assert_eq!(scans.count, count);
        // Every start is a base key, each once.
        let mut starts = scans.starts.clone();
        starts.sort();
        assert_eq!(starts, sorted);
    }

    #[test]
    fn copies_keep_subfolders_and_contents() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("from");
        fs::create_dir_all(from.join("sub")).unwrap();
        fs::write(from.join("a"), b"abc").unwrap();
        fs::write(from.join("sub").join("b"), b"de").unwrap();
        let to = dir.path().join("to");
        copy_folder(&from, &to).unwrap();
        assert_eq!(fs::read(to.join("a")).unwrap(), b"abc");
        assert_eq!(fs::read(to.join("sub").join("b")).unwrap(), b"de");
        assert!(copy_folder(&from, &to).is_err(), "the target must be new");
    }

    #[test]
    fn checks_name_what_differs() {
        assert!(check_sum(5, 5).is_ok());
        assert!(check_sum(5, 6).unwrap_err().contains('5'));
        assert_eq!(
            check_count(3, 2).unwrap_err(),
            "se leyeron 2 entradas y se esperaban 3"
        );
        assert_eq!(
            check_sum(5, 6).unwrap_err(),
            "la suma de comprobación es 6 y se esperaba 5"
        );
        assert!(missing(&[0xAB, 0x01]).ends_with("ab01"));
    }
}
