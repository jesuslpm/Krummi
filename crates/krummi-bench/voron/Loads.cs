// The 22 loads on Voron, exactly as design D10 of odd/tasks/benchmark.md, and the stores of the worker protocol
// (crates/krummi-bench/src/protocol.rs). Every load generates its data, its digest and its expected sums before any
// timer starts; a repetition's time goes from its first operation to its last commit or read (K8).

using System;
using System.Buffers.Binary;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.ExceptionServices;
using System.Threading;
using Voron;
using Voron.Data.BTrees;
using Voron.Impl;
using static VoronBench.Workload;

namespace VoronBench;

/// One `rep` line: the operations a repetition of a load counted and its time.
readonly record struct Rep(int Load, ulong Operations, ulong Nanos);

/// The digests a load's process prints and the work of one repetition. `Run` receives the opened store, or only its
/// folder for W16, which opens and closes it itself.
sealed record Plan((int Load, Digest Digest)[] Digests, Func<StorageEnvironment, string, Rep[]> Run);

/// Keys of 16 bytes and their values, stored contiguously in the order a load issues them.
sealed class Entries
{
    public readonly int Count;
    public readonly int ValueLen;
    readonly byte[] _keys;
    readonly byte[] _values;

    Entries(int count, int valueLen)
    {
        Count = count;
        ValueLen = valueLen;
        _keys = new byte[checked(count * Data.KeyLen)];
        _values = new byte[checked(count * valueLen)];
    }

    /// Entry `p` has key(index(p)) and value(valueIndex(p), valueLen); valueIndex defaults to index. A valueLen of 0
    /// holds keys only.
    public static Entries Of(int count, Func<int, ulong> index, int valueLen, Func<int, ulong> valueIndex = null)
    {
        var e = new Entries(count, valueLen);
        for (int p = 0; p < count; p++)
        {
            ulong i = index(p);
            Data.KeyInto(i, e.Key(p));
            if (valueLen > 0)
                Data.ValueInto(valueIndex == null ? i : valueIndex(p), e.Value(p));
        }
        return e;
    }

    public Span<byte> Key(int p) => _keys.AsSpan(p * Data.KeyLen, Data.KeyLen);

    public Span<byte> Value(int p) => _values.AsSpan(p * ValueLen, ValueLen);

    /// Feeds every key, followed by its value when there are values.
    public Digest FeedTo(Digest d)
    {
        for (int p = 0; p < Count; p++)
        {
            d.Feed(Key(p));
            if (ValueLen > 0)
                d.Feed(Value(p));
        }
        return d;
    }
}

static class Loads
{
    const string Bench = "bench", Multi = "multi", Fixed = "fixed";
    /// How long a writer waits for the write lock (see `Batches`).
    static readonly TimeSpan WriteLockWait = TimeSpan.FromMinutes(10);

    public static BenchFailure Missing(ReadOnlySpan<byte> key) =>
        new($"no se encuentra la clave {Convert.ToHexStringLower(key)}");

    static BenchFailure MissingFixed(ulong key) => new($"no se encuentra la clave entera {key}");

    public static BenchFailure WrongSum(ulong expected, ulong actual) =>
        new($"la suma de comprobación es {actual} y se esperaba {expected}");

    static BenchFailure WrongCount(ulong expected, ulong actual) =>
        new($"se leyeron {actual} entradas y se esperaban {expected}");

    // ---- Stores (protocol.rs) ----

    /// Builds the base store: indices 0..N in increasing order, writes of 1,000 (W2's data), and closes it.
    public static void Prepare(bool full, string store)
    {
        int n = (int)BaseEntries(full);
        var data = Entries.Of(n, p => (ulong)p, ValueLen);
        using var env = Open(store);
        Insert(env, data, 0, n, (int)BatchSize);
    }

