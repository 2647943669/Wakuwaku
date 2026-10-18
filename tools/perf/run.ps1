# A performance run, A against B (docs/PERFORMANCE.md section 4): each scene
# (configs/<scene>.json) with a fresh copy of each exe, A B A B ..., each
# measured for a minute after a minute's warm-up, then the medians compared
# (report.js).
#
#   powershell -File tools/perf/run.ps1 -A <exe> -B <exe> [-Scenes S1,S2] [-Runs 3] [-Out <folder>]
#       on the test machine itself
#   pwsh tools/perf/run.ps1 -A <exe> -B <exe> ... -Vm <name> -VmHelpers <vm helpers .ps1>
#       from the host: the exes and these scripts copied into the test VM,
#       the run started there on its own, followed until it ends, its
#       results fetched into -Out and the table made here
#   pwsh tools/perf/run.ps1 -Vm <name> -VmHelpers <...> -Collect -Out <folder>
#       a run in the VM followed again (or only fetched, once it is over)
#
# -VmHelpers (or WAKU_PERF_VM_HELPERS): a script that, dot-sourced, gives
# Use-WakuVM <name>, VM '<powershell>' (run in the VM's desktop session, as
# its user), VMPut <local> <remote> and VMGet <remote> <local>.
# -LabelA/-LabelB, -CommitA/-CommitB: what each side is, for the table.
# -Threads: every run broken down per thread too (bigger files).
# -Starts n: S7's cold and warm starts n each, not the scene's (a quick look).
# -FreshWebView: each run on a new WebView2 folder, not the side's primed one
# (see Initialize-PerfWebView).
#
# Each copy: its own data folder (WAKUWAKU_USER_DATA, with the scene's
# config.json written first; WebView2's files are kept in it), its own
# port (WAKUWAKU_PORT), WAKUWAKU_DEBUG=1 for /debug/eval; those set only
# around Start-Process. Its readiness and quiet are read from its
# recent.log. Stopped by POST /quit (her taskbar home gives Windows' back
# only so); killed only if that gets no answer. Its folder deleted after.
#
# Out: A.json and B.json (report.js's input: every run's totals), report.md,
# runs/<scene>-<side>-<n>.json (each measurement whole), logs/ (each copy's
# recent.log), progress.log.
# Nothing else may run on the test machine meanwhile; no copy of her may run
# (the run refuses to start while one does).
# Windows PowerShell 5.1 or pwsh; keep this file ASCII.
param(
    [string]$A,
    [string]$B,
    [string[]]$Scenes = @('S1', 'S2', 'S3', 'S4', 'S5', 'S6', 'S7', 'S8'),
    [int]$Runs = 3,
    [int]$Seconds = 60,
    [int]$Warm = 60,
    [string]$Out,
    [string]$LabelA = 'A',
    [string]$LabelB = 'B',
    [string]$CommitA = '',
    [string]$CommitB = '',
    [switch]$Threads,
    [string]$Vm,
    [string]$VmHelpers = $env:WAKU_PERF_VM_HELPERS,
    [string]$Work = 'C:\wakuperf',
    [int]$PortA = 47331,
    [int]$PortB = 47332,
    [int]$Starts = 0,
    [switch]$FreshWebView,
    [switch]$Collect
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$Scenes = @($Scenes | ForEach-Object { $_ -split ',' } | Where-Object { $_ } | ForEach-Object { $_.Trim().ToUpper() })
$utf8 = New-Object Text.UTF8Encoding $false

function Write-Utf8([string]$path, [string]$text) { [IO.File]::WriteAllText($path, $text, $utf8) }

# ======================================================================================
# From the host: the run started in the VM and followed there.
# ======================================================================================
if ($Vm) {
    if (-not $VmHelpers -or -not (Test-Path -LiteralPath $VmHelpers)) {
        throw '-Vm needs -VmHelpers <path> (or WAKU_PERF_VM_HELPERS): a script giving Use-WakuVM, VM, VMPut and VMGet'
    }
    . $VmHelpers
    Use-WakuVM $Vm | Out-Null
    if (-not $Out) { $Out = Join-Path ([IO.Path]::GetTempPath()) ('wakuwaku-perf-' + (Get-Date -Format 'yyyyMMdd-HHmm')) }
    New-Item -ItemType Directory -Force -Path $Out | Out-Null
    $Out = (Resolve-Path -LiteralPath $Out).Path
    $remoteOut = "$Work\out"
    $q = { param($s) "'" + ($s -replace "'", "''") + "'" }

    if (-not $Collect) {
        if (-not $A -or -not $B) { throw 'run.ps1 -A <exe> -B <exe> ...' }
        foreach ($exe in @($A, $B)) { if (-not (Test-Path -LiteralPath $exe)) { throw "no exe at $exe" } }
        # One run at a time; and not over another's results.
        $busy = VM ('$f = ' + (& $q "$remoteOut\running") + '; if (Test-Path $f) { $p = [int](Get-Content $f); if (Get-Process -Id $p -ErrorAction SilentlyContinue) { "busy $p" } }')
        if ("$busy" -match 'busy') { throw "a run is going on in $Vm already ($busy); -Collect follows it" }
        $old = "$Work\out-" + (Get-Date -Format 'yyyyMMdd-HHmmss')
        [void](VM ('foreach ($d in @(' + (& $q $remoteOut) + ', ' + (& $q "$Work\tools") + ')) { if (Test-Path $d) { if ($d -like "*\out") { [IO.Directory]::Move($d, ' + (& $q $old) + ') } else { [IO.Directory]::Delete($d, $true) } } }; foreach ($d in @("tools\configs", "A", "B", "out")) { [void][IO.Directory]::CreateDirectory((Join-Path ' + (& $q $Work) + ' $d)) }; "ok"'))
        foreach ($f in Get-ChildItem -LiteralPath $here -File -Filter *.ps1) { [void](VMPut $f.FullName "$Work\tools\$($f.Name)") }
        foreach ($f in Get-ChildItem -LiteralPath (Join-Path $here 'configs') -File -Filter *.json) { [void](VMPut $f.FullName "$Work\tools\configs\$($f.Name)") }
        [void](VMPut $A "$Work\A\wakuwaku.exe")
        [void](VMPut $B "$Work\B\wakuwaku.exe")
        $go = '& ' + (& $q "$Work\tools\run.ps1") + ' -A ' + (& $q "$Work\A\wakuwaku.exe") + ' -B ' + (& $q "$Work\B\wakuwaku.exe") +
            ' -Scenes ' + ($Scenes -join ',') + " -Runs $Runs -Seconds $Seconds -Warm $Warm -Out " + (& $q $remoteOut) + ' -Work ' + (& $q $Work) +
            ' -LabelA ' + (& $q $LabelA) + ' -LabelB ' + (& $q $LabelB) + ' -CommitA ' + (& $q $CommitA) + ' -CommitB ' + (& $q $CommitB) +
            " -PortA $PortA -PortB $PortB -Starts $Starts" + $(if ($Threads) { ' -Threads' } else { '' }) + $(if ($FreshWebView) { ' -FreshWebView' } else { '' }) + " *>> " + (& $q "$remoteOut\console.log")
        $goFile = Join-Path ([IO.Path]::GetTempPath()) 'wakuwaku-perf-go.ps1'
        Write-Utf8 $goFile $go
        [void](VMPut $goFile "$Work\tools\go.ps1")
        Remove-Item -LiteralPath $goFile
        $started = VM ('$p = Start-Process powershell.exe -WindowStyle Hidden -PassThru -ArgumentList ' + (& $q ('-NoProfile -ExecutionPolicy Bypass -File "' + "$Work\tools\go.ps1" + '"')) + '; Set-Content -Path ' + (& $q "$remoteOut\running") + ' -Value $p.Id; "started " + $p.Id')
        Write-Host "$Vm`: $started"
    }

    # Followed: its progress as it comes, until it says it is over (or is gone).
    $seen = 0
    for (;;) {
        $text = VM ('$o = ' + (& $q $remoteOut) + '; if (Test-Path "$o\progress.log") { Get-Content "$o\progress.log" -Encoding UTF8 | Select-Object -Skip ' + $seen + ' }; if (Test-Path "$o\finished") { "#finished" } elseif (Test-Path "$o\running") { $p = [int](Get-Content "$o\running"); if (-not (Get-Process -Id $p -ErrorAction SilentlyContinue)) { "#gone" } } else { "#gone" }') 300
        $lines = @("$text" -split "`r?`n" | Where-Object { $_ -ne '' })
        $end = $null
        foreach ($l in $lines) {
            if ($l -eq '#finished' -or $l -eq '#gone') { $end = $l; continue }
            Write-Host $l
            $seen++
        }
        if ($end) { break }
        Start-Sleep -Seconds 30
    }
    # Its results here.
    $files = VM ('Get-ChildItem -LiteralPath ' + (& $q $remoteOut) + ' -Recurse -File | ForEach-Object { $_.FullName }')
    foreach ($f in @("$files" -split "`r?`n" | Where-Object { $_ })) {
        $rel = $f.Substring($remoteOut.Length).TrimStart('\')
        $local = Join-Path $Out $rel
        New-Item -ItemType Directory -Force -Path (Split-Path $local) | Out-Null
        VMGet $f $local | Out-Null
    }
    Write-Host "results in $Out ($end)"
    if ((Test-Path (Join-Path $Out 'A.json')) -and (Test-Path (Join-Path $Out 'B.json'))) {
        & node (Join-Path $here 'report.js') (Join-Path $Out 'A.json') (Join-Path $Out 'B.json') --md (Join-Path $Out 'report.md') --json (Join-Path $Out 'report.json')
    }
    return
}

# ======================================================================================
# On the test machine.
# ======================================================================================
# Their functions; dot-sourcing them binds their parameters here too (-Seconds,
# -Threads, -Out ...), so this script's own are put back after.
$mine = @{ Seconds = $Seconds; Threads = $Threads; Out = $Out }
. (Join-Path $here 'measure.ps1')
. (Join-Path $here 'animations.ps1')
$Seconds = $mine.Seconds
$Threads = $mine.Threads
$Out = $mine.Out
if (-not $A -or -not $B){ throw 'run.ps1 -A <exe> -B <exe> [-Scenes S1,S2] [-Runs 3] [-Out <folder>]' }
$A = (Resolve-Path -LiteralPath $A).Path
$B = (Resolve-Path -LiteralPath $B).Path
if (-not $Out) { $Out = Join-Path $Work 'out' }
foreach ($d in @($Out, (Join-Path $Out 'runs'), (Join-Path $Out 'logs'), (Join-Path $Work 'data'))) { [void][IO.Directory]::CreateDirectory($d) }
$Out = (Resolve-Path -LiteralPath $Out).Path
$progress = Join-Path $Out 'progress.log'
Set-Content -Path (Join-Path $Out 'running') -Value $PID
if (Test-Path (Join-Path $Out 'finished')) { Remove-Item (Join-Path $Out 'finished') }

function Say([string]$line) {
    $l = (Get-Date -Format 'HH:mm:ss') + ' ' + $line
    Write-Host $l
    [IO.File]::AppendAllText($progress, $l + "`r`n", $utf8)
}

function Get-EpochMs([datetime]$t) { ([DateTimeOffset]$t.ToUniversalTime()).ToUnixTimeMilliseconds() }
function Now-Ms { [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }

Add-Type -Namespace WakuPerf -Name Win -MemberDefinition @'
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr ctx);
public delegate bool MonitorProc(IntPtr mon, IntPtr hdc, IntPtr rect, IntPtr data);
[DllImport("user32.dll")] public static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr clip, MonitorProc proc, IntPtr data);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
[StructLayout(LayoutKind.Sequential)] public struct MONITORINFO { public int cbSize; public RECT rc; public RECT work; public uint flags; }
[DllImport("user32.dll")] public static extern bool GetMonitorInfoW(IntPtr mon, ref MONITORINFO mi);
[DllImport("shcore.dll")] public static extern int GetDpiForMonitor(IntPtr mon, int type, out uint x, out uint y);
public static string[] Displays() {
    SetThreadDpiAwarenessContext(new IntPtr(-4));
    var list = new System.Collections.Generic.List<string>();
    EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, delegate (IntPtr mon, IntPtr hdc, IntPtr r, IntPtr d) {
        MONITORINFO mi = new MONITORINFO();
        mi.cbSize = Marshal.SizeOf(typeof(MONITORINFO));
        GetMonitorInfoW(mon, ref mi);
        uint x, y;
        GetDpiForMonitor(mon, 0, out x, out y);
        list.Add(string.Format("{0}x{1} {2}%{3}", mi.rc.R - mi.rc.L, mi.rc.B - mi.rc.T, x * 100 / 96, (mi.flags & 1) != 0 ? " primary" : ""));
        return true;
    }, IntPtr.Zero);
    return list.ToArray();
}
public static string Foreground() {
    var s = new System.Text.StringBuilder(256);
    GetClassNameW(GetForegroundWindow(), s, 256);
    return s.ToString();
}
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] public struct MONITORINFOEX { public int cbSize; public RECT rc; public RECT work; public uint flags; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string device; }
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool GetMonitorInfoW(IntPtr mon, ref MONITORINFOEX mi);
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] public struct DISPLAY_DEVICE { public int cb; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string name; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string text; public uint flags; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string id; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string key; }
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool EnumDisplayDevicesW(string device, uint n, ref DISPLAY_DEVICE dd, uint flags);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
// Each display as her desktop names it (desk.rs key_of): "d" and 48 bits of
// FNV-1a over its monitor's device interface name, lower case; "primary"
// or "other", its middle, as "<p|o> <key> <x> <y>".
public static string[] Keys() {
    SetThreadDpiAwarenessContext(new IntPtr(-4));
    var list = new System.Collections.Generic.List<string>();
    EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, delegate (IntPtr mon, IntPtr hdc, IntPtr r, IntPtr d) {
        MONITORINFOEX mi = new MONITORINFOEX();
        mi.cbSize = Marshal.SizeOf(typeof(MONITORINFOEX));
        GetMonitorInfoW(mon, ref mi);
        DISPLAY_DEVICE dd = new DISPLAY_DEVICE();
        dd.cb = Marshal.SizeOf(typeof(DISPLAY_DEVICE));
        string id = EnumDisplayDevicesW(mi.device, 0, ref dd, 1) && !string.IsNullOrEmpty(dd.id) ? dd.id : mi.device;
        ulong h = 0xcbf29ce484222325UL;
        foreach (char c in id.ToLowerInvariant()) { h ^= (ulong)(c & 0xff); h = unchecked(h * 0x100000001b3UL); }
        list.Add(string.Format("{0} d{1:x12} {2} {3}", (mi.flags & 1) != 0 ? "p" : "o", h & 0xFFFFFFFFFFFFUL, (mi.rc.L + mi.rc.R) / 2, (mi.rc.T + mi.rc.B) / 2));
        return true;
    }, IntPtr.Zero);
    return list.ToArray();
}
public static bool Park(int x, int y) {
    SetThreadDpiAwarenessContext(new IntPtr(-4));
    return SetCursorPos(x, y);
}
'@

