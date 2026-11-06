# Builds dist\Kiri-Setup.exe, the installer, and dist\Kiri-windows-x64.zip, which holds Kiri\Kiri.exe
# for the updater and a portable copy. With -Upload it adds both to the GitHub release v<version>
# that scripts/release.sh --publish created. Kiri isn't code-signed on Windows yet.
# See docs/operations/release.md.
param([switch]$Upload)
Set-Location (Join-Path $PSScriptRoot '..')

function Check($what) {
    if ($LASTEXITCODE -ne 0) { throw "$what failed" }
}

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$dir = 'dist\Kiri'
$zip = 'dist\Kiri-windows-x64.zip'
$setup = 'dist\Kiri-Setup.exe'

$iscc = @(
    (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source,
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) { throw 'Inno Setup 6 is missing; see docs/operations/release.md.' }

if ($Upload) {
    git diff --quiet HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Commit first: the assets are built from HEAD.' }
    gh release view "v$version" --json tagName | Out-Null
    Check "Finding release v$version (scripts/release.sh --publish creates it)"
}

cargo build --release
Check 'cargo build'
Remove-Item -Recurse -Force $dir, $zip, $setup -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item target\release\kiri.exe "$dir\Kiri.exe"
# Windows' own tar, which the updater unzips with too.
& "$env:SystemRoot\System32\tar.exe" -a -c -f $zip -C dist Kiri
Check 'tar'
& $iscc /Q /Odist "/DAppVersion=$version" "/DNumericVersion=$($version -replace '-.*')" "/DExe=$((Resolve-Path "$dir\Kiri.exe").Path)" windows\Kiri.iss
Check 'Inno Setup'
Write-Output "Built $setup and $zip ($version)"

if ($Upload) {
    gh release upload "v$version" $setup $zip --clobber
    Check 'gh release upload'
}