    public static void Run(int load, bool full, string store, string source)
    {
        Start start = StartOf(load);
        if (start == Start.Empty && source != null)
            throw new BenchFailure($"la carga {Id(load)} empieza en un almacén vacío");
        if (start != Start.Empty && source == null)
            throw new BenchFailure($"la carga {Id(load)} necesita su almacén de partida");
        if (source != null && !Directory.Exists(source))
            throw new BenchFailure($"no existe el almacén de partida {source}");
        Program.NewStore(store);

        Plan plan = Build(load, full);
        foreach (var (l, d) in plan.Digests)
            Console.Out.WriteLine($"data {Id(l)} {d.Hex}");

        var (warmups, measured) = Repetitions(full);
        int reps = warmups + measured;
        if (ReadOnly(load))
        {
            // K4: a read-only load opens its store once; every repetition reads it with the caches the earlier left.
            string dir = RepDir(store, 0);
            CopyStore(source, dir);
            using var env = Open(dir);
            for (int i = 0; i < reps; i++)
            {
                Program.MaybeFail(load);
                Print(i, plan.Run(env, dir));
            }
            return;
        }
        for (int i = 0; i < reps; i++)
        {
            string dir = RepDir(store, i);
            if (source != null)
                CopyStore(source, dir);
            else
                Directory.CreateDirectory(dir);
            Rep[] result;
            if (load == W16)
            {
                Program.MaybeFail(load);
                result = plan.Run(null, dir);
            }
            else
            {
                using var env = Open(dir);
                Program.MaybeFail(load);
                result = plan.Run(env, dir);
            }
            Print(i, result);
            if (i + 1 < reps)
                Directory.Delete(dir, true);
        }
    }

    /// K3: Voron's options all keep their defaults. The journal is opened write-through, so every commit is durable.
    static StorageEnvironment Open(string path) =>
        new(StorageEnvironmentOptions.ForPath(path, null, null, null, null, null, null));

    static string RepDir(string store, int i) => Path.Combine(store, $"rep-{i}");

    static void CopyStore(string from, string to)
    {
        Directory.CreateDirectory(to);
        foreach (string file in Directory.GetFiles(from))
            File.Copy(file, Path.Combine(to, Path.GetFileName(file)));
        foreach (string dir in Directory.GetDirectories(from))
            CopyStore(dir, Path.Combine(to, Path.GetFileName(dir)));
    }

    static void Print(int index, Rep[] reps)
    {
        foreach (var r in reps)
            Console.Out.WriteLine($"rep {index} {Id(r.Load)} {r.Operations} {r.Nanos}");
    }

    // ---- The loads (D10) ----

    static Plan Build(int load, bool full)
    {
        Params p = ParamsOf(load, full);
        int n = (int)BaseEntries(full);
        ulong b = Data.IndexBase(load);
        switch (load)
        {
            case W1:
            case W2:
            case W3:
            {
                uint[] order = load == W1 ? SortedByKey(n) : Identity(n);
                return Fill(load, Entries.Of(n, i => order[i], ValueLen), (int)p.Batch);
            }
            case W4:
            case W5:
            case W6:
                return Writers(load, (int)p.Threads, (int)(p.Entries / p.Threads), b);
            case W7:
            case W8:
            case W9:
            {
                var keys = Enumerable.Range(0, (int)p.Threads)
                    .Select(t =>
                    {
                        uint[] order = Data.ReadOrder(load, (ulong)t, (ulong)n);
                        return Entries.Of(n, i => order[i], 0);
                    })
                    .ToArray();
                return Lookups(load, keys, ValueSums(n, 0, ValueLen).Sum());
            }
            case W10:
                return Scan(n);
            case W11:
                return Ranges(n, (int)p.Ranges, (int)p.RangeLen);
            case W12:
                return ReadersWithWriter(n, p);
            case W14:
            {
                var removed = Entries.Of(n, i => (ulong)i, 0);
                return new Plan([(load, removed.FeedTo(new Digest()))], (env, _) =>
                {
                    long s = Now();
                    Remove(env, removed, 0, n, (int)p.Batch);
                    return [new Rep(load, (ulong)n, Nanos(s, Now()))];
                });
            }
            case W15:
            {
                var removed = Entries.Of(n / 2, i => (ulong)i, 0);
                var inserted = Entries.Of(n / 2, i => b + (ulong)i, ValueLen);
                return new Plan([(load, inserted.FeedTo(removed.FeedTo(new Digest())))], (env, _) =>
                {
                    long s = Now();
                    Remove(env, removed, 0, removed.Count, (int)p.Batch);
                    Insert(env, inserted, 0, inserted.Count, (int)p.Batch);
                    return [new Rep(load, (ulong)n, Nanos(s, Now()))];
                });
            }
            case W16:
                return Opens((int)p.Opens);
            case W17:
                return Fill(load, Entries.Of((int)p.Entries, i => b + (ulong)i, p.ValueLen), (int)p.Batch);
            case W18:
            {
                ulong b17 = Data.IndexBase(W17);
                var keys = Entries.Of((int)p.Entries, i => b17 + (ulong)i, 0);
                return Lookups(load, [keys], ValueSums((int)p.Entries, b17, p.ValueLen).Sum());
            }
            case W19:
                return MultiAdd(p, b);
            case W20:
                return MultiRead(p);
            case W21:
                return FixedAdd(n, b, (int)p.Batch);
            case W22:
                return FixedRead(n);
            default:
                throw new BenchFailure($"carga no válida: {Id(load)}");
        }
    }