# The displays' keys for a scene's "{primary}" and "{other}" (desk layout
# entries per display), and where the pointer rests: the primary display's
# middle, away from her island and strip.
$displayKeys = @{}
$parkAt = @(0, 0)
foreach ($k in [WakuPerf.Win]::Keys()) {
    $f = $k -split ' '
    if ($f[0] -eq 'p') { $displayKeys['{primary}'] = $f[1]; $parkAt = @([int]$f[2], [int]$f[3]) }
    elseif (-not $displayKeys.ContainsKey('{other}')) { $displayKeys['{other}'] = $f[1] }
}

function Test-WindowsTaskbar { [WakuPerf.Win]::IsWindowVisible([WakuPerf.Win]::FindWindowW('Shell_TrayWnd', $null)) }

function Get-PerfMachine {
    $cs = Get-CimInstance Win32_ComputerSystem
    $os = Get-CimInstance Win32_OperatingSystem
    $wv = ''
    foreach ($k in @('HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}')) {
        try { $wv = (Get-ItemProperty -Path $k -ErrorAction Stop).pv; break } catch {}
    }
    [ordered]@{
        name = $env:COMPUTERNAME
        os = "$($os.Caption) $($os.BuildNumber)"
        cpus = $cs.NumberOfLogicalProcessors
        cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
        # Its rated clock: cycles to time for report.js (QueryProcessCycleTime
        # counts at it; a quiet process's CPU time is counted in 15.6 ms ticks).
        mhz = (Get-CimInstance Win32_Processor | Select-Object -First 1).MaxClockSpeed
        memoryGB = [math]::Round($cs.TotalPhysicalMemory / 1GB, 1)
        displays = @([WakuPerf.Win]::Displays())
        webview2 = $wv
    }
}

