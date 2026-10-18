# Measures one process tree for a while (docs/PERFORMANCE.md section 2): her
# exe and every process under it (WebView2's browser, renderer, GPU and
# utility processes, the taskbar guard, a plugin's script), as they come and go.
#
#   powershell -File tools/perf/measure.ps1 -Root <pid> [-Seconds 60] [-Threads] [-Out <file.json>]
#   . tools/perf/measure.ps1              # dot-sourced: the functions only (run.ps1)
#
# Per process and for the whole tree, over the window:
#   cycles     QueryProcessCycleTime's difference, as Mcyc/s (the main number)
#   cpu        kernel + user time's difference, as ms/s and % of one core
#   wakeups    the context switches of its threads (NtQuerySystemInformation), per second
#   memory     working set and private bytes at the window's end
#   gpu        GPU engine running time (all engines, % of the window) and the
#              GPU memory at the end, from the GPUPerformanceCounters WMI classes
#              (locale-independent, unlike Get-Counter's names)
# With -Threads, each thread too: CPU time, cycles, wakeups and its name
# (GetThreadDescription: Chromium's CrRendererMain, Compositor, ...; Rust's
# from the names their threads were started with).
#
# The tree is looked at every second: a process started in the window counts
# from its start; one that ends is read from the handle kept open on it, so
# only one that both starts and ends within a second is missed.
# Windows PowerShell 5.1 or pwsh; keep this file ASCII (5.1 reads it as ANSI).
param(
    [int]$Root,
    [int]$Seconds = 60,
    [switch]$Threads,
    [string]$Out,
    [int]$EveryMs = 1000
)

$probe = @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;

namespace WakuPerf {
    public class ThreadSeen {
        public int Tid;
        public long Created;
        public bool Born;
        public long Time0, TimeLast;
        public uint Sw0, SwLast;
        public ulong Cyc0, CycLast;
        public IntPtr Handle = IntPtr.Zero;
        public string Name = "";
    }

    public class ProcSeen {
        public int Pid, Parent;
        public long Created;
        public string Name;
        public bool Born, Gone;
        public IntPtr Handle = IntPtr.Zero;
        public ulong Cyc0, CycLast;
        public long Time0, TimeLast;
        public long WorkingSet, Private;
        public int ThreadCount, Handles;
        public Dictionary<int, ThreadSeen> Threads = new Dictionary<int, ThreadSeen>();
    }

    public class Snap {
        public int Pid, Parent, NThreads, Handles;
        public long Created, Time, WorkingSet, Private;
        public string Name;
        public int[] Tids;
        public long[] TCreated, TTime;
        public uint[] TSw;
    }

    public static class Native {
        [DllImport("ntdll.dll")] public static extern int NtQuerySystemInformation(int cls, IntPtr buf, int len, out int ret);
        [DllImport("kernel32.dll", SetLastError = true)] public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
        [DllImport("kernel32.dll", SetLastError = true)] public static extern IntPtr OpenThread(uint access, bool inherit, int tid);
        [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
        [DllImport("kernel32.dll")] public static extern bool QueryProcessCycleTime(IntPtr h, out ulong cycles);
        [DllImport("kernel32.dll")] public static extern bool QueryThreadCycleTime(IntPtr h, out ulong cycles);
        [DllImport("kernel32.dll")] public static extern bool GetProcessTimes(IntPtr h, out long create, out long exit, out long kernel, out long user);
        [DllImport("kernel32.dll")] public static extern bool GetThreadTimes(IntPtr h, out long create, out long exit, out long kernel, out long user);
        [DllImport("kernel32.dll")] public static extern int GetThreadDescription(IntPtr h, out IntPtr desc);
        [DllImport("kernel32.dll")] public static extern IntPtr LocalFree(IntPtr p);
    }

    public class Tree {
        const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
        const uint THREAD_QUERY_LIMITED_INFORMATION = 0x0800;
        public int Root;
        public bool WithThreads;
        public long StartFt;
        public Stopwatch Clock = new Stopwatch();
        public double Elapsed;
        public Dictionary<int, ProcSeen> Procs = new Dictionary<int, ProcSeen>();
        public List<ProcSeen> Ended = new List<ProcSeen>();
        public int Samples;

        public Tree(int root, bool withThreads) {
            Root = root;
            WithThreads = withThreads;
        }