    /// W1, W2, W3, W17: insert every entry in its order, in writes of `batch`.
    static Plan Fill(int load, Entries data, int batch) =>
        new([(load, data.FeedTo(new Digest()))], (env, _) =>
        {
            long s = Now();
            Insert(env, data, 0, data.Count, batch);
            return [new Rep(load, (ulong)data.Count, Nanos(s, Now()))];
        });

    /// W4, W5, W6: thread t inserts B + t·per + j for j in 0..per, one entry per write, all threads started together.
    static Plan Writers(int load, int threads, int per, ulong b)
    {
        var data = Entries.Of(threads * per, i => b + (ulong)i, ValueLen);
        return new Plan([(load, data.FeedTo(new Digest()))], (env, _) =>
        {
            var (s, e) = Together(threads, t => Insert(env, data, t * per, (t + 1) * per, 1));
            return [new Rep(load, (ulong)data.Count, Nanos(s, e))];
        });
    }

    /// W7, W8, W9, W18: thread t, in its own read transaction, gets every key of keys[t]; the sum of the value bytes
    /// of each thread must be `expected`.
    static Plan Lookups(int load, Entries[] keys, ulong expected)
    {
        var digest = new Digest();
        foreach (var k in keys)
            k.FeedTo(digest);
        return new Plan([(load, digest)], (env, _) =>
        {
            var (s, e) = Together(keys.Length, t =>
            {
                Entries mine = keys[t];
                using var tx = env.ReadTransaction();
                Tree tree = ReadTree(tx, Bench);
                ulong sum = 0;
                for (int i = 0; i < mine.Count; i++)
                    sum += Get(tx, tree, mine.Key(i));
                if (sum != expected)
                    throw WrongSum(expected, sum);
            });
            return [new Rep(load, (ulong)(keys.Length * keys[0].Count), Nanos(s, e))];
        });
    }

    /// W10: the whole tree forward in one read transaction, summing key and value bytes.
    static unsafe Plan Scan(int n)
    {
        ulong expected = KeySums(n).Sum() + ValueSums(n, 0, ValueLen).Sum();
        var digest = new Digest();
        digest.FeedU64((ulong)n);
        return new Plan([(W10, digest)], (env, _) =>
        {
            long s = Now();
            using var tx = env.ReadTransaction();
            Tree tree = ReadTree(tx, Bench);
            ulong sum = 0, count = 0;
            using (var it = tree.Iterate(false))
            {
                if (it.Seek(Slices.BeforeAllKeys))
                {
                    do
                    {
                        sum += Data.ByteSum(it.CurrentKey.AsReadOnlySpan());
                        var v = it.CreateReaderForCurrent();
                        sum += Data.ByteSum(v.Base, v.Length);
                        count++;
                    } while (it.MoveNext());
                }
            }
            long e = Now();
            if (count != (ulong)n)
                throw WrongCount((ulong)n, count);
            if (sum != expected)
                throw WrongSum(expected, sum);
            return [new Rep(W10, count, Nanos(s, e))];
        });
    }