function Get-PerfExe([string]$exe) {
    $item = Get-Item -LiteralPath $exe
    [ordered]@{ path = $exe; bytes = $item.Length; sha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLower(); fileVersion = $item.VersionInfo.FileVersion }
}

# --- A copy of her ----------------------------------------------------------------------

# Her log since this process started: [ms, text] each.
function Read-PerfLog([string]$data, [int]$procId) {
    $path = Join-Path $data 'recent.log'
    if (-not (Test-Path -LiteralPath $path)) { return @() }
    $fs = [IO.File]::Open($path, 'Open', 'Read', 'ReadWrite, Delete')
    try { $text = (New-Object IO.StreamReader($fs, [Text.Encoding]::UTF8)).ReadToEnd() } finally { $fs.Dispose() }
    $lines = $text -split "`n"
    $from = -1
    for ($i = $lines.Count - 1; $i -ge 0; $i--) { if ($lines[$i] -match "\] start: pid $procId,") { $from = $i; break } }
    if ($from -lt 0) { return @() }
    $outLines = New-Object System.Collections.ArrayList
    for ($i = $from; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^(\d+) \[\w+\] (.*?)\r?$') { [void]$outLines.Add(@([int64]$Matches[1], $Matches[2])) }
    }
    , $outLines.ToArray()
}

