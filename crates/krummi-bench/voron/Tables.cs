// The table loads W23-W32 on Voron's Table API (E21 and *Cargas del banco* of docs/prd/krummi/tables/tables-schema.prd.md;
// IX15 and *Cargas del banco* of docs/prd/krummi/tables/tables-indexes.prd.md). The data, the orders and the expected
// checksums come from TableData (Data.cs), a port of crates/krummi-bench/src/table_data.rs; the digests are those of
// `table_data::digest`. Every load generates them before any timer starts.
//
// The table of W23-W29: a TableSchema with the primary key on column 0 (`id`, 16 bytes) and the columns stored as Voron
// table values, in schema order: id (16 bytes), n (long), x (its IEEE bits as a long), ts (microseconds, long), flag
// (bool, one byte) and name (75 ASCII bytes). It is created, as RavenDB creates its collections (16 pages per raw data
// section), inside each write of the inserting loads, where it is a no-op once it exists. Every commit is durable, as
// in the other loads (K3).
//
// The table of W30-W32 (class Indexed) has the same columns and two indexes that are not unique, ix_n and ix_ts, whose
// keys sort as Krummi's entries do, so both engines return the same rows in the same order: n and ts are stored
// big-endian with the sign bit flipped, so the order of their bytes is that of their values, and ix_ts covers ts and
// then id, so rows with the same ts follow their primary key as in Krummi (IX8). The columns are laid out n, ts, id, x,
// flag, name, because an index of Voron covers contiguous columns. ix_n covers n alone: W31 checks that each of its
// lookups gives exactly one row, so the order of equal values never shows. The table and its indexes are created
// before the measured inserts of W30 (IX15).

using System;
using System.Buffers.Binary;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Voron;
using Voron.Data.Tables;
using Sparrow.Server;
using Voron.Impl;
using static VoronBench.Workload;

namespace VoronBench;

/// The rows of a table load in the order it issues them, each column contiguous; ids and names in pinned arrays, so
/// a row's builder points at them without copying.
sealed unsafe class TableRows
{
    public readonly int Count;
    public readonly long[] N, XBits, Ts;
    public readonly bool[] Flag;
    readonly byte[] _ids, _names;

    TableRows(int count)
    {
        Count = count;
        N = new long[count];
        XBits = new long[count];
        Ts = new long[count];
        Flag = new bool[count];
        _ids = GC.AllocateUninitializedArray<byte>(checked(count * TableData.IdLen), pinned: true);
        _names = GC.AllocateUninitializedArray<byte>(checked(count * TableData.NameLen), pinned: true);
    }

    /// Rows 0..count with keys of the given kind.
    public static TableRows Of(int count, bool uuid)
    {
        var rows = new TableRows(count);
        for (int p = 0; p < count; p++)
        {
            ulong r = (ulong)p;
            TableData.KeyInto(r, uuid, rows.Id(p));
            rows.N[p] = TableData.N(r);
            rows.XBits[p] = TableData.XBits(r);
            rows.Ts[p] = TableData.Ts(r);
            rows.Flag[p] = TableData.Flag(r);
            TableData.NameInto(r, rows.Name(p));
        }
        return rows;
    }

    public Span<byte> Id(int p) => _ids.AsSpan(p * TableData.IdLen, TableData.IdLen);

    public Span<byte> Name(int p) => _names.AsSpan(p * TableData.NameLen, TableData.NameLen);

    public byte* IdPtr(int p) => (byte*)Unsafe.AsPointer(ref _ids[p * TableData.IdLen]);

    public byte* NamePtr(int p) => (byte*)Unsafe.AsPointer(ref _names[p * TableData.NameLen]);

    /// Feeds every row as `Row::feed` of table_data.rs: id, n, x's bits and ts as 8 little-endian bytes each, flag as
    /// one byte and name, each its own item.
    public Digest FeedTo(Digest d)
    {
        Span<byte> flag = stackalloc byte[1];
        for (int p = 0; p < Count; p++)
        {
            d.Feed(Id(p));
            d.FeedU64((ulong)N[p]);
            d.FeedU64((ulong)XBits[p]);
            d.FeedU64((ulong)Ts[p]);
            flag[0] = Flag[p] ? (byte)1 : (byte)0;
            d.Feed(flag);
            d.Feed(Name(p));
        }
        return d;
    }
}