        // Every process of the system with its threads, in one call
        // (SystemProcessInformation; x64 layout of SYSTEM_PROCESS_INFORMATION
        // and SYSTEM_THREAD_INFORMATION).
        public static Dictionary<int, Snap> Snapshot() {
            int len = 1 << 20;
            for (;;) {
                IntPtr buf = Marshal.AllocHGlobal(len);
                try {
                    int ret;
                    int st = Native.NtQuerySystemInformation(5, buf, len, out ret);
                    if (st == unchecked((int)0xC0000004)) {
                        len = Math.Max(len * 2, ret + 65536);
                        continue;
                    }
                    if (st != 0) throw new Exception("NtQuerySystemInformation: 0x" + st.ToString("X8"));
                    return Parse(buf);
                } finally {
                    Marshal.FreeHGlobal(buf);
                }
            }
        }

        static Dictionary<int, Snap> Parse(IntPtr buf) {
            var all = new Dictionary<int, Snap>();
            long off = 0;
            for (;;) {
                IntPtr p = new IntPtr(buf.ToInt64() + off);
                int next = Marshal.ReadInt32(p, 0);
                int n = Marshal.ReadInt32(p, 4);
                var s = new Snap();
                s.NThreads = n;
                s.Created = Marshal.ReadInt64(p, 32);
                s.Time = Marshal.ReadInt64(p, 40) + Marshal.ReadInt64(p, 48);
                int nameBytes = Marshal.ReadInt16(p, 56) & 0xffff;
                IntPtr namePtr = Marshal.ReadIntPtr(p, 64);
                s.Name = namePtr == IntPtr.Zero ? "" : Marshal.PtrToStringUni(namePtr, nameBytes / 2);
                s.Pid = (int)Marshal.ReadInt64(p, 80);
                s.Parent = (int)Marshal.ReadInt64(p, 88);
                s.Handles = Marshal.ReadInt32(p, 96);
                s.WorkingSet = Marshal.ReadInt64(p, 144);
                s.Private = Marshal.ReadInt64(p, 200);
                s.Tids = new int[n];
                s.TCreated = new long[n];
                s.TTime = new long[n];
                s.TSw = new uint[n];
                for (int i = 0; i < n; i++) {
                    IntPtr t = new IntPtr(p.ToInt64() + 256 + i * 80);
                    s.TTime[i] = Marshal.ReadInt64(t, 0) + Marshal.ReadInt64(t, 8);
                    s.TCreated[i] = Marshal.ReadInt64(t, 16);
                    s.Tids[i] = (int)Marshal.ReadInt64(t, 48);
                    s.TSw[i] = (uint)Marshal.ReadInt32(t, 64);
                }
                all[s.Pid] = s;
                if (next == 0) break;
                off += next;
            }
            return all;
        }

        public void Start() {
            StartFt = DateTime.UtcNow.ToFileTimeUtc();
            Clock.Start();
            Sample();
        }

        // The tree now: the root and, again and again, any process whose
        // parent is in it and which started after that parent (a parent id
        // can be an old, reused one).
        public void Sample() {
            var all = Snapshot();
            var inTree = new Dictionary<int, Snap>();
            Snap root;
            if (all.TryGetValue(Root, out root)) inTree[Root] = root;
            bool grew = true;
            while (grew) {
                grew = false;
                foreach (var s in all.Values) {
                    if (inTree.ContainsKey(s.Pid) || s.Pid == 0) continue;
                    Snap parent;
                    if (inTree.TryGetValue(s.Parent, out parent) && s.Created >= parent.Created) {
                        inTree[s.Pid] = s;
                        grew = true;
                    }
                }
            }
            foreach (var s in inTree.Values) {
                ProcSeen ps;
                if (Procs.TryGetValue(s.Pid, out ps) && ps.Created != s.Created) {
                    Finish(ps);
                    Procs.Remove(s.Pid);
                    ps = null;
                }
                if (ps == null) {
                    ps = new ProcSeen();
                    ps.Pid = s.Pid;
                    ps.Parent = s.Parent;
                    ps.Created = s.Created;
                    ps.Name = s.Name;
                    ps.Born = s.Created > StartFt;
                    ps.Handle = Native.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, s.Pid);
                    if (!ps.Born) {
                        ps.Cyc0 = Cycles(ps.Handle);
                        ps.Time0 = s.Time;
                    }
                    Procs[s.Pid] = ps;
                }
                ps.CycLast = Math.Max(ps.CycLast, Cycles(ps.Handle));
                ps.TimeLast = s.Time;
                ps.WorkingSet = s.WorkingSet;
                ps.Private = s.Private;
                ps.ThreadCount = s.NThreads;
                ps.Handles = s.Handles;
                for (int i = 0; i < s.Tids.Length; i++) {
                    ThreadSeen ts;
                    if (ps.Threads.TryGetValue(s.Tids[i], out ts) && ts.Created != s.TCreated[i]) {
                        ps.Threads.Remove(s.Tids[i]);
                        ts = null;
                    }
                    if (ts == null) {
                        ts = new ThreadSeen();
                        ts.Tid = s.Tids[i];
                        ts.Created = s.TCreated[i];
                        ts.Born = s.TCreated[i] > StartFt;
                        if (!ts.Born) {
                            ts.Time0 = s.TTime[i];
                            ts.Sw0 = s.TSw[i];
                        }
                        if (WithThreads) {
                            ts.Handle = Native.OpenThread(THREAD_QUERY_LIMITED_INFORMATION, false, ts.Tid);
                            if (!ts.Born) ts.Cyc0 = ThreadCycles(ts.Handle);
                        }
                        ps.Threads[ts.Tid] = ts;
                    }
                    ts.TimeLast = s.TTime[i];
                    ts.SwLast = s.TSw[i];
                    if (WithThreads) ts.CycLast = Math.Max(ts.CycLast, ThreadCycles(ts.Handle));
                }
            }
            foreach (var ps in new List<ProcSeen>(Procs.Values)) {
                if (!inTree.ContainsKey(ps.Pid)) {
                    Finish(ps);
                    Procs.Remove(ps.Pid);
                }
            }
            Samples++;
        }