    /// W11: from each of the first `ranges` of permutation(N, seed(W11, 0)), scan forward from its key, included, up
    /// to `rangeLen` entries. The expected count and sum come from the base keys sorted, with prefix sums.
    static unsafe Plan Ranges(int n, int ranges, int rangeLen)
    {
        uint[] sorted = SortedByKey(n);
        uint[] keySums = KeySums(n), valueSums = ValueSums(n, 0, ValueLen);
        var rank = new int[n];
        var prefix = new ulong[n + 1];
        for (int pos = 0; pos < n; pos++)
        {
            rank[sorted[pos]] = pos;
            prefix[pos + 1] = prefix[pos] + keySums[sorted[pos]] + valueSums[sorted[pos]];
        }
        uint[] perm = Data.Permutation((ulong)n, Data.Seed(W11, 0));
        var starts = Entries.Of(ranges, i => perm[i], 0);
        ulong expectedCount = 0, expectedSum = 0;
        for (int i = 0; i < ranges; i++)
        {
            int pos = rank[perm[i]];
            int c = Math.Min(rangeLen, n - pos);
            expectedCount += (ulong)c;
            expectedSum += prefix[pos + c] - prefix[pos];
        }
        return new Plan([(W11, starts.FeedTo(new Digest()))], (env, _) =>
        {
            long s = Now();
            using var tx = env.ReadTransaction();
            Tree tree = ReadTree(tx, Bench);
            ulong sum = 0, count = 0;
            using (var it = tree.Iterate(false))
            {
                for (int i = 0; i < ranges; i++)
                {
                    Span<byte> start = starts.Key(i);
                    using (Slice.From(tx.Allocator, start, out Slice key))
                    {
                        if (!it.Seek(key) || !it.CurrentKey.AsReadOnlySpan().SequenceEqual(start))
                            throw Missing(start);
                    }
                    int read = 0;
                    do
                    {
                        sum += Data.ByteSum(it.CurrentKey.AsReadOnlySpan());
                        var v = it.CreateReaderForCurrent();
                        sum += Data.ByteSum(v.Base, v.Length);
                        read++;
                    } while (read < rangeLen && it.MoveNext());
                    count += (ulong)read;
                }
            }
            long e = Now();
            if (count != expectedCount)
                throw WrongCount(expectedCount, count);
            if (sum != expectedSum)
                throw WrongSum(expectedSum, sum);
            return [new Rep(W11, count, Nanos(s, e))];
        });
    }

    /// W12 and W13: 4 readers cycle through their read orders, a new read transaction every `ReadsPerTransaction`
    /// reads, while one writer does `Writes` writes of `Batch` new entries. All five start together; the time is the
    /// writer's. A reader checks after every read whether the writer finished and then stops without counting it.
    static Plan ReadersWithWriter(int n, Params p)
    {
        int readers = (int)p.Threads, perTx = (int)p.ReadsPerTransaction, batch = (int)p.Batch;
        uint[][] orders = Enumerable.Range(0, readers).Select(t => Data.ReadOrder(W12, (ulong)t, (ulong)n)).ToArray();
        Entries[] keys = orders.Select(order => Entries.Of(n, i => order[i], 0)).ToArray();
        uint[] valueSums = ValueSums(n, 0, ValueLen);
        ulong b13 = Data.IndexBase(W13);
        var written = Entries.Of((int)p.Writes * batch, i => b13 + (ulong)i, ValueLen);
        var readDigest = new Digest();
        foreach (var k in keys)
            k.FeedTo(readDigest);
        return new Plan([(W12, readDigest), (W13, written.FeedTo(new Digest()))], (env, _) =>
        {
            long ws = 0, we = 0;
            int done = 0;
            var counts = new ulong[readers];
            Together(readers + 1, t =>
            {
                if (t == readers)
                {
                    try
                    {
                        ws = Now();
                        Insert(env, written, 0, written.Count, batch);
                        we = Now();
                    }
                    finally
                    {
                        Volatile.Write(ref done, 1);
                    }
                    return;
                }
                Entries mine = keys[t];
                uint[] order = orders[t];
                int pos = 0;
                ulong count = 0;
                while (true)
                {
                    using var tx = env.ReadTransaction();
                    Tree tree = ReadTree(tx, Bench);
                    for (int r = 0; r < perTx; r++)
                    {
                        ulong sum = Get(tx, tree, mine.Key(pos));
                        if (sum != valueSums[order[pos]])
                            throw WrongSum(valueSums[order[pos]], sum);
                        if (Volatile.Read(ref done) == 1)
                        {
                            counts[t] = count;
                            return;
                        }
                        count++;
                        pos = pos + 1 == n ? 0 : pos + 1;
                    }
                }
            });
            ulong nanos = Nanos(ws, we);
            return [new Rep(W12, counts.Aggregate(0UL, (a, c) => a + c), nanos), new Rep(W13, (ulong)written.Count, nanos)];
        });
    }

