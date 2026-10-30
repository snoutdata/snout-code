# Install SnoutCode with its installer, silently and for the current user, start it, and
# uninstall it: the program, the engine with its Java runtime, the Start menu entry and the
# uninstaller must be where they belong, the app must open its window, and the uninstaller must
# take it all away again.
#
#   install-test.ps1 -Setup <SnoutCode-…-setup.exe> [-Out <dir>]
param(
    [Parameter(Mandatory = $true)][string]$Setup,
    [string]$Out = "target\windows-install-test"
)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path

$dir = Join-Path $env:LOCALAPPDATA "Programs\SnoutCode"
$menu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\SnoutCode.lnk"
function Fail([string]$why) { Write-Host "FAIL: $why"; exit 1 }

Write-Host "== Installing $Setup"
$p = Start-Process -FilePath $Setup -Wait -PassThru -ArgumentList @(
    "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/LOG=`"$Out\install.log`"")
if ($p.ExitCode -ne 0) { Get-Content "$Out\install.log" -Tail 30; Fail "the installer exited with $($p.ExitCode)" }
foreach ($f in "snout-code.exe", "engine\runtime\bin\java.exe", "engine\lib", "engine\jdtls", "unins000.exe") {
    if (-not (Test-Path (Join-Path $dir $f))) { Fail "$f is not in $dir" }
}
if (-not (Test-Path $menu)) { Fail "no Start menu entry at $menu" }
$key = Get-ChildItem "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall" |
    Where-Object { $_.GetValue("DisplayName") -eq "SnoutCode" } | Select-Object -First 1
if (-not $key) { Fail "SnoutCode is not in Settings > Apps" }
Write-Host "installed in $dir, version $($key.GetValue('DisplayVersion'))"

Write-Host "== Starting it"
$home_ = Join-Path ([System.IO.Path]::GetTempPath()) ("snout-install-home-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Force -Path "$home_\Roaming", "$home_\Local" | Out-Null
$saved = @{ APPDATA = $env:APPDATA; LOCALAPPDATA = $env:LOCALAPPDATA }
$env:APPDATA = "$home_\Roaming"
$env:LOCALAPPDATA = "$home_\Local"
$env:SNOUT_STARTUP_TRACE = "1"
$env:SNOUT_NO_FETCH = "1"
$log = Join-Path $Out "installed-app.log"
$app = Start-Process -FilePath (Join-Path $dir "snout-code.exe") -PassThru -RedirectStandardError $log `
    -RedirectStandardOutput (Join-Path $Out "installed-app-stdout.log")
$env:APPDATA = $saved.APPDATA
$env:LOCALAPPDATA = $saved.LOCALAPPDATA
$deadline = (Get-Date).AddSeconds(120)
while (-not ((Test-Path $log) -and (Select-String -Path $log -Pattern 'main window painted' -Quiet))) {
    if ($app.HasExited) { Get-Content $log -Tail 40; Fail "the installed app exited ($($app.ExitCode))" }
    if ((Get-Date) -gt $deadline) { Get-Content $log -Tail 40; Fail "the installed app showed no window" }
    Start-Sleep -Milliseconds 500
}
Select-String -Path $log -Pattern 'main window painted' | Select-Object -First 1 | ForEach-Object { Write-Host $_.Line }
Start-Sleep -Seconds 3
Stop-Process -Id $app.Id -Force
Start-Sleep -Seconds 2

Write-Host "== Uninstalling"
$p = Start-Process -FilePath (Join-Path $dir "unins000.exe") -Wait -PassThru -ArgumentList @(
    "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/LOG=`"$Out\uninstall.log`"")
if ($p.ExitCode -ne 0) { Fail "the uninstaller exited with $($p.ExitCode)" }
# The uninstaller runs on from a copy of itself in the temporary folder.
$deadline = (Get-Date).AddSeconds(60)
while ((Test-Path (Join-Path $dir "snout-code.exe")) -or (Test-Path $menu)) {
    if ((Get-Date) -gt $deadline) { Fail "the uninstaller left SnoutCode behind" }
    Start-Sleep -Milliseconds 500
}
if (Test-Path (Join-Path $dir "engine")) { Fail "the uninstaller left the engine behind" }
Write-Host "PASS: installed, started and uninstalled"