        // A process gone from the tree: its last numbers from the handle
        // kept on it (an ended process can still be asked).
        void Finish(ProcSeen ps) {
            ps.Gone = true;
            if (ps.Handle != IntPtr.Zero) {
                ps.CycLast = Math.Max(ps.CycLast, Cycles(ps.Handle));
                long c, e, k, u;
                if (Native.GetProcessTimes(ps.Handle, out c, out e, out k, out u)) ps.TimeLast = Math.Max(ps.TimeLast, k + u);
            }
            ps.WorkingSet = 0;
            ps.Private = 0;
            Ended.Add(ps);
        }

        public void Run(int ms, int every) {
            var until = Clock.ElapsedMilliseconds + ms;
            for (;;) {
                long left = until - Clock.ElapsedMilliseconds;
                if (left <= 0) break;
                System.Threading.Thread.Sleep((int)Math.Min(left, every));
                Sample();
            }
        }

        public void Stop() {
            Sample();
            Clock.Stop();
            Elapsed = Clock.Elapsed.TotalSeconds;
            if (WithThreads) {
                foreach (var ps in All()) {
                    foreach (var ts in ps.Threads.Values) ts.Name = ThreadName(ts.Handle);
                }
            }
        }

        public List<ProcSeen> All() {
            var list = new List<ProcSeen>(Ended);
            list.AddRange(Procs.Values);
            return list;
        }

        public void Close() {
            foreach (var ps in All()) {
                if (ps.Handle != IntPtr.Zero) Native.CloseHandle(ps.Handle);
                ps.Handle = IntPtr.Zero;
                foreach (var ts in ps.Threads.Values) {
                    if (ts.Handle != IntPtr.Zero) Native.CloseHandle(ts.Handle);
                    ts.Handle = IntPtr.Zero;
                }
            }
        }

        static ulong Cycles(IntPtr h) {
            ulong c;
            return h != IntPtr.Zero && Native.QueryProcessCycleTime(h, out c) ? c : 0;
        }

        static ulong ThreadCycles(IntPtr h) {
            ulong c;
            return h != IntPtr.Zero && Native.QueryThreadCycleTime(h, out c) ? c : 0;
        }

        static string ThreadName(IntPtr h) {
            if (h == IntPtr.Zero) return "";
            IntPtr p;
            try {
                if (Native.GetThreadDescription(h, out p) < 0 || p == IntPtr.Zero) return "";
            } catch (EntryPointNotFoundException) {
                return "";
            }
            string name = Marshal.PtrToStringUni(p);
            Native.LocalFree(p);
            return name ?? "";
        }
    }
}
'@

function Initialize-WakuPerf {
    if (-not ('WakuPerf.Tree' -as [type])) { Add-Type -TypeDefinition $probe -Language CSharp }
}