function Find-PerfLine($log, [string]$pattern, [int64]$after = 0) {
    foreach ($l in $log) { if ($l[0] -ge $after -and $l[1] -match $pattern) { return $l } }
    $null
}

# The scene's settings as her config.json, in a new folder; with a copy of
# the side's primed WebView2 folder unless -Fresh.
function New-PerfData([string]$data, $cfg, [string]$side = '', [switch]$Fresh) {
    if (Test-Path -LiteralPath $data) { Remove-PerfFolder $data }
    [void][IO.Directory]::CreateDirectory($data)
    if (-not $Fresh -and $side -and $webviews[$side]) {
        $null = & robocopy.exe $webviews[$side] (Join-Path $data 'webview') /E /R:2 /W:1 /NFL /NDL /NJH /NJS /NP
        if ($LASTEXITCODE -ge 8) { throw "could not copy $($webviews[$side]) (robocopy $LASTEXITCODE)" }
        $global:LASTEXITCODE = 0
    }
    $text = $cfg.settings | ConvertTo-Json -Depth 30
    foreach ($k in $displayKeys.Keys) { $text = $text.Replace($k, $displayKeys[$k]) }
    if ($text -match '\{(primary|other)\}') { throw "$($cfg.scene) wants a display this machine does not have ($($Matches[0]))" }
    Write-Utf8 (Join-Path $data 'config.json') $text
}

function Remove-PerfFolder([string]$dir) {
    for ($i = 0; $i -lt 20; $i++) {
        try {
            if (Test-Path -LiteralPath $dir) { [IO.Directory]::Delete($dir, $true) }
            return
        } catch { Start-Sleep -Milliseconds 500 }
    }
    Say "  could not delete $dir"
}

