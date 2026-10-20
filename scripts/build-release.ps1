#!/usr/bin/env pwsh
# Builds the release asset: src-tauri's release exe, named as the updater
# takes it (wakuwaku-v<version>-windows-x64.exe), with no path of the machine
# that built it inside.
#
#   pwsh scripts/build-release.ps1 [-Out <folder>]
#
# A Rust exe names the source file of every place that may panic (an
# unwrap, an index) in plain text, for the panic's message: her own files by
# their path in the repo, but the crates' from cargo's cache and the standard
# library's from the toolchain by whole paths, the user's name in them
# (C:\Users\<name>\.cargo\registry\src\..., ...\.rustup\toolchains\...; 425 of
# them in v0.3.3), and the vendored tiny_http's (a path dependency) by the
# repo's whole path. So rustc is told to write those as crates\..., rust\...
# and wakuwaku\... (--remap-path-prefix; cargo's trim-paths, which would do
# it for every build, is not stable yet), the messages still saying which
# crate's file and line. And build.rs is told it is an asset
# (WAKUWAKU_ASSET=1): no source tree compiled in (data.rs), which an exe run
# from its target folder uses to keep its data in the tree.
#
# It builds into a target folder of its own (src-tauri\target\asset): the
# flags differ from a plain build's, so sharing target\release would rebuild
# all of it each way, and would replace the exe a build from source runs.
# Then it looks through the exe for any path of this machine left, and stops
# if one is. The exe goes into -Out (else that target folder), with its size
# and SHA-256.
param(
    [string]$Out
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$tauri = Join-Path $root 'src-tauri'
$version = (Select-String -LiteralPath (Join-Path $tauri 'Cargo.toml') -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$name = "wakuwaku-v$version-windows-x64.exe"
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$sysroot = (& rustc --print sysroot).Trim()
$target = Join-Path $tauri 'target\asset'
if (-not $Out) { $Out = $target }

# Each prefix and what it is written as; where two match, rustc takes the
# last, so the repo goes first (a cargo home or a toolchain inside it wins).
$maps = @(
    @($root, 'wakuwaku'),
    @((Join-Path $cargoHome 'registry\src'), 'crates'),
    @((Join-Path $cargoHome 'git\checkouts'), 'git'),
    @($sysroot, 'rust')
)
# As one argument each (CARGO_ENCODED_RUSTFLAGS, split on 0x1F), so a path
# with a space in it stays whole.
$flags = $maps | ForEach-Object { "--remap-path-prefix=$($_[0])=$($_[1])" }
$saved = @{ RUSTFLAGS = $env:RUSTFLAGS; CARGO_ENCODED_RUSTFLAGS = $env:CARGO_ENCODED_RUSTFLAGS; CARGO_TARGET_DIR = $env:CARGO_TARGET_DIR; WAKUWAKU_ASSET = $env:WAKUWAKU_ASSET }
try {
    $env:RUSTFLAGS = $null
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]0x1F
    $env:CARGO_TARGET_DIR = $target
    $env:WAKUWAKU_ASSET = '1'
    "building v$version (remapped: $(($maps | ForEach-Object { $_[1] }) -join ', '))"
    Push-Location $tauri
    try {
        & cargo build --release
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }
    } finally {
        Pop-Location
    }
} finally {
    # As they were (a null one taken away).
    foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) }
}

$built = Join-Path $target 'release\wakuwaku.exe'
# Any path of this machine left: the prefixes above, the user's folder, a
# drive's Users folder at all.
$text = [Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($built))
$needles = @($root, $cargoHome, $sysroot, $env:USERPROFILE) | Where-Object { $_ } | ForEach-Object { [regex]::Escape($_) }
$left = [regex]::Matches($text, "(?i)($($needles -join '|')|[a-z]:\\users\\)[ -~]{0,80}")
if ($left.Count -gt 0) {
    $left | Select-Object -First 10 | ForEach-Object { "  left: $($_.Value)" }
    throw "$($left.Count) path(s) of this machine are still in $built"
}

New-Item -ItemType Directory -Force -Path $Out | Out-Null
$asset = Join-Path $Out $name
Copy-Item -LiteralPath $built -Destination $asset -Force
$size = (Get-Item -LiteralPath $asset).Length
$sha = (Get-FileHash -Algorithm SHA256 -LiteralPath $asset).Hash.ToLowerInvariant()
"no path of this machine in it"
"$asset"
"$size bytes, SHA-256 $sha"
