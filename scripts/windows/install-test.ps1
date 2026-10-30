# Install SnoutCode with its installer, silently and for the current user, try it, and
# uninstall it. The program, the engine with its Java runtime, the Start menu entry and the
# uninstaller must be where they belong; the installed app must open a project, start its Java
# engine from the installed engine folder and show its window (smoke.ps1, screenshots in -Out);
# and the uninstaller must take all it installed away again, leaving the user's settings.
#
#   install-test.ps1 -Setup <SnoutCode-…-setup.exe> -Jdk <JDK 21 home> [-Out <dir>]
param(
    [Parameter(Mandatory = $true)][string]$Setup,
    [Parameter(Mandatory = $true)][string]$Jdk,
    [string]$Out = "target\windows-install-test"
)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path

$dir = Join-Path $env:LOCALAPPDATA "Programs\SnoutCode"
$menu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\SnoutCode.lnk"
$uninstallKeys = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall"
function Fail([string]$why) { Write-Host "FAIL: $why"; exit 1 }
function UninstallEntry {
    Get-ChildItem $uninstallKeys | Where-Object { $_.GetValue("DisplayName") -eq "SnoutCode" } | Select-Object -First 1
}

Write-Host "== Installing $Setup"
$p = Start-Process -FilePath $Setup -Wait -PassThru -ArgumentList @(
    "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/LOG=`"$Out\install.log`"")
if ($p.ExitCode -ne 0) { Get-Content "$Out\install.log" -Tail 30; Fail "the installer exited with $($p.ExitCode)" }
foreach ($f in "snout-code.exe", "engine\runtime\bin\java.exe", "engine\lib", "engine\jdtls", "server", "unins000.exe") {
    if (-not (Test-Path (Join-Path $dir $f))) { Fail "$f is not in $dir" }
}
if (-not (Test-Path $menu)) { Fail "no Start menu entry at $menu" }
$key = UninstallEntry
if (-not $key) { Fail "SnoutCode is not in Settings > Apps" }
$files = @(Get-ChildItem $dir -Recurse -File).Count
Write-Host "installed in $dir ($files files), version $($key.GetValue('DisplayVersion'))"

Write-Host "== Starting it"
# Its own process: smoke.ps1 gives itself a scratch profile through the environment.
pwsh -NoProfile -File (Join-Path $PSScriptRoot "smoke.ps1") -Bin (Join-Path $dir "snout-code.exe") -Out $Out -Jdk $Jdk
if ($LASTEXITCODE -ne 0) { Fail "the installed app did not work" }
$left = @(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($dir, 'OrdinalIgnoreCase') })
if ($left.Count -gt 0) { Fail "still running from $dir after the app ended: $($left.ExecutablePath -join ', ')" }

# The user's settings, which uninstalling keeps (docs/windows.md).
$settings = Join-Path $env:APPDATA "SnoutCode"
$mine = -not (Test-Path $settings)
New-Item -ItemType Directory -Force -Path $settings | Out-Null
$marker = Join-Path $settings "install-test-kept.txt"
Set-Content -Path $marker -Value "kept"

Write-Host "== Uninstalling"
$p = Start-Process -FilePath (Join-Path $dir "unins000.exe") -Wait -PassThru -ArgumentList @(
    "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/LOG=`"$Out\uninstall.log`"")
if ($p.ExitCode -ne 0) { Fail "the uninstaller exited with $($p.ExitCode)" }
# The uninstaller runs on from a copy of itself in the temporary folder, and removes the
# folder last.
$deadline = (Get-Date).AddSeconds(60)
while ((Test-Path $dir) -or (Test-Path $menu) -or (UninstallEntry)) {
    if ((Get-Date) -gt $deadline) {
        if (Test-Path $dir) { Get-ChildItem $dir -Recurse | ForEach-Object { Write-Host "left: $($_.FullName)" } }
        if (Test-Path $menu) { Write-Host "left: $menu" }
        if (UninstallEntry) { Write-Host "left: the entry in Settings > Apps" }
        Fail "the uninstaller left SnoutCode behind"
    }
    Start-Sleep -Milliseconds 500
}
if (-not (Test-Path $marker)) { Fail "the uninstaller took the user's settings ($settings)" }
Remove-Item $marker
if ($mine) { Remove-Item $settings -Recurse -Force }
Write-Host "PASS: installed, started (window, project, engine) and uninstalled; the settings stayed"
