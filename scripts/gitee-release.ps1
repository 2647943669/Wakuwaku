#!/usr/bin/env pwsh
# Copies a GitHub release of Wakuwaku to Gitee (gitee.com/zhoupengjie/Wakuwaku),
# for those in China: the same tag, title and notes; the same exe, taken from
# GitHub's release (or given) and checked by GitHub's SHA-256; and
# <exe>.sha256 beside it, which the updater checks by when GitHub gives it
# none (src-tauri/src/updates.rs). Then the exe downloaded back from Gitee,
# without an account, and checked again.
#
#   $env:GITEE_TOKEN = '<a Gitee private token, scope projects>'
#   pwsh scripts/gitee-release.ps1 -Version 0.2.2 [-Exe <the exe>] [-DryRun]
#
# The tag must be on Gitee first (git push <gitee remote> v0.2.2). Run again,
# it adds only what is missing. -DryRun does all but the writes to Gitee (no
# token needed).
param(
    [Parameter(Mandatory)][string]$Version,
    [string]$Exe,
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
$Version = $Version.TrimStart('v', 'V')
$tag = "v$Version"
$name = "wakuwaku-v$Version-windows-x64.exe"
$owner = 'zhoupengjie'
$repo = 'Wakuwaku'
$api = "https://gitee.com/api/v5/repos/$owner/$repo"
$headers = @{ 'User-Agent' = 'wakuwaku-release' }
$token = $env:GITEE_TOKEN
if (-not $token -and -not $DryRun) { throw 'GITEE_TOKEN is not set (a Gitee private token with the projects scope)' }

function Sha256([string]$path) { (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant() }

# GitHub's release: its title, its notes, its exe's SHA-256.
$gh = Invoke-RestMethod -Headers ($headers + @{ Accept = 'application/vnd.github+json' }) "https://api.github.com/repos/$owner/$repo/releases/tags/$tag"
$asset = $gh.assets | Where-Object name -EQ $name
if (-not $asset) { throw "GitHub's $tag has no $name" }
$digest = "$($asset.digest)" -replace '^sha256:', ''
if ($digest -notmatch '^[0-9a-fA-F]{64}$') { throw "GitHub gives no SHA-256 for $name" }
$digest = $digest.ToLowerInvariant()

# The exe, as GitHub has it, and its .sha256 as sha256sum writes it.
$work = Join-Path ([IO.Path]::GetTempPath()) "wakuwaku-gitee-$Version"
New-Item -ItemType Directory -Force $work | Out-Null
$file = Join-Path $work $name
if ($Exe) { Copy-Item -LiteralPath $Exe $file -Force }
else { Invoke-WebRequest -Headers $headers $asset.browser_download_url -OutFile $file }
if ((Sha256 $file) -ne $digest) { throw "$name is not the one GitHub gives (SHA-256 $digest)" }
$sums = "$file.sha256"
[IO.File]::WriteAllText($sums, "$digest  $name`n", [Text.UTF8Encoding]::new($false))
"GitHub's $name`: SHA-256 $digest"

# The tag on Gitee, and the commit it is on.
$refs = git ls-remote --tags "https://gitee.com/$owner/$repo.git"
$line = $refs | Where-Object { $_ -match "\trefs/tags/$([regex]::Escape($tag))\^\{\}$" } | Select-Object -First 1
if (-not $line) { $line = $refs | Where-Object { $_ -match "\trefs/tags/$([regex]::Escape($tag))$" } | Select-Object -First 1 }
if (-not $line) { throw "$tag is not on Gitee yet: push it first (git push <gitee remote> $tag)" }
$commit = ($line -split "`t")[0]
"Gitee's $tag`: $commit"

# The release there, made if missing (GitHub's title and notes).
$release = Invoke-RestMethod -Headers $headers "$api/releases/tags/$tag"
if (-not $release.id) {
    if ($DryRun) { "would make the release $tag on Gitee: $($gh.name)"; return }
    $form = @{ access_token = $token; tag_name = $tag; name = $gh.name; body = "$($gh.body)"; prerelease = 'false'; target_commitish = $commit }
    $release = Invoke-RestMethod -Method Post -Headers $headers -ContentType 'application/x-www-form-urlencoded; charset=utf-8' -Body $form "$api/releases"
    "made the release $tag on Gitee (id $($release.id))"
}

# The exe and its .sha256, those not there yet.
$have = @(Invoke-RestMethod -Headers $headers "$api/releases/$($release.id)/attach_files" | ForEach-Object { $_.name })
foreach ($f in @($file, $sums)) {
    $leaf = Split-Path $f -Leaf
    if ($have -contains $leaf) { "on Gitee already: $leaf"; continue }
    if ($DryRun) { "would upload: $leaf"; continue }
    Invoke-RestMethod -Method Post -Headers $headers -Form @{ access_token = $token; file = Get-Item -LiteralPath $f } "$api/releases/$($release.id)/attach_files" | Out-Null
    "uploaded: $leaf"
}
if ($DryRun) { return }

# Downloaded back as the updater does (no account): the exe, and the .sha256.
$listed = Invoke-RestMethod -Headers $headers "$api/releases?per_page=10&direction=desc" | Where-Object tag_name -EQ $tag
$url = ($listed.assets | Where-Object name -EQ $name).browser_download_url
$sumsUrl = ($listed.assets | Where-Object name -EQ "$name.sha256").browser_download_url
if (-not $url -or -not $sumsUrl) { throw "Gitee's $tag does not list $name and its .sha256" }
$back = Join-Path $work "back-$name"
Invoke-WebRequest -Headers $headers $url -OutFile $back
if ((Sha256 $back) -ne $digest) { throw "Gitee's $name is not GitHub's: delete it there and run this again" }
Invoke-WebRequest -Headers $headers $sumsUrl -OutFile "$back.sha256"
$said = ((Get-Content -Raw -LiteralPath "$back.sha256") -split '\s+')[0].ToLowerInvariant()
if ($said -ne $digest) { throw "Gitee's $name.sha256 says $said, not $digest" }
"checked: Gitee gives the same exe, and its .sha256 agrees"
"https://gitee.com/$owner/$repo/releases/tag/$tag"
