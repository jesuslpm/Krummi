// The data of every load: a port of crates/krummi-bench/src/data.rs (K4, design D5). Both must produce the same bytes;
// the orchestrator compares the digest of what every engine received (AC-K7), and `selftest` prints known answers.

using System;
using System.Buffers.Binary;
using System.Runtime.InteropServices;
using System.Runtime.Intrinsics;
using System.Runtime.Intrinsics.X86;

namespace VoronBench;

static class Data
{
    public const int KeyLen = 16;

    /// The SplitMix64 mixing function, a bijection of u64.
    public static ulong Splitmix(ulong z)
    {
        unchecked
        {
            z += 0x9E3779B97F4A7C15UL;
            z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9UL;
            z = (z ^ (z >> 27)) * 0x94D049BB133111EBUL;
            return z ^ (z >> 31);
        }
    }

    /// The first index of the new keys of a load (bits 40 and up hold the load's number).
    public static ulong IndexBase(int load) => (ulong)load << 40;

    /// Writes the 16-byte key of index i: splitmix(i ^ 0x5EED0001) and splitmix(!i), both big-endian.
    public static void KeyInto(ulong i, Span<byte> out16)
    {
        BinaryPrimitives.WriteUInt64BigEndian(out16, Splitmix(i ^ 0x5EED0001UL));
        BinaryPrimitives.WriteUInt64BigEndian(out16.Slice(8), Splitmix(~i));
    }

    public static byte[] Key(ulong i)
    {
        var k = new byte[KeyLen];
        KeyInto(i, k);
        return k;
    }

    /// Fills `output` with the value of index i: the big-endian bytes of splitmix((i * 0x10000 + j) ^
    /// 0xBA5E000000000000) for j = 0, 1, ..., the last word truncated.
    public static void ValueInto(ulong i, Span<byte> output)
    {
        Span<byte> word = stackalloc byte[8];
        for (int j = 0, pos = 0; pos < output.Length; j++, pos += 8)
        {
            ulong w = Splitmix(unchecked(i * 0x10000UL + (ulong)j) ^ 0xBA5E000000000000UL);
            BinaryPrimitives.WriteUInt64BigEndian(word, w);
            int n = Math.Min(8, output.Length - pos);
            word.Slice(0, n).CopyTo(output.Slice(pos));
        }
    }

    public static byte[] Value(ulong i, int len)
    {
        var v = new byte[len];
        ValueInto(i, v);
        return v;
    }

    /// The integer key of index i of a fixed-size tree (W21, W22).
    public static ulong FixedKey(ulong i) => Splitmix(i ^ 0xF1DE0000UL);

    /// The seed of the random order of `thread` in a load.
    public static ulong Seed(int load, ulong thread) => Splitmix(((ulong)load << 32) | thread);

    /// The indices 0..n in the random order of `seed`: Fisher-Yates from the last position down.
    public static uint[] Permutation(ulong n, ulong seed)
    {
        var order = new uint[n];
        for (uint i = 0; i < n; i++)
            order[i] = i;
        ulong state = seed;
        for (long i = (long)n - 1; i >= 1; i--)
        {
            state = Splitmix(state);
            long j = (long)(state % (ulong)(i + 1));
            (order[i], order[j]) = (order[j], order[i]);
        }
        return order;
    }

    /// The order in which `thread` of a reading load visits the n base indices.
    public static uint[] ReadOrder(int load, ulong thread, ulong n)
    {
        if (thread != 0)
            return Permutation(n, Seed(load, thread));
        var order = new uint[n];
        for (uint i = 0; i < n; i++)
            order[i] = i;
        return order;
    }

    /// The sum of the bytes of `bytes`, the checksum of a read (K8). SSE2 sums 16 bytes per step, as the compiled
    /// Rust loop does with its vector instructions. Up to 16 bytes, the most a key or a column of the table loads
    /// has, it takes a short path as data.rs does (M2 of the 2026-11-15 performance diagnostic): one SSE2 step for
    /// exactly 16 bytes, which Voron already took, and a scalar sum below.
    public static ulong ByteSum(ReadOnlySpan<byte> bytes)
    {
        if (bytes.Length == 16 && Sse2.IsSupported)
        {
            ref byte first = ref MemoryMarshal.GetReference(bytes);
            var sums = Sse2.SumAbsoluteDifferences(Vector128.LoadUnsafe(ref first), Vector128<byte>.Zero).AsUInt64();
            return sums.GetElement(0) + sums.GetElement(1);
        }
        if (bytes.Length <= 16)
        {
            // At most 16 × 255, far from the largest uint.
            uint shortSum = 0;
            foreach (byte b in bytes)
                shortSum += b;
            return shortSum;
        }
        ulong sum = 0;
        int i = 0;
        if (Sse2.IsSupported && bytes.Length >= 16)
        {
            ref byte start = ref MemoryMarshal.GetReference(bytes);
            var acc = Vector128<ulong>.Zero;
            for (; i + 16 <= bytes.Length; i += 16)
            {
                var v = Vector128.LoadUnsafe(ref start, (nuint)i);
                acc = Sse2.Add(acc, Sse2.SumAbsoluteDifferences(v, Vector128<byte>.Zero).AsUInt64());
            }
            sum = acc.GetElement(0) + acc.GetElement(1);
        }
        for (; i < bytes.Length; i++)
            sum += bytes[i];
        return sum;
    }

    public static unsafe ulong ByteSum(byte* p, int len) => ByteSum(new ReadOnlySpan<byte>(p, len));
}

