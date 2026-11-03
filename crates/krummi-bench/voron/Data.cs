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
    /// Rust loop does with its vector instructions.
    public static ulong ByteSum(ReadOnlySpan<byte> bytes)
    {
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
