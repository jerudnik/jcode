param(
    [Parameter(Mandatory = $true)][string]$ArtifactExePath,
    [Parameter(Mandatory = $true)][string]$Version
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$resolvedArtifact = (Resolve-Path -LiteralPath $ArtifactExePath).Path
$tempRoot = Join-Path $env:RUNNER_TEMP ("jcode-windows-install-verify-" + [guid]::NewGuid().ToString('N'))
$localAppData = Join-Path $tempRoot 'localappdata'
$appData = Join-Path $tempRoot 'appdata'
$userProfile = Join-Path $tempRoot 'userprofile'
$jcodeHome = Join-Path $tempRoot '.jcode'
$installDir = Join-Path $localAppData 'jcode\bin'

New-Item -ItemType Directory -Force -Path $localAppData, $appData, $userProfile, $jcodeHome | Out-Null

$env:LOCALAPPDATA = $localAppData
$env:APPDATA = $appData
$env:USERPROFILE = $userProfile
$env:JCODE_HOME = $jcodeHome

$installScript = Join-Path $repoRoot 'scripts\install.ps1'

& $installScript `
    -InstallDir $installDir `
    -Version $Version `
    -ArtifactExePath $resolvedArtifact `
    -SkipAlacrittySetup `
    -SkipHotkeySetup

# F20c: the installer publishes to ONE fixed path. The version store and the
# stable channel it used to assert here were deleted, so asserting them would
# only re-pin state no resolver reads.
$launcherPath = Join-Path $installDir 'jcode.exe'
$publishedPath = Join-Path $localAppData 'jcode\current\jcode.exe'

foreach ($path in @($launcherPath, $publishedPath)) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Expected installed file missing: $path"
    }
}

# The retired layout must not be recreated by the installer.
foreach ($residue in @('jcode\builds\versions', 'jcode\builds\stable', 'jcode\builds\current')) {
    $residuePath = Join-Path $localAppData $residue
    if (Test-Path -LiteralPath $residuePath) {
        throw "Retired distribution layout was recreated: $residuePath"
    }
}

$versionOutput = & $launcherPath --version
if ($LASTEXITCODE -ne 0) {
    throw "Installed launcher failed to run --version"
}

if ($versionOutput -notmatch 'jcode') {
    throw "Installed launcher returned unexpected version output: $versionOutput"
}

& $installScript `
    -InstallDir $installDir `
    -Version $Version `
    -ArtifactExePath $resolvedArtifact `
    -SkipAlacrittySetup `
    -SkipHotkeySetup

if (-not (Test-Path -LiteralPath $launcherPath)) {
    throw "Launcher missing after reinstall: $launcherPath"
}

# Reinstall republishes in place rather than accumulating versions.
if (-not (Test-Path -LiteralPath $publishedPath)) {
    throw "Published binary missing after reinstall: $publishedPath"
}

Write-Host "Windows install verification passed for $Version" -ForegroundColor Green