# What a process of the tree is, from its command line.
function Get-PerfKind([string]$name, [string]$cmd) {
    if ($name -like 'msedgewebview2*') {
        if ($cmd -match '--type=renderer') { return 'renderer' }
        if ($cmd -match '--type=gpu-process') { return 'gpu' }
        if ($cmd -match '--utility-sub-type=([\w.]+)') { return 'utility:' + ($Matches[1] -replace '\.mojom\..*', '') }
        if ($cmd -match '--type=crashpad-handler') { return 'crashpad' }
        if ($cmd -match '--type=([\w-]+)') { return $Matches[1] }
        return 'browser'
    }
    if ($name -like 'wakuwaku*') {
        if ($cmd -match '--taskbar-guard') { return 'guard' }
        if ($cmd -match ' --([\w-]+)') { return 'exe:' + $Matches[1] }
        return 'exe'
    }
    if ($name -match '^(pwsh|powershell)') {
        if ($cmd -match '([\w-]+)\.ps1') { return 'script:' + $Matches[1] }
        return 'script'
    }
    return $name -replace '\.exe$', ''
}

# GPU engines' running time (100 ns, cumulative) and GPU memory, per process id.
function Get-PerfGpu {
    $engines = @{}
    $memory = @{}
    try {
        foreach ($e in Get-CimInstance -Query 'SELECT Name,RunningTime FROM Win32_PerfRawData_GPUPerformanceCounters_GPUEngine' -ErrorAction Stop) {
            if ($e.Name -match '^pid_(\d+)_.*engtype_(.*)$') {
                $key = $Matches[1] + '|' + $Matches[2]
                $engines[$key] = [double]$engines[$key] + [double]$e.RunningTime
            }
        }
        foreach ($m in Get-CimInstance -Query 'SELECT Name,DedicatedUsage,SharedUsage FROM Win32_PerfRawData_GPUPerformanceCounters_GPUProcessMemory' -ErrorAction Stop) {
            if ($m.Name -match '^pid_(\d+)_') {
                $pid0 = [int]$Matches[1]
                $was = $memory[$pid0]
                if (-not $was) { $was = @{ dedicated = 0.0; shared = 0.0 } }
                $memory[$pid0] = @{ dedicated = $was.dedicated + [double]$m.DedicatedUsage; shared = $was.shared + [double]$m.SharedUsage }
            }
        }
    } catch {
        return $null
    }
    @{ engines = $engines; memory = $memory }
}

# The window begins: the tree as it is, the GPU's counters.
function Start-PerfTree([int]$Root, [switch]$Threads) {
    Initialize-WakuPerf
    $gpu = Get-PerfGpu
    $tree = New-Object WakuPerf.Tree($Root, [bool]$Threads)
    $tree.Start()
    [pscustomobject]@{ tree = $tree; gpu0 = $gpu; threads = [bool]$Threads }
}

# Looks at the tree every $EveryMs for $Ms.
function Wait-PerfTree($m, [int]$Ms, [int]$EveryMs = 1000) { $m.tree.Run($Ms, $EveryMs) }