/// The keys of W30's indexes: values of n or ts stored so that the order of their bytes is that of their values, in
/// one pinned array, so a lookup's key points at them without copying.
sealed unsafe class SortableKeys
{
    public readonly long[] Stored;

    public SortableKeys(int count) => Stored = GC.AllocateUninitializedArray<long>(count, pinned: true);

    public byte* KeyPtr(int p) => (byte*)Unsafe.AsPointer(ref Stored[p]);
}

/// The ulid keys of W23's rows in the order a load visits them, in one pinned array.
sealed unsafe class TableKeys
{
    public readonly int Count;
    readonly byte[] _keys;

    public TableKeys(ReadOnlySpan<uint> rows)
    {
        Count = rows.Length;
        _keys = GC.AllocateUninitializedArray<byte>(checked(Count * TableData.IdLen), pinned: true);
        for (int p = 0; p < Count; p++)
            TableData.KeyInto(rows[p], false, Key(p));
    }

    public Span<byte> Key(int p) => _keys.AsSpan(p * TableData.IdLen, TableData.IdLen);

    public byte* KeyPtr(int p) => (byte*)Unsafe.AsPointer(ref _keys[p * TableData.IdLen]);
}

static partial class Loads
{
    const string TableName = "table";
    const int IdColumn = 0, NColumn = 1, XColumn = 2, TsColumn = 3, FlagColumn = 4, NameColumn = 5;

    /// The schema of every table load: the primary key on `id`, local to the table.
    static readonly TableSchema RowsSchema = new TableSchema().DefineKey(new TableSchema.IndexDef
    {
        StartIndex = IdColumn,
        Count = 1,
    });

    /// The table of W30-W32: the columns laid out n, ts, id, x, flag, name, the primary key on id, and the indexes ix_n
    /// on n and ix_ts on ts then id (see the file's comment).
    static class Indexed
    {
        public const int N = 0, Ts = 1, Id = 2, X = 3, Flag = 4, Name = 5;
        public static readonly TableSchema.IndexDef IxN, IxTs;
        public static readonly TableSchema Schema;

        static Indexed()
        {
            using (StorageEnvironment.GetStaticContext(out var ctx))
            {
                Slice.From(ctx, "ix_n", ByteStringType.Immutable, out Slice ixN);
                Slice.From(ctx, "ix_ts", ByteStringType.Immutable, out Slice ixTs);
                IxN = new TableSchema.IndexDef { StartIndex = N, Count = 1, Name = ixN };
                IxTs = new TableSchema.IndexDef { StartIndex = Ts, Count = 2, Name = ixTs };
            }
            Schema = new TableSchema()
                .DefineKey(new TableSchema.IndexDef { StartIndex = Id, Count = 1 })
                .DefineIndex(IxN)
                .DefineIndex(IxTs);
        }

        /// A long stored so that the order of its bytes is that of the values: the sign bit flipped, big-endian (the
        /// long's own bytes in memory, little-endian on the machines the benchmark runs on, are those of the result).
        public static long Sortable(long value) => BinaryPrimitives.ReverseEndianness(value ^ long.MinValue);

        public static long Value(long stored) => BinaryPrimitives.ReverseEndianness(stored) ^ long.MinValue;
    }

