# Lists the animations running endlessly in each of her pages (docs/PERFORMANCE.md
# section 2, the last row): through /debug/eval, document.getAnimations()
# in every page that answers it, those playing with infinite iterations.
# A copy started with WAKUWAKU_DEBUG=1 only.
#
#   powershell -File tools/perf/animations.ps1 -Port <n> [-Allow ring,glow] [-Json]
#   . tools/perf/animations.ps1          # dot-sourced: the functions only (run.ps1)
#
# Pages asked: pet, island (her home: the island or the taskbar strip) and
# the desktop's (desk-0 ...). The side screens' taskbars (bar-N) and the
# taskbar's flyout (barpop) do not answer /debug/eval and are not asked.
# -Allow: animation names a scene allows (its exceptions); the rest are
# counted as "disallowed".
# Windows PowerShell 5.1 or pwsh; keep this file ASCII.
param(
    [int]$Port,
    [string[]]$Allow = @(),
    [string[]]$Pages = @('pet', 'island', 'desk-0', 'desk-1', 'desk-2', 'desk-3', 'desk-4', 'desk-5'),
    [switch]$Json
)

# What one page has running: every animation, CSS or scripted, playing and
# endless; each with its name and the element it moves.
$animationsJs = @'
(() => {
  const what = (el) => {
    if (!el || !el.tagName) return ''
    const cls = typeof el.className === 'string' ? el.className.trim().split(/\s+/).slice(0, 3).join('.') : ''
    return el.tagName.toLowerCase() + (el.id ? '#' + el.id : '') + (cls ? '.' + cls : '')
  }
  const all = document.getAnimations()
  const endless = all.filter((a) => {
    if (a.playState !== 'running') return false
    const t = a.effect && a.effect.getComputedTiming ? a.effect.getComputedTiming() : null
    return t && t.iterations === Infinity
  })
  return {
    hidden: document.hidden,
    total: all.length,
    running: all.filter((a) => a.playState === 'running').length,
    endless: endless.map((a) => ({ name: a.animationName || a.transitionProperty || a.id || a.constructor.name, target: what(a.effect && a.effect.target) })),
  }
})()
'@

function Invoke-PerfEval([int]$Port, [string]$Page, [string]$Code, [int]$TimeoutSec = 6) {
    $body = @{ page = $Page; code = $Code } | ConvertTo-Json -Compress
    try {
        $r = Invoke-WebRequest -UseBasicParsing -Method Post -TimeoutSec $TimeoutSec -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) "http://127.0.0.1:$Port/debug/eval"
        $text = [Text.Encoding]::UTF8.GetString($r.RawContentStream.ToArray())
        $j = $text | ConvertFrom-Json
        if ($null -ne $j.error) { return $null }
        return $j.result
    } catch {
        return $null
    }
}

# Every page's endless animations: { count, disallowed, pages: { <page>: { hidden, total, running, endless: [...] } } }.
function Get-PerfAnimations([int]$Port, [string[]]$Allow = @(), [string[]]$Pages = @('pet', 'island', 'desk-0', 'desk-1', 'desk-2', 'desk-3', 'desk-4', 'desk-5')) {
    $seen = [ordered]@{}
    $count = 0
    $disallowed = 0
    foreach ($page in $Pages) {
        $r = Invoke-PerfEval $Port $page $animationsJs
        if ($null -eq $r -or $r -is [string]) { continue }
        $seen[$page] = $r
        foreach ($a in @($r.endless)) {
            if ($null -eq $a) { continue }
            $count++
            if ($Allow -notcontains [string]$a.name) { $disallowed++ }
        }
    }
    [ordered]@{ count = $count; disallowed = $disallowed; pages = $seen }
}

if ($MyInvocation.InvocationName -eq '.') { return }
if (-not $Port) { throw 'animations.ps1 -Port <n> [-Allow name,...] [-Json]' }
$Allow = @($Allow | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$found = Get-PerfAnimations -Port $Port -Allow $Allow -Pages $Pages
if ($Json) { return ($found | ConvertTo-Json -Depth 8) }
foreach ($page in $found.pages.Keys) {
    $p = $found.pages[$page]
    "{0}: {1} animations, {2} running, {3} endless{4}" -f $page, $p.total, $p.running, @($p.endless).Count, $(if ($p.hidden) { ' (hidden)' } else { '' })
    foreach ($a in @($p.endless)) { if ($a) { "  {0} on {1}{2}" -f $a.name, $a.target, $(if ($Allow -contains [string]$a.name) { ' (allowed)' } else { '' }) } }
}
"endless: {0} ({1} not allowed)" -f $found.count, $found.disallowed