    /// W16: `opens` times open the store, get key(j) for the j-th open and close it; the time includes both.
    static Plan Opens(int opens)
    {
        var keys = Entries.Of(opens, i => (ulong)i, 0);
        uint[] expected = ValueSums(opens, 0, ValueLen);
        return new Plan([(W16, keys.FeedTo(new Digest()))], (_, path) =>
        {
            long s = Now();
            for (int j = 0; j < opens; j++)
            {
                using var env = Open(path);
                using var tx = env.ReadTransaction();
                ulong sum = Get(tx, ReadTree(tx, Bench), keys.Key(j));
                if (sum != expected[j])
                    throw WrongSum(expected[j], sum);
            }
            return [new Rep(W16, (ulong)opens, Nanos(s, Now()))];
        });
    }

    /// W19: value v of key k is value(B + k·10 + v, 16), added for v in 0..10 { for k in 0..keys }, writes of 1,000.
    static Plan MultiAdd(Params p, ulong b)
    {
        int keys = (int)p.Keys, count = keys * (int)p.ValuesPerKey;
        var data = Entries.Of(count, i => b + (ulong)(i % keys), p.ValueLen,
            i => b + (ulong)(i % keys) * p.ValuesPerKey + (ulong)(i / keys));
        return new Plan([(W19, data.FeedTo(new Digest()))], (env, _) =>
        {
            long s = Now();
            Batches(env, count, (int)p.Batch, (tx, from, to) =>
            {
                Tree tree = tx.CreateTree(Multi);
                for (int i = from; i < to; i++)
                {
                    using (Slice.From(tx.Allocator, data.Key(i), out Slice key))
                    using (Slice.From(tx.Allocator, data.Value(i), out Slice value))
                        tree.MultiAdd(key, value);
                }
            });
            return [new Rep(W19, (ulong)count, Nanos(s, Now()))];
        });
    }

    /// W20: in one read transaction, every value of each key of W19 in Voron's order; 10 per key. The sum takes the
    /// key bytes once per key and every value's bytes.
    static Plan MultiRead(Params p)
    {
        int keys = (int)p.Keys, perKey = (int)p.ValuesPerKey;
        ulong b19 = Data.IndexBase(W19);
        var data = Entries.Of(keys, i => b19 + (ulong)i, 0);
        ulong expected = 0;
        for (int k = 0; k < keys; k++)
        {
            expected += Data.ByteSum(data.Key(k));
            expected += ValueSums(perKey, b19 + (ulong)k * (ulong)perKey, p.ValueLen).Sum();
        }
        return new Plan([(W20, data.FeedTo(new Digest()))], (env, _) =>
        {
            long s = Now();
            using var tx = env.ReadTransaction();
            Tree tree = ReadTree(tx, Multi);
            ulong sum = 0, count = 0;
            for (int k = 0; k < keys; k++)
            {
                Span<byte> key = data.Key(k);
                int values = 0;
                sum += Data.ByteSum(key);
                using (Slice.From(tx.Allocator, key, out Slice slice))
                using (var it = tree.MultiRead(slice))
                {
                    if (it.Seek(Slices.BeforeAllKeys))
                    {
                        do
                        {
                            sum += Data.ByteSum(it.CurrentKey.AsReadOnlySpan());
                            values++;
                        } while (it.MoveNext());
                    }
                }
                if (values == 0)
                    throw Missing(key);
                if (values != perKey)
                    throw WrongCount((ulong)perKey, (ulong)values);
                count += (ulong)values;
            }
            long e = Now();
            if (sum != expected)
                throw WrongSum(expected, sum);
            return [new Rep(W20, count, Nanos(s, e))];
        });
    }