# The window ends: the numbers, as a hashtable ready for ConvertTo-Json.
function Stop-PerfTree($m) {
    $tree = $m.tree
    $tree.Stop()
    $gpu1 = Get-PerfGpu
    $secs = $tree.Elapsed
    $all = @($tree.All())
    $cmds = @{}
    foreach ($p in Get-CimInstance Win32_Process -ErrorAction SilentlyContinue) { $cmds[[int]$p.ProcessId] = [string]$p.CommandLine }
    $procs = @()
    $threads = @()
    $tot = @{ cyc = 0.0; time = 0.0; sw = 0.0; ws = 0.0; priv = 0.0; gpu = 0.0; dedicated = 0.0; shared = 0.0 }
    $byEngine = @{}
    foreach ($p in $all) {
        $cyc = [double]$p.CycLast - [double]$p.Cyc0
        $time = [double]($p.TimeLast - $p.Time0)
        $sw = 0.0
        foreach ($t in $p.Threads.Values) {
            $tsw = [double]$t.SwLast - [double]$t.Sw0
            $sw += $tsw
            if ($m.threads) {
                $ttime = [double]($t.TimeLast - $t.Time0)
                $tcyc = [double]$t.CycLast - [double]$t.Cyc0
                if ($ttime -gt 0 -or $tsw -gt 0) {
                    $threads += [ordered]@{
                        pid = $p.Pid; tid = $t.Tid; name = $t.Name; born = $t.Born
                        msPerSec = [math]::Round($ttime / 1e4 / $secs, 3)
                        mcycPerSec = [math]::Round($tcyc / 1e6 / $secs, 3)
                        wakeupsPerSec = [math]::Round($tsw / $secs, 2)
                    }
                }
            }
        }
        $gpuTime = 0.0
        if ($m.gpu0 -and $gpu1) {
            foreach ($k in @($gpu1.engines.Keys)) {
                $parts = $k -split '\|', 2
                if ([int]$parts[0] -ne $p.Pid) { continue }
                $d = [double]$gpu1.engines[$k] - [double]$m.gpu0.engines[$k]
                if ($d -gt 0) {
                    $gpuTime += $d
                    $byEngine[$parts[1]] = [double]$byEngine[$parts[1]] + $d
                }
            }
        }
        $mem = $null
        if ($gpu1 -and -not $p.Gone) { $mem = $gpu1.memory[$p.Pid] }
        $cmd = [string]$cmds[$p.Pid]
        $procs += [ordered]@{
            pid = $p.Pid; parent = $p.Parent; name = $p.Name; kind = (Get-PerfKind $p.Name $cmd); born = $p.Born; gone = $p.Gone
            mcycPerSec = [math]::Round($cyc / 1e6 / $secs, 3)
            msPerSec = [math]::Round($time / 1e4 / $secs, 3)
            wakeupsPerSec = [math]::Round($sw / $secs, 2)
            workingSetMB = [math]::Round($p.WorkingSet / 1MB, 1)
            privateMB = [math]::Round($p.Private / 1MB, 1)
            threads = $p.ThreadCount
            gpuPct = [math]::Round($gpuTime / 1e5 / $secs, 3)
            gpuDedicatedMB = $(if ($mem) { [math]::Round($mem.dedicated / 1MB, 1) } else { 0 })
        }
        $tot.cyc += $cyc; $tot.time += $time; $tot.sw += $sw; $tot.ws += $p.WorkingSet; $tot.priv += $p.Private; $tot.gpu += $gpuTime
        if ($mem) { $tot.dedicated += $mem.dedicated; $tot.shared += $mem.shared }
    }
    $tree.Close()
    $engines = [ordered]@{}
    foreach ($k in $byEngine.Keys) { $engines[$k] = [math]::Round($byEngine[$k] / 1e5 / $secs, 3) }
    $ms = $tot.time / 1e4 / $secs
    $out = [ordered]@{
        root = $tree.Root
        seconds = [math]::Round($secs, 2)
        samples = $tree.Samples
        totals = [ordered]@{
            mcycPerSec = [math]::Round($tot.cyc / 1e6 / $secs, 3)
            msPerSec = [math]::Round($ms, 3)
            corePct = [math]::Round($ms / 10, 3)
            wakeupsPerSec = [math]::Round($tot.sw / $secs, 1)
            workingSetMB = [math]::Round($tot.ws / 1MB, 1)
            privateMB = [math]::Round($tot.priv / 1MB, 1)
            gpuPct = $(if ($m.gpu0 -and $gpu1) { [math]::Round($tot.gpu / 1e5 / $secs, 3) } else { $null })
            gpuDedicatedMB = [math]::Round($tot.dedicated / 1MB, 1)
            gpuSharedMB = [math]::Round($tot.shared / 1MB, 1)
            processes = @($all | Where-Object { -not $_.Gone }).Count
        }
        gpuEngines = $engines
        processes = @($procs | Sort-Object { - $_.mcycPerSec })
    }
    if ($m.threads) { $out.threads = @($threads | Sort-Object { - $_.mcycPerSec }) }
    $out
}

function Measure-PerfTree([int]$Root, [int]$Seconds = 60, [switch]$Threads, [int]$EveryMs = 1000) {
    $m = Start-PerfTree -Root $Root -Threads:$Threads
    Wait-PerfTree $m ($Seconds * 1000) $EveryMs
    Stop-PerfTree $m
}

# Dot-sourced (run.ps1): the functions only.
if ($MyInvocation.InvocationName -eq '.') { return }
if (-not $Root) { throw 'measure.ps1 -Root <pid> [-Seconds 60] [-Threads] [-Out <file.json>]' }
if (-not (Get-Process -Id $Root -ErrorAction SilentlyContinue)) { throw "no process $Root" }
$result = Measure-PerfTree -Root $Root -Seconds $Seconds -Threads:$Threads -EveryMs $EveryMs
$json = $result | ConvertTo-Json -Depth 8
if ($Out) {
    [IO.File]::WriteAllText([IO.Path]::GetFullPath($Out), $json, (New-Object Text.UTF8Encoding $false))
} else {
    $json
}