    static unsafe Plan BuildTable(int load, Params p)
    {
        int n = (int)p.Entries;
        switch (load)
        {
            case W23:
            case W24:
            case W25:
            {
                var rows = TableRows.Of(n, load == W24);
                return new Plan([(load, rows.FeedTo(new Digest()))], (env, _) =>
                {
                    long s = Now();
                    InsertRows(env, rows, (int)p.Batch);
                    return [new Rep(load, (ulong)n, Nanos(s, Now()))];
                });
            }
            case W30:
            {
                var rows = TableRows.Of(n, false);
                return new Plan([(W30, rows.FeedTo(new Digest()))], (env, _) =>
                {
                    // The empty table and its two indexes exist before the measured inserts (IX15).
                    using (var tx = env.WriteTransaction(timeout: WriteLockWait))
                    {
                        Indexed.Schema.Create(tx, TableName, 16);
                        tx.Commit();
                    }
                    long s = Now();
                    InsertIndexedRows(env, rows, (int)p.Batch);
                    return [new Rep(W30, (ulong)n, Nanos(s, Now()))];
                });
            }
            case W31:
                return IndexLookups(n);
            case W32:
                return IndexRanges(n, (int)p.Ranges, (int)p.RangeLen);
            case W26:
                return TableReads(n);
            case W27:
                return TableRanges(n, (int)p.Ranges, (int)p.RangeLen);
            case W28:
                return TableUpdates(n, (int)p.Batch);
            case W29:
            {
                var keys = new TableKeys(TableData.Order(W29, (ulong)n));
                return new Plan([(W29, FeedKeys(keys, new Digest()))], (env, _) =>
                {
                    long s = Now();
                    Batches(env, n, (int)p.Batch, (tx, from, to) =>
                    {
                        Table table = tx.OpenTable(RowsSchema, TableName) ?? throw NoTable();
                        for (int i = from; i < to; i++)
                        {
                            using (Slice.External(tx.Allocator, keys.KeyPtr(i), TableData.IdLen, out Slice key))
                            {
                                if (!table.DeleteByKey(key))
                                    throw Missing(keys.Key(i));
                            }
                        }
                    });
                    return [new Rep(W29, (ulong)n, Nanos(s, Now()))];
                });
            }
            default:
                throw new BenchFailure($"carga no válida: {Id(load)}");
        }
    }

    /// W23, W24, W25: insert every row in its order, in writes of `batch` (W25: one write with all of them).
    static unsafe void InsertRows(StorageEnvironment env, TableRows rows, int batch) =>
        Batches(env, rows.Count, batch, (tx, from, to) =>
        {
            RowsSchema.Create(tx, TableName, 16);
            Table table = tx.OpenTable(RowsSchema, TableName);
            for (int i = from; i < to; i++)
            {
                using (table.Allocate(out TableValueBuilder builder))
                {
                    builder.Add(rows.IdPtr(i), TableData.IdLen);
                    builder.Add(rows.N[i]);
                    builder.Add(rows.XBits[i]);
                    builder.Add(rows.Ts[i]);
                    builder.Add(rows.Flag[i]);
                    builder.Add(rows.NamePtr(i), TableData.NameLen);
                    table.Insert(builder);
                }
            }
        });

    /// W30: insert every row in its order into the table of Indexed, in writes of `batch`.
    static unsafe void InsertIndexedRows(StorageEnvironment env, TableRows rows, int batch) =>
        Batches(env, rows.Count, batch, (tx, from, to) =>
        {
            Table table = tx.OpenTable(Indexed.Schema, TableName) ?? throw NoTable();
            for (int i = from; i < to; i++)
            {
                using (table.Allocate(out TableValueBuilder builder))
                {
                    builder.Add(Indexed.Sortable(rows.N[i]));
                    builder.Add(Indexed.Sortable(rows.Ts[i]));
                    builder.Add(rows.IdPtr(i), TableData.IdLen);
                    builder.Add(rows.XBits[i]);
                    builder.Add(rows.Flag[i]);
                    builder.Add(rows.NamePtr(i), TableData.NameLen);
                    table.Insert(builder);
                }
            }
        });