# Started with her test variables set for the Start-Process alone; the
# process's own environment as it was afterwards. --wakuwaku-out-of-job:
# she stays the process started (detach.rs: else, started in a job, she
# starts herself again and this one goes).
$herVars = @('WAKUWAKU_USER_DATA', 'WAKUWAKU_PORT', 'WAKUWAKU_DEBUG', 'WAKUWAKU_QUIET_AFTER_MS', 'WAKUWAKU_IDLE_HIDE_MS', 'WAKUWAKU_BROWSER_ARGS', 'WAKUWAKU_UPDATE_URL', 'WAKUWAKU_UPDATE_GITEE_URL', 'WAKUWAKU_UPDATE_FORCE', 'WAKUWAKU_CRASH_URL', 'WAKUWAKU_CRASH_TEST', 'WAKUWAKU_DESK_TRACE')
function Start-PerfCopy([string]$exe, [string]$data, [int]$port, $envs) {
    $set = @{ WAKUWAKU_USER_DATA = $data; WAKUWAKU_PORT = "$port"; WAKUWAKU_DEBUG = '1' }
    if ($envs) { foreach ($p in $envs.PSObject.Properties) { $set[$p.Name] = [string]$p.Value } }
    $names = @($herVars + @($set.Keys) | Select-Object -Unique)
    $saved = @{}
    foreach ($n in $names) { $saved[$n] = [Environment]::GetEnvironmentVariable($n, 'Process') }
    try {
        foreach ($n in $names) { [Environment]::SetEnvironmentVariable($n, $set[$n], 'Process') }
        $p = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -ArgumentList '--wakuwaku-out-of-job' -PassThru
    } finally {
        foreach ($n in $names) { [Environment]::SetEnvironmentVariable($n, $saved[$n], 'Process') }
    }
    $p
}

# Up (/health answers) and both pages ready (her log: "island: page ready",
# "pet: page ready"; 0.3.2's /health does not say), within $timeout s.
function Wait-PerfReady($p, [string]$data, [int]$port, [int]$timeout = 90) {
    $until = (Get-Date).AddSeconds($timeout)
    while ((Get-Date) -lt $until) {
        if ($p.HasExited) { throw "the copy ended (exit $($p.ExitCode))" }
        $log = Read-PerfLog $data $p.Id
        $island = Find-PerfLine $log '^island: page ready'
        $pet = Find-PerfLine $log '^pet: page ready'
        if ($island -and $pet) {
            try {
                $h = Invoke-RestMethod -TimeoutSec 5 "http://127.0.0.1:$port/health"
                if ($h.ok) { return $log }
            } catch {}
        }
        Start-Sleep -Milliseconds 200
    }
    throw "not ready in $timeout s"
}

# Quiet (her log's last "quiet:" line says true), within $timeout s.
function Wait-PerfQuiet($p, [string]$data, [int]$timeout = 300) {
    $until = (Get-Date).AddSeconds($timeout)
    while ((Get-Date) -lt $until) {
        $last = $null
        foreach ($l in (Read-PerfLog $data $p.Id)) { if ($l[1] -match '^quiet: (true|false)') { $last = $Matches[1] } }
        if ($last -eq 'true') { return $true }
        Start-Sleep -Seconds 2
    }
    $false
}

# The tree under a process (ids), as it is now.
function Get-PerfTreeIds([int]$root) {
    $all = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId)
    $ids = @($root)
    $grew = $true
    while ($grew) {
        $grew = $false
        foreach ($x in $all) {
            if ($ids -notcontains [int]$x.ProcessId -and $ids -contains [int]$x.ParentProcessId) { $ids += [int]$x.ProcessId; $grew = $true }
        }
    }
    $ids
}

# /quit, then every process of hers gone; killed only if not within 40 s.
function Stop-PerfCopy($p, [int]$port) {
    $tree = @(Get-PerfTreeIds $p.Id)
    $how = 'quit'
    try { [void](Invoke-RestMethod -Method Post -TimeoutSec 10 "http://127.0.0.1:$port/quit") } catch { $how = 'quit unanswered' }
    $until = (Get-Date).AddSeconds(40)
    while ((Get-Date) -lt $until) {
        if (-not @($tree | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue }).Count) { break }
        Start-Sleep -Milliseconds 250
    }
    $left = @($tree | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
    if ($left.Count) {
        $how += ", killed $($left.Count) left"
        foreach ($id in $left) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Seconds 2
    }
    $how
}

function Send-PerfHook([int]$port, $body) {
    $json = $body | ConvertTo-Json -Compress -Depth 10
    try {
        [void](Invoke-RestMethod -Method Post -TimeoutSec 10 "http://127.0.0.1:$port/hook" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($json)))
        $true
    } catch { Say "  hook failed: $($_.Exception.Message)"; $false }
}

function Assert-PerfClear([int[]]$ports) {
    $others = @(Get-Process wakuwaku* -ErrorAction SilentlyContinue)
    if ($others.Count) { throw ("a copy of her runs (" + (($others | ForEach-Object { "$($_.Id) $($_.Path)" }) -join '; ') + "): quit it first") }
    foreach ($port in $ports) {
        $l = New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback, $port)
        try { $l.Start() } catch { throw "port $port is taken" } finally { try { $l.Stop() } catch {} }
    }
}

# --- Scenes --------------------------------------------------------------------------------

$sides = [ordered]@{
    A = @{ exe = $A; port = $PortA; label = $LabelA; commit = $CommitA }
    B = @{ exe = $B; port = $PortB; label = $LabelB; commit = $CommitB }
}
$machine = Get-PerfMachine
$results = [ordered]@{}
foreach ($s in $sides.Keys) {
    $results[$s] = [ordered]@{
        side = $s; label = $sides[$s].label; commit = $sides[$s].commit; exe = (Get-PerfExe $sides[$s].exe)
        machine = $machine; date = (Get-Date -Format 's'); seconds = $Seconds; warm = $Warm; scenes = [ordered]@{}
    }
}

