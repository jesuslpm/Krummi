// The `cores` command (B4 of the Krummi and Voron only PRD): one `core <id> <class>` line per logical core of the
// machine, a higher class being a faster core. Windows gives the `EfficiencyClass` of its CPU sets; Linux, the
// `cpu_capacity` of each online core under /sys. It prints nothing when the system gives no class, and fails when it
// cannot be asked or gives a class to only some cores. The orchestrator limits the engine processes to the cores of
// the fastest class when there is more than one (crates/krummi-bench/src/orchestrator/cores.rs).

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;

namespace VoronBench;

static class Cores
{
    public static void Print()
    {
        List<(int Core, int Class)> classes;
        if (OperatingSystem.IsWindows())
            classes = Windows();
        else if (OperatingSystem.IsLinux())
            classes = Linux();
        else
            classes = [];
        foreach (var (core, cls) in classes)
            Console.Out.WriteLine($"core {core} {cls}");
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetSystemCpuSetInformation(
        IntPtr information, uint bufferLength, out uint returnedLength, IntPtr process, uint flags);

    // SYSTEM_CPU_SET_INFORMATION: Size (DWORD) at 0, Type (0 = CpuSetInformation) at 4, then the CpuSet: Id at 8,
    // Group (WORD) at 12, LogicalProcessorIndex at 14, CoreIndex at 15, LastLevelCacheIndex at 16, NumaNodeIndex at
    // 17 and EfficiencyClass at 18 (bytes).
    static List<(int, int)> Windows()
    {
        GetSystemCpuSetInformation(IntPtr.Zero, 0, out uint length, IntPtr.Zero, 0);
        if (length == 0)
            throw new BenchFailure($"GetSystemCpuSetInformation falló ({Marshal.GetLastWin32Error()})");
        IntPtr buffer = Marshal.AllocHGlobal((int)length);
        try
        {
            if (!GetSystemCpuSetInformation(buffer, length, out length, IntPtr.Zero, 0))
                throw new BenchFailure($"GetSystemCpuSetInformation falló ({Marshal.GetLastWin32Error()})");
            var classes = new List<(int, int)>();
            for (int offset = 0; offset < length;)
            {
                int size = Marshal.ReadInt32(buffer, offset);
                if (size <= 0)
                    throw new BenchFailure("GetSystemCpuSetInformation dio una entrada vacía");
                if (Marshal.ReadInt32(buffer, offset + 4) == 0)
                {
                    int group = (ushort)Marshal.ReadInt16(buffer, offset + 12);
                    int index = Marshal.ReadByte(buffer, offset + 14);
                    classes.Add((group * 64 + index, Marshal.ReadByte(buffer, offset + 18)));
                }
                offset += size;
            }
            return classes;
        }
        finally
        {
            Marshal.FreeHGlobal(buffer);
        }
    }

    static List<(int, int)> Linux()
    {
        const string root = "/sys/devices/system/cpu";
        var cores = Directory.GetDirectories(root, "cpu*")
            .Select(Path.GetFileName)
            .Where(n => n.Length > 3 && n[3..].All(char.IsAsciiDigit))
            .Select(n => int.Parse(n[3..]))
            .Where(c => !File.Exists($"{root}/cpu{c}/online") || File.ReadAllText($"{root}/cpu{c}/online").Trim() != "0")
            .Order()
            .ToList();
        var classes = new List<(int, int)>();
        foreach (int core in cores)
        {
            string capacity = $"{root}/cpu{core}/cpu_capacity";
            if (File.Exists(capacity))
                classes.Add((core, int.Parse(File.ReadAllText(capacity).Trim())));
        }
        if (classes.Count != 0 && classes.Count != cores.Count)
            throw new BenchFailure("solo algunos núcleos dan su cpu_capacity");
        return classes;
    }
}