    /// W31: in one read transaction, look up in ix_n the n of every row of W30's table, in order(W31), with the interval
    /// of that value (the entries whose key is that value), all columns of the row read into the checksum, which must be
    /// that of every row. Each lookup must give exactly one row.
    static unsafe Plan IndexLookups(int n)
    {
        uint[] order = TableData.Order(W31, (ulong)n);
        var keys = new SortableKeys(n);
        var digest = new Digest();
        ulong expected = 0;
        for (int i = 0; i < n; i++)
        {
            long value = TableData.N(order[i]);
            keys.Stored[i] = Indexed.Sortable(value);
            digest.FeedU64((ulong)value);
            expected = unchecked(expected + TableData.RowChecksum((ulong)i));
        }
        digest.FeedU64(expected);
        return new Plan([(W31, digest)], (env, _) =>
        {
            long s = Now();
            using var tx = env.ReadTransaction();
            Table table = tx.OpenTable(Indexed.Schema, TableName) ?? throw NoTable();
            ulong sum = 0;
            for (int i = 0; i < n; i++)
            {
                int found = 0;
                using (Slice.External(tx.Allocator, keys.KeyPtr(i), sizeof(long), out Slice key))
                {
                    foreach (var entry in table.SeekForwardFromPrefix(Indexed.IxN, key, key, 0))
                    {
                        sum = unchecked(sum + IndexedRowChecksum(in entry.Result.Reader));
                        found++;
                    }
                }
                if (found != 1)
                    throw new BenchFailure($"ix_n da {found} filas con n = {Indexed.Value(keys.Stored[i])} y se esperaba 1");
            }
            long e = Now();
            if (sum != expected)
                throw WrongSum(expected, sum);
            return [new Rep(W31, (ulong)n, Nanos(s, e))];
        });
    }

    /// W32: in one read transaction, from the ts of each of the first `ranges` rows of order(W32), included, up to
    /// `rangeLen` rows forward in ix_ts, all columns read into the checksum. The expected rows of a range are the next
    /// ones of TableData.TsOrder from the first with that ts; their count and checksum come from prefix sums.
    static unsafe Plan IndexRanges(int n, int ranges, int rangeLen)
    {
        uint[] order = TableData.Order(W32, (ulong)n);
        uint[] sorted = TableData.TsOrder((ulong)n);
        var sortedTs = new long[n];
        var prefix = new ulong[n + 1];
        for (int i = 0; i < n; i++)
        {
            sortedTs[i] = TableData.Ts(sorted[i]);
            prefix[i + 1] = unchecked(prefix[i] + TableData.RowChecksum(sorted[i]));
        }
        var starts = new SortableKeys(ranges);
        var digest = new Digest();
        ulong expectedCount = 0, expectedSum = 0;
        for (int i = 0; i < ranges; i++)
        {
            long ts = TableData.Ts(order[i]);
            starts.Stored[i] = Indexed.Sortable(ts);
            digest.FeedU64((ulong)ts);
            int from = FirstAtLeast(sortedTs, ts), to = Math.Min(n, from + rangeLen);
            expectedCount += (ulong)(to - from);
            expectedSum = unchecked(expectedSum + prefix[to] - prefix[from]);
        }
        digest.FeedU64(expectedCount);
        digest.FeedU64(expectedSum);
        return new Plan([(W32, digest)], (env, _) =>
        {
            long s = Now();
            using var tx = env.ReadTransaction();
            Table table = tx.OpenTable(Indexed.Schema, TableName) ?? throw NoTable();
            ulong sum = 0, count = 0;
            for (int i = 0; i < ranges; i++)
            {
                int read = 0;
                using (Slice.External(tx.Allocator, starts.KeyPtr(i), sizeof(long), out Slice key))
                {
                    foreach (var entry in table.SeekForwardFrom(Indexed.IxTs, key, 0))
                    {
                        ref TableValueReader row = ref entry.Result.Reader;
                        // An interval starts at an existing value: its first row has that ts.
                        if (read == 0 && *(long*)row.Read(Indexed.Ts, out int _) != starts.Stored[i])
                            throw MissingTs(starts.Stored[i]);
                        sum = unchecked(sum + IndexedRowChecksum(in row));
                        if (++read == rangeLen)
                            break;
                    }
                }
                if (read == 0)
                    throw MissingTs(starts.Stored[i]);
                count += (ulong)read;
            }
            long e = Now();
            if (count != expectedCount)
                throw WrongCount(expectedCount, count);
            if (sum != expectedSum)
                throw WrongSum(expectedSum, sum);
            return [new Rep(W32, count, Nanos(s, e))];
        });
    }

    /// The first position of `sorted` whose value is at least `value`, or its length.
    static int FirstAtLeast(long[] sorted, long value)
    {
        int lo = 0, hi = sorted.Length;
        while (lo < hi)
        {
            int mid = lo + (hi - lo) / 2;
            if (sorted[mid] < value)
                lo = mid + 1;
            else
                hi = mid;
        }
        return lo;
    }