function Save-PerfResults {
    foreach ($s in $sides.Keys) { Write-Utf8 (Join-Path $Out "$s.json") ($results[$s] | ConvertTo-Json -Depth 12) }
}

function Get-PerfScene([string]$scene) {
    $file = Join-Path $here "configs\$scene.json"
    if (-not (Test-Path -LiteralPath $file)) { throw "no scene $scene ($file)" }
    Get-Content -LiteralPath $file -Raw -Encoding UTF8 | ConvertFrom-Json
}

function Add-PerfRun([string]$side, $cfg, $rec) {
    $sc = $results[$side].scenes
    if (-not $sc.Contains($cfg.scene)) { $sc[$cfg.scene] = [ordered]@{ title = $cfg.title; quiet = [bool]$cfg.quiet; runs = @() } }
    $sc[$cfg.scene].runs += $rec
}

# One measured run of a scene: started, ready, warmed up, quiet where the
# scene is, measured, its endless animations listed, quit, cleaned away.
function Invoke-PerfRun($cfg, [string]$side, [int]$n) {
    $exe = $sides[$side].exe
    $port = $sides[$side].port
    $name = "$($cfg.scene)-$side-$n"
    $data = Join-Path $Work "data\$name"
    $rec = [ordered]@{ run = $n; notes = @() }
    Assert-PerfClear @($port)
    if (-not (Test-WindowsTaskbar)) { $rec.notes += "Windows' taskbar not shown before the start" }
    # The pointer away from her (SetCursorPos: not input, quiet stays).
    [void][WakuPerf.Win]::Park($parkAt[0], $parkAt[1])
    New-PerfData $data $cfg $side
    $p = Start-PerfCopy $exe $data $port $cfg.env
    $nudge = $null
    try {
        [void](Wait-PerfReady $p $data $port)
        foreach ($h in @($cfg.hooks)) { if ($h) { [void](Send-PerfHook $port $h) } }
        if ($cfg.nudge) {
            $nudge = Start-Process powershell.exe -WindowStyle Hidden -PassThru -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $here 'nudge.ps1'), '-Seconds', [string]($Warm + $Seconds + 600), '-Every', [string]$cfg.nudge)
        }
        Start-Sleep -Seconds $Warm
        if ($cfg.quiet) {
            if (-not (Wait-PerfQuiet $p $data 300)) { $rec.notes += 'not quiet within 300 s' }
        }
        $rec.foreground = [WakuPerf.Win]::Foreground()
        if ($cfg.event) {
            # The event's own cost (S6: the ring after "done"): a window from
            # just before it, then the hold after it.
            $m = Start-PerfTree -Root $p.Id -Threads:$Threads
            foreach ($h in @($cfg.event.hooks)) { if ($h) { [void](Send-PerfHook $port $h) } }
            Wait-PerfTree $m ([int]$cfg.event.seconds * 1000)
            $ev = Stop-PerfTree $m
            $rec.event = [ordered]@{ seconds = $ev.seconds; totals = $ev.totals }
            Write-Utf8 (Join-Path $Out "runs\$name-event.json") ($ev | ConvertTo-Json -Depth 8)
            if ($cfg.event.settle) { Start-Sleep -Seconds ([int]$cfg.event.settle) }
        }
        $since = Now-Ms
        $res = Measure-PerfTree -Root $p.Id -Seconds $Seconds -Threads:$Threads
        $rec.totals = $res.totals
        $rec.byKind = [ordered]@{}
        foreach ($x in $res.processes) {
            if (-not $rec.byKind.Contains($x.kind)) { $rec.byKind[$x.kind] = [ordered]@{ mcycPerSec = 0.0; msPerSec = 0.0; wakeupsPerSec = 0.0; privateMB = 0.0; processes = 0 } }
            $k = $rec.byKind[$x.kind]
            foreach ($f in @('mcycPerSec', 'msPerSec', 'wakeupsPerSec', 'privateMB')) { $k[$f] = [math]::Round($k[$f] + $x[$f], 3) }
            $k.processes++
        }
        Write-Utf8 (Join-Path $Out "runs\$name.json") ($res | ConvertTo-Json -Depth 8)
        $anim = Get-PerfAnimations -Port $port -Allow @($cfg.allow)
        $rec.animations = $anim
        # Quiet all through the window, where the scene is to be.
        $line = if ($cfg.quiet) { '^quiet: false' } else { '^quiet: true' }
        $turned = Find-PerfLine (Read-PerfLog $data $p.Id) $line $since
        if ($turned) {
            $what = ($turned[1] -split ' ', 3)[0..1] -join ' '
            $rec.notes += ('{0} in the window ({1} ms in)' -f $what, ($turned[0] - $since))
        }
    } catch {
        $rec.error = $_.Exception.Message
        Say "  $name failed: $($rec.error)"
    } finally {
        if ($nudge -and -not $nudge.HasExited) { Stop-Process -Id $nudge.Id -Force -ErrorAction SilentlyContinue }
        $rec.stop = Stop-PerfCopy $p $port
        if (-not (Test-WindowsTaskbar)) { $rec.notes += "Windows' taskbar not shown after the quit"; Say "  Windows' taskbar not shown after $name" }
        if (Test-Path -LiteralPath (Join-Path $data 'recent.log')) { Copy-Item -LiteralPath (Join-Path $data 'recent.log') -Destination (Join-Path $Out "logs\$name.log") }
        Remove-PerfFolder $data
    }
    $rec
}

