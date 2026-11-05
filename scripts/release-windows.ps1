# Builds dist\Kiri-windows-x64.zip, which holds Kiri\Kiri.exe, and with -Upload adds it to the
# GitHub release v<version> that scripts/release.sh --publish created. Kiri isn't code-signed on
# Windows yet. See docs/operations/release.md.
param([switch]$Upload)
Set-Location (Join-Path $PSScriptRoot '..')

function Check($what) {
    if ($LASTEXITCODE -ne 0) { throw "$what failed" }
}

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$dir = 'dist\Kiri'
$zip = 'dist\Kiri-windows-x64.zip'

if ($Upload) {
    git diff --quiet HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Commit first: the zip is built from HEAD.' }
    gh release view "v$version" --json tagName | Out-Null
    Check "Finding release v$version (scripts/release.sh --publish creates it)"
}

cargo build --release
Check 'cargo build'
Remove-Item -Recurse -Force $dir, $zip -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item target\release\kiri.exe "$dir\Kiri.exe"
# Windows' own tar, which the updater unzips with too.
& "$env:SystemRoot\System32\tar.exe" -a -c -f $zip -C dist Kiri
Check 'tar'
Write-Output "Built $zip ($version)"

if ($Upload) {
    gh release upload "v$version" $zip --clobber
    Check 'gh release upload'
}
