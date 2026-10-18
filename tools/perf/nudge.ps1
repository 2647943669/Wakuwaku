# Someone at the test machine (scene S3, docs/PERFORMANCE.md): the mouse moved
# a few pixels and back every few seconds, as real input (SendInput, which
# Windows counts as input; SetCursorPos alone does not), never a click.
# Run on the test machine only, in the desktop session her copy runs in.
#
#   powershell -File tools/perf/nudge.ps1 -Seconds 200 [-Every 10] [-X 1280 -Y 700]
#
# -X/-Y: where the pointer rests first (physical pixels; the primary
# display's middle by default), away from her strip and her island.
# Windows PowerShell 5.1 or pwsh; keep this file ASCII.
param(
    [int]$Seconds = 200,
    [int]$Every = 10,
    [int]$X = -1,
    [int]$Y = -1
)
Add-Type -Namespace WakuPerf -Name Nudge -MemberDefinition @'
[StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
[StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public MOUSEINPUT mi; }
[DllImport("user32.dll", SetLastError = true)] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr ctx);
[DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
public static uint Move(int dx, int dy) {
    INPUT[] i = new INPUT[1];
    i[0].type = 0;
    i[0].mi.dx = dx;
    i[0].mi.dy = dy;
    i[0].mi.dwFlags = 1;
    return SendInput(1, i, Marshal.SizeOf(typeof(INPUT)));
}
'@
[void][WakuPerf.Nudge]::SetThreadDpiAwarenessContext([IntPtr](-4))
if ($X -lt 0) { $X = [int]([WakuPerf.Nudge]::GetSystemMetrics(0) / 2) }
if ($Y -lt 0) { $Y = [int]([WakuPerf.Nudge]::GetSystemMetrics(1) / 2) }
[void][WakuPerf.Nudge]::SetCursorPos($X, $Y)
$until = (Get-Date).AddSeconds($Seconds)
$sign = 1
while ((Get-Date) -lt $until) {
    [void][WakuPerf.Nudge]::Move(3 * $sign, 0)
    $sign = -$sign
    Start-Sleep -Seconds $Every
}
