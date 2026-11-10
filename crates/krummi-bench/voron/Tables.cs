// The table loads W23-W29 on Voron's Table API (E21 and *Cargas del banco* of docs/prd/krummi/tables-schema.prd.md).
// The data, the orders and the expected checksums come from TableData (Data.cs), a port of
// crates/krummi-bench/src/table_data.rs; the digests are those of `table_data::digest`. Every load generates them before
// any timer starts.
//
// The table: a TableSchema with the primary key on column 0 (`id`, 16 bytes) and the columns stored as Voron table
// values, in schema order: id (16 bytes), n (long), x (its IEEE bits as a long), ts (microseconds, long), flag (bool,
// one byte) and name (75 ASCII bytes). It is created, as RavenDB creates its collections (16 pages per raw data
// section), inside each write of the inserting loads, where it is a no-op once it exists. Every commit is durable, as
// in the other loads (K3).

using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Voron;
using Voron.Data.Tables;
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
        for (int load = W23; load <= W29; load++)
            yield return ($"table-digest({Id(load)})", BuildTable(load, ParamsOf(load, false)).Digests[0].Digest.Hex);
    }
}