    /// W21: the fixed-size tree, keys fixed_key(i) for i in 0..N with value(B + i, 8), writes of 1,000.
    static Plan FixedAdd(int n, ulong b, int batch)
    {
        var keys = new ulong[n];
        var values = new long[n];
        var digest = new Digest();
        Span<byte> value = stackalloc byte[8];
        for (int i = 0; i < n; i++)
        {
            keys[i] = Data.FixedKey((ulong)i);
            Data.ValueInto(b + (ulong)i, value);
            // Voron stores the long in memory order, little-endian on x64: exactly the 8 bytes of the value.
            values[i] = BinaryPrimitives.ReadInt64LittleEndian(value);
            digest.FeedU64(keys[i]);
            digest.Feed(value);
        }
        return new Plan([(W21, digest)], (env, _) =>
        {
            long s = Now();
            Batches(env, n, batch, (tx, from, to) =>
            {
                var tree = tx.FixedTreeFor(Fixed, 8);
                for (int i = from; i < to; i++)
                    tree.Add(Signed(keys[i]), values[i]);
            });
            return [new Rep(W21, (ulong)n, Nanos(s, Now()))];
        });
    }

    /// W22: in one read transaction, get fixed_key(i) for i in 0..N and sum the value bytes.
    static unsafe Plan FixedRead(int n)
    {
        var keys = new ulong[n];
        var digest = new Digest();
        for (int i = 0; i < n; i++)
        {
            keys[i] = Data.FixedKey((ulong)i);
            digest.FeedU64(keys[i]);
        }
        ulong expected = ValueSums(n, Data.IndexBase(W21), 8).Sum();
        return new Plan([(W22, digest)], (env, _) =>
        {
            long s = Now();
            using var tx = env.ReadTransaction();
            var tree = tx.FixedTreeFor(Fixed, 8);
            ulong sum = 0;
            for (int i = 0; i < n; i++)
            {
                byte* value = tree.ReadPtr(Signed(keys[i]), out int size);
                if (value == null)
                    throw MissingFixed(keys[i]);
                sum += Data.ByteSum(value, size);
            }
            long e = Now();
            if (sum != expected)
                throw WrongSum(expected, sum);
            return [new Rep(W22, (ulong)n, Nanos(s, e))];
        });
    }

    // ---- Helpers ----

    /// Voron's fixed-size keys are signed: flipping the top bit keeps the numeric order of the u64 keys.
    static long Signed(ulong key) => unchecked((long)(key ^ 0x8000000000000000UL));

    static Tree ReadTree(Transaction tx, string name) =>
        tx.ReadTree(name) ?? throw new BenchFailure($"el almacén no tiene el árbol {name}");

    /// Gets `key` and returns the sum of its value bytes, read in place.
    static unsafe ulong Get(Transaction tx, Tree tree, ReadOnlySpan<byte> key)
    {
        using (Slice.From(tx.Allocator, key, out Slice slice))
        {
            if (!tree.TryRead(slice, out ValueReader value))
                throw Missing(key);
            return Data.ByteSum(value.Base, value.Length);
        }
    }

    /// Runs `body` over [0, count) in write transactions of `batch` items, each committed.
    ///
    /// A writer waits for the write lock up to `WriteLockWait`. Voron's default limit is 30 s, and its lock is not
    /// fair, so with slow durable commits (about 6 ms each on WSL 2) a concurrent writer of W5 or W6 could wait longer
    /// than that and fail the run. The longer limit still turns a lock that is never released into a named failure
    /// of the load instead of a hang, since the orchestrator has no time limit per process.
    static void Batches(StorageEnvironment env, int count, int batch, Action<Transaction, int, int> body)
    {
        for (int from = 0; from < count; from += batch)
        {
            using var tx = env.WriteTransaction(timeout: WriteLockWait);
            body(tx, from, Math.Min(count, from + batch));
            tx.Commit();
        }
    }