    static BenchFailure MissingTs(long stored) => new($"no se encuentra ts = {Indexed.Value(stored)} en ix_ts");

    /// W26: in one read transaction, every row of W23's table by its key, in order(W26), all columns read into the
    /// checksum, which must be that of every row.
    static unsafe Plan TableReads(int n)
    {
        var keys = new TableKeys(TableData.Order(W26, (ulong)n));
        ulong expected = 0;
        for (int r = 0; r < n; r++)
            expected = unchecked(expected + TableData.RowChecksum((ulong)r));
        var digest = FeedKeys(keys, new Digest());
        digest.FeedU64(expected);
        return new Plan([(W26, digest)], (env, _) =>
        {
            long s = Now();
            using var tx = env.ReadTransaction();
            Table table = tx.OpenTable(RowsSchema, TableName) ?? throw NoTable();
            ulong sum = 0;
            for (int i = 0; i < n; i++)
            {
                using (Slice.External(tx.Allocator, keys.KeyPtr(i), TableData.IdLen, out Slice key))
                {
                    if (!table.ReadByKey(key, out TableValueReader row))
                        throw Missing(keys.Key(i));
                    sum = unchecked(sum + RowChecksum(in row));
                }
            }
            long e = Now();
            if (sum != expected)
                throw WrongSum(expected, sum);
            return [new Rep(W26, (ulong)n, Nanos(s, e))];
        });
    }

    /// W27: in one read transaction, from the key of each of the first `ranges` rows of order(W27), included, up to
    /// `rangeLen` rows forward by the primary key, all columns read into the checksum. W23's keys grow with the row,
    /// so the expected rows of a range are the next rows; their count and checksum come from prefix sums.
    static unsafe Plan TableRanges(int n, int ranges, int rangeLen)
    {
        uint[] order = TableData.Order(W27, (ulong)n);
        var starts = new TableKeys(order.AsSpan(0, ranges));
        var prefix = new ulong[n + 1];
        for (int r = 0; r < n; r++)
            prefix[r + 1] = unchecked(prefix[r] + TableData.RowChecksum((ulong)r));
        ulong expectedCount = 0, expectedSum = 0;
        for (int i = 0; i < ranges; i++)
        {
            int from = (int)order[i], to = Math.Min(n, from + rangeLen);
            expectedCount += (ulong)(to - from);
            expectedSum = unchecked(expectedSum + prefix[to] - prefix[from]);
        }
        var digest = FeedKeys(starts, new Digest());
        digest.FeedU64(expectedCount);
        digest.FeedU64(expectedSum);
        return new Plan([(W27, digest)], (env, _) =>
        {
            long s = Now();
            using var tx = env.ReadTransaction();
            Table table = tx.OpenTable(RowsSchema, TableName) ?? throw NoTable();
            ulong sum = 0, count = 0;
            for (int i = 0; i < ranges; i++)
            {
                int read = 0;
                using (Slice.External(tx.Allocator, starts.KeyPtr(i), TableData.IdLen, out Slice key))
                {
                    foreach (var holder in table.SeekByPrimaryKey(key, 0))
                    {
                        if (read == 0 && !IdOf(in holder.Reader).SequenceEqual(starts.Key(i)))
                            throw Missing(starts.Key(i));
                        sum = unchecked(sum + RowChecksum(in holder.Reader));
                        if (++read == rangeLen)
                            break;
                    }
                }
                if (read == 0)
                    throw Missing(starts.Key(i));
                count += (ulong)read;
            }
            long e = Now();
            if (count != expectedCount)
                throw WrongCount(expectedCount, count);
            if (sum != expectedSum)
                throw WrongSum(expectedSum, sum);
            return [new Rep(W27, count, Nanos(s, e))];
        });
    }

