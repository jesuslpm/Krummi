// The Voron worker of the benchmark: the protocol of crates/krummi-bench/src/protocol.rs.
//
//   run voron <load> <scale> <store> [<source>]   runs a load and prints `data` and `rep` lines
//   prepare voron <scale> <store>                 builds the base store and closes it
//   versions                                      prints `versions dotnet <x.y.z>`
//   selftest                                      prints known answers of the data functions, to compare with Rust
//
// On success only protocol lines go to standard output and the exit code is 0; on failure one line with the cause
// goes to standard error and the exit code is 1. KRUMMI_BENCH_FAIL=voron:<load>:<kind> simulates a failure (AC-K10).

using System;
using System.IO;
using System.Linq;
using System.Text;

namespace VoronBench;

/// A failure the worker detects itself (a missing key, a wrong count or sum, a wrong argument).
sealed class BenchFailure(string cause) : Exception(cause);

static class Program
{
    public const string FailVariable = "KRUMMI_BENCH_FAIL";

    static int Main(string[] args)
    {
        // UTF-8 without a byte order mark whatever the console's code page, as the Rust side reads it.
        var utf8 = new UTF8Encoding(false);
        Console.SetOut(new StreamWriter(Console.OpenStandardOutput(), utf8) { AutoFlush = true });
        Console.SetError(new StreamWriter(Console.OpenStandardError(), utf8) { AutoFlush = true });
        try
        {
            Execute(args);
            Console.Out.Flush();
            return 0;
        }
        catch (Exception e)
        {
            Console.Out.Flush();
            Console.Error.WriteLine(Cause(e));
            return 1;
        }
    }

    static void Execute(string[] args)
    {
        switch (args)
        {
            case ["versions"]:
                Console.Out.WriteLine($"versions dotnet {Environment.Version}");
                break;
            case ["selftest"]:
                SelfTest();
                break;
            case ["prepare", "voron", var scale, var store]:
                Loads.Prepare(ParseScale(scale), NewStore(store));
                break;
            case ["run", "voron", var load, var scale, var store, .. var rest] when rest.Length <= 1:
                int l = Workload.FromId(load) is int n && n != Workload.W13
                    ? n
                    : throw new BenchFailure($"carga no válida: {load}");
                Loads.Run(l, ParseScale(scale), store, rest.Length == 1 ? rest[0] : null);
                break;
            default:
                throw new BenchFailure($"orden no válida: {string.Join(' ', args)}");
        }
    }

    static bool ParseScale(string scale) => scale switch
    {
        "full" => true,
        "reduced" => false,
        _ => throw new BenchFailure($"escala no válida: {scale}"),
    };

    /// Creates the store folder, which must not exist yet.
    public static string NewStore(string store)
    {
        if (Directory.Exists(store) || File.Exists(store))
            throw new BenchFailure($"la carpeta {store} ya existe");
        Directory.CreateDirectory(store);
        return store;
    }

    /// The single-line cause of a failure: the text of the error, with its line breaks turned into spaces.
    static string Cause(Exception e)
    {
        if (e is AggregateException { InnerExceptions.Count: 1 } a)
            e = a.InnerExceptions[0];
        string text = string.IsNullOrWhiteSpace(e.Message) ? e.GetType().Name : e.Message;
        return string.Join(' ', text.Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries).Select(s => s.Trim()));
    }

    /// Makes the load fail as KRUMMI_BENCH_FAIL says, when it names this engine and `load` (AC-K10). W12's process also
    /// fails for W13, the load it measures with W12.
    public static void MaybeFail(int load)
    {
        string spec = Environment.GetEnvironmentVariable(FailVariable);
        if (spec == null)
            return;
        string[] parts = spec.Split(':');
        if (parts.Length != 3 || parts[0] != "voron" || Workload.FromId(parts[1]) is not int target)
            return;
        if (target != load && !(load == Workload.W12 && target == Workload.W13))
            return;
        switch (parts[2])
        {
            case "error":
                throw new InvalidOperationException("error simulado del motor");
            case "missing":
                throw Loads.Missing(Data.Key(ulong.MaxValue));
            case "checksum":
                throw Loads.WrongSum(0, 1);
            case "exit":
                Console.Out.Flush();
                Environment.Exit(3);
                break;
        }
    }

    static void SelfTest()
    {
        Console.Out.WriteLine($"splitmix(0) {Data.Splitmix(0):x16}");
        Console.Out.WriteLine($"splitmix(1) {Data.Splitmix(1):x16}");
        Console.Out.WriteLine($"permutation(8,42) {string.Join(' ', Data.Permutation(8, 42))}");
        var d = new Digest();
        d.Feed(Data.Key(0));
        d.Feed(Data.Value(3, 100));
        Console.Out.WriteLine($"digest(key(0),value(3,100)) {d.Hex}");
    }
}