/// The digest of the data an engine received for a load (AC-K7), as data.rs: every 8-byte chunk (the last padded
/// with zeros) read little-endian and mixed as h = splitmix(h ^ word), then h = splitmix(h ^ len).
sealed class Digest
{
    ulong _h = 0x6B72756D6D690006UL;

    public void Feed(ReadOnlySpan<byte> item)
    {
        Span<byte> word = stackalloc byte[8];
        for (int pos = 0; pos < item.Length; pos += 8)
        {
            int n = Math.Min(8, item.Length - pos);
            word.Clear();
            item.Slice(pos, n).CopyTo(word);
            _h = Data.Splitmix(_h ^ BinaryPrimitives.ReadUInt64LittleEndian(word));
        }
        _h = Data.Splitmix(_h ^ (ulong)item.Length);
    }

    public void FeedU64(ulong value)
    {
        Span<byte> b = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(b, value);
        Feed(b);
    }

    public ulong Value => _h;

    public string Hex => _h.ToString("x16");
}

/// The data of the table loads W23-W29: a port of crates/krummi-bench/src/table_data.rs, whose module comment gives
/// every column, key and order. Row r takes its values from Word(r, j) = Splitmix((r * 0x10000 + j) ^ Salt).
static class TableData
{
    public const int IdLen = 16, NameLen = 75;
    /// 2000-01-01T00:00:00Z and 2030-01-01T00:00:00Z (excluded) in microseconds since the Unix epoch.
    public const long TsMin = 946_684_800_000_000, TsMax = 1_893_456_000_000_000;
    /// The millisecond of the ulid of row 0: 2026-01-01T00:00:00Z.
    public const ulong UlidMs0 = 1_767_225_600_000;
    const ulong Salt = 0x7AB1E00000000000UL;
    const ulong NameWord = 4, UlidWord = 79, UuidWord = 81, NewNWord = 83;

    /// The characters of `name`, in ASCII order.
    static ReadOnlySpan<byte> NameAlphabet => "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"u8;

    static ulong Word(ulong r, ulong j) => Data.Splitmix(unchecked(r * 0x10000UL + j) ^ Salt);

    /// The high 64 bits of a * b: uniform in 0..b when a is uniform over every ulong.
    static ulong Hi(ulong a, ulong b) => Math.BigMul(a, b, out _);

    public static long N(ulong r) => unchecked((long)Word(r, 0));

    /// x in [0, 1), as its IEEE bits: (word >> 11) * 2^-53, exact.
    public static long XBits(ulong r) => BitConverter.DoubleToInt64Bits((Word(r, 1) >> 11) * (1.0 / (1UL << 53)));

    public static long Ts(ulong r) => TsMin + (long)Hi(Word(r, 2), (ulong)(TsMax - TsMin));

    public static bool Flag(ulong r) => (Word(r, 3) & 1) == 1;

    public static void NameInto(ulong r, Span<byte> out75)
    {
        for (int k = 0; k < NameLen; k++)
            out75[k] = NameAlphabet[(int)Hi(Word(r, NameWord + (ulong)k), (ulong)NameAlphabet.Length)];
    }

    /// The key of row r: E15's ulid layout (millisecond UlidMs0 + r, then 80 bits), or a version 4 uuid in RFC 9562
    /// byte order.
    public static void KeyInto(ulong r, bool uuid, Span<byte> out16)
    {
        if (uuid)
        {
            BinaryPrimitives.WriteUInt64BigEndian(out16, Word(r, UuidWord));
            BinaryPrimitives.WriteUInt64BigEndian(out16.Slice(8), Word(r, UuidWord + 1));
            out16[6] = (byte)((out16[6] & 0x0F) | 0x40);
            out16[8] = (byte)((out16[8] & 0x3F) | 0x80);
        }
        else
        {
            Span<byte> ms = stackalloc byte[8];
            BinaryPrimitives.WriteUInt64BigEndian(ms, UlidMs0 + r);
            ms.Slice(2).CopyTo(out16);
            BinaryPrimitives.WriteUInt16BigEndian(out16.Slice(6), (ushort)Word(r, UlidWord));
            BinaryPrimitives.WriteUInt64BigEndian(out16.Slice(8), Word(r, UlidWord + 1));
        }
    }

    /// The new `n` W28 gives row r; never the old one.
    public static long NewN(ulong r) => unchecked((long)Word(r, NewNWord));

    /// The checksum of a row read, the same in both engines: the wrapping sum of the bytes of id, n, the bits of x,
    /// ts, flag as 0 or 1 and the bytes of name.
    public static ulong Checksum(ReadOnlySpan<byte> id, long n, long xBits, long ts, bool flag, ReadOnlySpan<byte> name) =>
        unchecked(Data.ByteSum(id) + (ulong)n + (ulong)xBits + (ulong)ts + (flag ? 1UL : 0UL) + Data.ByteSum(name));

    /// The checksum of row r of W23's table.
    public static ulong RowChecksum(ulong r)
    {
        Span<byte> id = stackalloc byte[IdLen];
        Span<byte> name = stackalloc byte[NameLen];
        KeyInto(r, false, id);
        NameInto(r, name);
        return Checksum(id, N(r), XBits(r), Ts(r), Flag(r), name);
    }

    /// The rows 0..n of W23's table in the random order of `load` (W26-W29).
    public static uint[] Order(int load, ulong n) => Data.Permutation(n, Data.Seed(load, 0));
}