    static void Insert(StorageEnvironment env, Entries data, int from, int to, int batch) =>
        Batches(env, to - from, batch, (tx, s, e) =>
        {
            Tree tree = tx.CreateTree(Bench);
            for (int i = from + s; i < from + e; i++)
            {
                using (Slice.From(tx.Allocator, data.Key(i), out Slice key))
                    tree.Add(key, data.Value(i));
            }
        });

    static void Remove(StorageEnvironment env, Entries data, int from, int to, int batch) =>
        Batches(env, to - from, batch, (tx, s, e) =>
        {
            Tree tree = tx.CreateTree(Bench);
            for (int i = from + s; i < from + e; i++)
            {
                using (Slice.From(tx.Allocator, data.Key(i), out Slice key))
                    tree.Delete(key);
            }
        });

    /// Starts `count` threads, created first and released together; returns the earliest start and the latest end.
    /// The first exception of any thread is rethrown once all have finished.
    static (long Start, long End) Together(int count, Action<int> body)
    {
        using var barrier = new Barrier(count);
        var starts = new long[count];
        var ends = new long[count];
        Exception failure = null;
        var threads = new Thread[count];
        for (int t = 0; t < count; t++)
        {
            int id = t;
            threads[t] = new Thread(() =>
            {
                barrier.SignalAndWait();
                starts[id] = Now();
                try
                {
                    body(id);
                }
                catch (Exception e)
                {
                    Interlocked.CompareExchange(ref failure, e, null);
                }
                ends[id] = Now();
            });
        }
        foreach (var thread in threads)
            thread.Start();
        foreach (var thread in threads)
            thread.Join();
        if (failure != null)
            ExceptionDispatchInfo.Throw(failure);
        return (starts.Min(), ends.Max());
    }

    /// The indices 0..n sorted by the bytes of their keys, built through Data.KeyInto so the order follows the key
    /// formula wherever it is defined.
    static uint[] SortedByKey(int n)
    {
        var items = new (byte[] Key, uint Index)[n];
        for (uint i = 0; i < n; i++)
        {
            var key = new byte[Data.KeyLen];
            Data.KeyInto(i, key);
            items[i] = (key, i);
        }
        Array.Sort(items, (x, y) => x.Key.AsSpan().SequenceCompareTo(y.Key));
        return items.Select(x => x.Index).ToArray();
    }

    /// The indices 0..n in increasing order.
    static uint[] Identity(int n) => Enumerable.Range(0, n).Select(i => (uint)i).ToArray();

    /// The byte sum of value(b + i, len) for i in 0..n.
    static uint[] ValueSums(int n, ulong b, int len)
    {
        var sums = new uint[n];
        var value = new byte[len];
        for (int i = 0; i < n; i++)
        {
            Data.ValueInto(b + (ulong)i, value);
            sums[i] = (uint)Data.ByteSum(value);
        }
        return sums;
    }

    /// The byte sum of key(i) for i in 0..n.
    static uint[] KeySums(int n)
    {
        var sums = new uint[n];
        Span<byte> key = stackalloc byte[Data.KeyLen];
        for (int i = 0; i < n; i++)
        {
            Data.KeyInto((ulong)i, key);
            sums[i] = (uint)Data.ByteSum(key);
        }
        return sums;
    }

    static ulong Sum(this uint[] values) => values.Aggregate(0UL, (a, v) => a + v);

    static long Now() => Stopwatch.GetTimestamp();

    /// Nanoseconds between two Stopwatch timestamps: ticks · 10⁹ / Stopwatch.Frequency, without overflow.
    static ulong Nanos(long start, long end) =>
        (ulong)((UInt128)(ulong)(end - start) * 1_000_000_000UL / (ulong)Stopwatch.Frequency);
}