# Start-up (S7): cold (a new copy of the exe, a new data and WebView2 folder)
# and warm (the same exe and folder again), each side in turn; from her log,
# process start -> up -> pages ready -> Windows' taskbar put away.
function Measure-PerfStart($cfg, [string]$side, [string]$kind, [int]$n, [string]$exe, [string]$data) {
    $port = $sides[$side].port
    $name = "S7-$kind-$side-$n"
    $rec = [ordered]@{ kind = $kind; n = $n; notes = @() }
    Assert-PerfClear @($port)
    [void][WakuPerf.Win]::Park($parkAt[0], $parkAt[1])
    # Cold: all new. The warm folder: made by its first start (primed as the runs' are).
    if ($kind -eq 'cold') { New-PerfData $data $cfg -Fresh } elseif ($kind -eq 'prime') { New-PerfData $data $cfg $side }
    $p = Start-PerfCopy $exe $data $port $cfg.env
    try {
        $t0 = Get-EpochMs $p.StartTime
        $log = Wait-PerfReady $p $data $port
        $taskbar = $cfg.settings.display -eq 'taskbar'
        $until = (Get-Date).AddSeconds(30)
        while ($taskbar -and (Get-Date) -lt $until -and -not (Find-PerfLine $log "^taskbar: Windows' put away")) {
            Start-Sleep -Milliseconds 250
            $log = Read-PerfLog $data $p.Id
        }
        Start-Sleep -Seconds 3
        $log = Read-PerfLog $data $p.Id
        $up = Find-PerfLine $log '^up on port'
        $island = Find-PerfLine $log '^island: page ready'
        $pet = Find-PerfLine $log '^pet: page ready'
        $taken = Find-PerfLine $log "^taskbar: Windows' put away"
        $ready = [math]::Max($island[0], $pet[0])
        $rec.procToUp = $(if ($up) { $up[0] - $t0 } else { $null })
        $rec.procToIsland = $island[0] - $t0
        $rec.procToPet = $pet[0] - $t0
        $rec.procToReady = $ready - $t0
        if ($taken) {
            $rec.procToTaken = $taken[0] - $t0
            $rec.readyToTaken = $taken[0] - $ready
            if ($taken[1] -match '\(put away (\d+) ms, guard (\d+) ms, host (\d+) ms\)') { $rec.take = "put away $($Matches[1]), guard $($Matches[2]), host $($Matches[3]) ms" }
        } elseif ($taskbar) { $rec.notes += 'no taskbar line' }
    } catch {
        $rec.error = $_.Exception.Message
        Say "  $name failed: $($rec.error)"
    } finally {
        $rec.stop = Stop-PerfCopy $p $port
        if (-not (Test-WindowsTaskbar)) { $rec.notes += "Windows' taskbar not shown after the quit"; Say "  Windows' taskbar not shown after $name" }
        if (Test-Path -LiteralPath (Join-Path $data 'recent.log')) { Copy-Item -LiteralPath (Join-Path $data 'recent.log') -Destination (Join-Path $Out "logs\$name.log") -Force }
    }
    Say ("  {0}: up {1}, ready {2}, taken {3} ms" -f $name, $rec.procToUp, $rec.procToReady, $rec.procToTaken)
    $rec
}

function Invoke-PerfStarts($cfg) {
    $cold = $(if ($Starts) { $Starts } else { [int]$cfg.starts.cold })
    $warm = $(if ($Starts) { $Starts } else { [int]$cfg.starts.warm })
    foreach ($s in $sides.Keys) { $results[$s].scenes[$cfg.scene] = [ordered]@{ title = $cfg.title; starts = @() } }
    for ($i = 1; $i -le $cold; $i++) {
        foreach ($s in $sides.Keys) {
            $dir = Join-Path $Work "cold\$s-$i"
            [void][IO.Directory]::CreateDirectory($dir)
            $exe = Join-Path $dir 'wakuwaku.exe'
            Copy-Item -LiteralPath $sides[$s].exe -Destination $exe -Force
            $data = Join-Path $Work "data\S7-cold-$s-$i"
            $results[$s].scenes[$cfg.scene].starts += (Measure-PerfStart $cfg $s 'cold' $i $exe $data)
            Remove-PerfFolder $data
            Remove-PerfFolder $dir
            Save-PerfResults
            Start-Sleep -Seconds 5
        }
    }
    # Warm: each side's folder made by a start not counted.
    $warmData = @{}
    foreach ($s in $sides.Keys) {
        $warmData[$s] = Join-Path $Work "data\S7-warm-$s"
        [void](Measure-PerfStart $cfg $s 'prime' 0 $sides[$s].exe $warmData[$s])
        Start-Sleep -Seconds 5
    }
    for ($i = 1; $i -le $warm; $i++) {
        foreach ($s in $sides.Keys) {
            $results[$s].scenes[$cfg.scene].starts += (Measure-PerfStart $cfg $s 'warm' $i $sides[$s].exe $warmData[$s])
            Save-PerfResults
            Start-Sleep -Seconds 5
        }
    }
    foreach ($s in $sides.Keys) { Remove-PerfFolder $warmData[$s] }
}

