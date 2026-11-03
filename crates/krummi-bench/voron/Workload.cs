// The loads and their quantities: a port of crates/krummi-bench/src/workload.rs (workload table, K8, K11). A load is
// its number: 1 for W1.

using System;

namespace VoronBench;

enum Start { Empty, Base, After }

/// The quantities of a load at a scale. Fields a load does not use are 0.
readonly record struct Params(
    ulong Entries = 0,
    ulong Batch = 0,
    ulong Threads = 1,
    int ValueLen = Workload.ValueLen,
    ulong Ranges = 0,
    ulong RangeLen = 0,
    ulong Writes = 0,
    ulong ReadsPerTransaction = 0,
    ulong Opens = 0,
    ulong Keys = 0,
    ulong ValuesPerKey = 0);

static class Workload
{
    public const int ValueLen = 100;
    public const ulong BatchSize = 1_000;

    public const int W1 = 1, W2 = 2, W3 = 3, W4 = 4, W5 = 5, W6 = 6, W7 = 7, W8 = 8, W9 = 9, W10 = 10, W11 = 11,
        W12 = 12, W13 = 13, W14 = 14, W15 = 15, W16 = 16, W17 = 17, W18 = 18, W19 = 19, W20 = 20, W21 = 21, W22 = 22;

    public static string Id(int load) => "W" + load;

    public static int? FromId(string id)
    {
        for (int l = W1; l <= W22; l++)
            if (Id(l) == id)
                return l;
        return null;
    }

    public static Start StartOf(int load) => load switch
    {
        W1 or W2 or W3 or W17 or W19 or W21 => Start.Empty,
        W18 or W20 or W22 => Start.After,
        _ => Start.Base,
    };

    /// The loads that only read (K4): their process opens the store once and measures with warm caches.
    public static bool ReadOnly(int load) => load is W7 or W8 or W9 or W10 or W11 or W18 or W20 or W22;

    /// Repetitions without measuring (the warm-up) and measured ones (K8, K11).
    public static (int Warmups, int Measured) Repetitions(bool full) => full ? (1, 3) : (0, 1);

    /// Entries of the base store (K4).
    public static ulong BaseEntries(bool full) => full ? 1_000_000UL : 10_000UL;

    public static Params ParamsOf(int load, bool full)
    {
        ulong Pick(ulong f, ulong r) => full ? f : r;
        ulong n = BaseEntries(full);
        return load switch
        {
            W1 or W2 or W14 or W15 => new Params(Entries: n, Batch: BatchSize),
            W3 => new Params(Entries: n, Batch: n),
            W4 or W5 or W6 => new Params(Entries: Pick(10_000, 160), Batch: 1, Threads: load switch { W4 => 1UL, W5 => 4UL, _ => 16UL }),
            W7 or W8 or W9 or W10 => new Params(Entries: n, Threads: load switch { W8 => 4UL, W9 => 16UL, _ => 1UL }),
            W11 => new Params(Ranges: Pick(100_000, 1_000), RangeLen: 100),
            W12 or W13 => new Params(Threads: 4, Writes: Pick(1_000, 10), Batch: 100, ReadsPerTransaction: 1_000),
            W16 => new Params(Opens: Pick(100, 10)),
            W17 or W18 => new Params(Entries: Pick(4_096, 64), Batch: 256, ValueLen: 64 * 1024),
            W19 or W20 => new Params(Keys: Pick(100_000, 1_000), ValuesPerKey: 10, Batch: BatchSize, ValueLen: 16),
            W21 or W22 => new Params(Entries: n, Batch: BatchSize, ValueLen: 8),
            _ => throw new ArgumentOutOfRangeException(nameof(load)),
        };
    }
}