    /// W28: in writes of `batch`, for each row in order(W28), read it by its key and store it whole with its new `n`
    /// (Voron cannot update one column). The row's other values are copied out of the page first, because Update must
    /// not read from the row it replaces.
    static unsafe Plan TableUpdates(int n, int batch)
    {
        uint[] order = TableData.Order(W28, (ulong)n);
        var keys = new TableKeys(order);
        var newN = new long[n];
        var digest = new Digest();
        for (int i = 0; i < n; i++)
        {
            newN[i] = TableData.NewN(order[i]);
            digest.Feed(keys.Key(i));
            digest.FeedU64((ulong)newN[i]);
        }
        byte[] scratch = GC.AllocateUninitializedArray<byte>(TableData.IdLen + TableData.NameLen, pinned: true);
        return new Plan([(W28, digest)], (env, _) =>
        {
            long s = Now();
            Batches(env, n, batch, (tx, from, to) =>
            {
                byte* id = (byte*)Unsafe.AsPointer(ref scratch[0]);
                byte* name = id + TableData.IdLen;
                Table table = tx.OpenTable(RowsSchema, TableName) ?? throw NoTable();
                for (int i = from; i < to; i++)
                {
                    using (Slice.External(tx.Allocator, keys.KeyPtr(i), TableData.IdLen, out Slice key))
                    {
                        if (!table.ReadByKey(key, out TableValueReader row))
                            throw Missing(keys.Key(i));
                        Buffer.MemoryCopy(row.Read(IdColumn, out int idSize), id, TableData.IdLen, idSize);
                        Buffer.MemoryCopy(row.Read(NameColumn, out int nameSize), name, TableData.NameLen, nameSize);
                        long x = *(long*)row.Read(XColumn, out int _);
                        long ts = *(long*)row.Read(TsColumn, out int _);
                        bool flag = *row.Read(FlagColumn, out int _) != 0;
                        using (table.Allocate(out TableValueBuilder builder))
                        {
                            builder.Add(id, idSize);
                            builder.Add(newN[i]);
                            builder.Add(x);
                            builder.Add(ts);
                            builder.Add(flag);
                            builder.Add(name, nameSize);
                            table.Update(row.Id, builder);
                        }
                    }
                }
            });
            return [new Rep(W28, (ulong)n, Nanos(s, Now()))];
        });
    }

    /// The checksum of a row read: every column, as TableData.Checksum.
    static unsafe ulong RowChecksum(in TableValueReader row)
    {
        byte* name = row.Read(NameColumn, out int nameSize);
        return TableData.Checksum(IdOf(in row), *(long*)row.Read(NColumn, out int _), *(long*)row.Read(XColumn, out int _),
            *(long*)row.Read(TsColumn, out int _), *row.Read(FlagColumn, out int _) != 0, new ReadOnlySpan<byte>(name, nameSize));
    }

    static unsafe ReadOnlySpan<byte> IdOf(in TableValueReader row) =>
        new(row.Read(IdColumn, out int size), size);

    /// The checksum of a row of the table of Indexed: every column, as TableData.Checksum, n and ts back to their values.
    static unsafe ulong IndexedRowChecksum(in TableValueReader row)
    {
        byte* id = row.Read(Indexed.Id, out int idSize);
        byte* name = row.Read(Indexed.Name, out int nameSize);
        return TableData.Checksum(new ReadOnlySpan<byte>(id, idSize), Indexed.Value(*(long*)row.Read(Indexed.N, out int _)),
            *(long*)row.Read(Indexed.X, out int _), Indexed.Value(*(long*)row.Read(Indexed.Ts, out int _)),
            *row.Read(Indexed.Flag, out int _) != 0, new ReadOnlySpan<byte>(name, nameSize));
    }

    static Digest FeedKeys(TableKeys keys, Digest d)
    {
        for (int i = 0; i < keys.Count; i++)
            d.Feed(keys.Key(i));
        return d;
    }

    static BenchFailure NoTable() => new($"el almacén no tiene la tabla {TableName}");

    /// The known answers of the table data at the reduced quantities, as tests/table_data.rs pins them: the checksum of
    /// row 0 and the digest of each table load.
    public static IEnumerable<(string Name, string Got)> TableSelfTest()
    {
        yield return ("table-checksum(row 0)", $"{TableData.RowChecksum(0):x16}");
        for (int load = W23; load <= W32; load++)
            yield return ($"table-digest({Id(load)})", BuildTable(load, ParamsOf(load, false)).Digests[0].Digest.Hex);
    }
}