# Each side's WebView2 folder primed once (docs/PERFORMANCE.md section 6). A
# new one fetches some 60 MB of WebView2's own components in its first
# minutes (Widevine, the subresource filter, speech, hyphenation ...): in
# some runs that landed in the window, tens of Mcyc/s in WebView2's browser
# process on either side (2026-10-18). A copy started once and left until
# that folder stops growing, then quit; each run starts on a copy of it, as
# a person's long-used one would be. S7's cold starts keep a new one.
$webviews = @{}
function Initialize-PerfWebView([string]$side, $cfg) {
    $data = Join-Path $Work "data\prime-$side"
    New-PerfData $data $cfg -Fresh
    $port = $sides[$side].port
    $p = Start-PerfCopy $sides[$side].exe $data $port $null
    $size = -1
    $still = 0
    $t0 = Get-Date
    try {
        [void](Wait-PerfReady $p $data $port)
        while (((Get-Date) - $t0).TotalSeconds -lt 300) {
            Start-Sleep -Seconds 5
            $now = (Get-ChildItem -LiteralPath (Join-Path $data 'webview') -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum
            if ($now -eq $size) { $still += 5 } else { $still = 0; $size = $now }
            if ($still -ge 30 -and ((Get-Date) - $t0).TotalSeconds -ge 90) { break }
        }
    } finally {
        [void](Stop-PerfCopy $p $port)
    }
    $keep = Join-Path $Work "webview-$side"
    if (Test-Path -LiteralPath $keep) { Remove-PerfFolder $keep }
    [IO.Directory]::Move((Join-Path $data 'webview'), $keep)
    Remove-PerfFolder $data
    $webviews[$side] = $keep
    Say ("  {0}'s WebView2 folder primed: {1:N0} MB, still for 30 s after {2:N0} s" -f $side, ($size / 1MB), ((Get-Date) - $t0).TotalSeconds)
}

# --- The run ---------------------------------------------------------------------------------

Say "run: $($Scenes -join ',') x $Runs, $Seconds s each after $Warm s; A $LabelA ($($results.A.exe.sha256.Substring(0, 12))), B $LabelB ($($results.B.exe.sha256.Substring(0, 12))); $($machine.name), $($machine.displays -join ' + ')"
Assert-PerfClear @($PortA, $PortB)
try {
    if ($FreshWebView) {
        Say 'each run with a new WebView2 folder (-FreshWebView)'
    } else {
        foreach ($s in $sides.Keys) { Initialize-PerfWebView $s (Get-PerfScene 'S1') }
    }
    foreach ($s in $sides.Keys) { $results[$s].webview = $(if ($FreshWebView) { 'new each run' } else { 'primed once, copied into each run' }) }
    foreach ($scene in $Scenes) {
        $cfg = Get-PerfScene $scene
        Say "$scene $($cfg.title)"
        if ($cfg.starts) {
            Invoke-PerfStarts $cfg
            Save-PerfResults
            continue
        }
        for ($n = 1; $n -le $Runs; $n++) {
            foreach ($s in $sides.Keys) {
                $rec = Invoke-PerfRun $cfg $s $n
                Add-PerfRun $s $cfg $rec
                Save-PerfResults
                $t = $rec.totals
                $extra = ''
                if ($rec.event) { $extra = (' | event {0:N0} Mcyc in {1:N0} s' -f ($rec.event.totals.mcycPerSec * $rec.event.seconds), $rec.event.seconds) }
                if ($t) {
                    Say ("  {0}-{1}-{2}: {3:N1} Mcyc/s, {4:N2} ms/s, {5:N0} wakeups/s, {6:N0} MB private, GPU {7}%, endless {8}{9}{10}" -f $scene, $s, $n, $t.mcycPerSec, $t.msPerSec, $t.wakeupsPerSec, $t.privateMB, $t.gpuPct, $rec.animations.count, $extra, $(if ($rec.notes.Count) { ' [' + ($rec.notes -join '; ') + ']' } else { '' }))
                }
                Start-Sleep -Seconds 5
            }
        }
    }
} finally {
    Save-PerfResults
    foreach ($w in $webviews.Values) { Remove-PerfFolder $w }
    try {
        $node = Get-Command node -ErrorAction SilentlyContinue
        if ($node) { & $node.Source (Join-Path $here 'report.js') (Join-Path $Out 'A.json') (Join-Path $Out 'B.json') --md (Join-Path $Out 'report.md') --json (Join-Path $Out 'report.json') | Out-Null }
    } catch {}
    Set-Content -Path (Join-Path $Out 'finished') -Value (Get-Date -Format 's')
    Remove-Item -LiteralPath (Join-Path $Out 'running') -ErrorAction SilentlyContinue
    Say 'finished'
}
