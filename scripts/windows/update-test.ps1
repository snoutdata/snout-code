# A real update on Windows: install SnoutCode, run it, and let the waiter put the new version in
# place through the real installer, as a Restart does. The program must be replaced, what was lost
# must be back, the waiter must end, its lock must be lifted, and the app must come back as the real
# app, not as a process that shows nothing. Then a launch must not be held by a lock left behind.
#
#   update-test.ps1 -Setup <SnoutCode-…-setup.exe> [-Out <dir>]
#
# The waiter runs from a real copy of the program. From a hard link to it, as in 1.0.0 and 1.0.1, the
# installer killed the waiter a few seconds in, and the waiter's lock stayed behind.
param(
    [Parameter(Mandatory = $true)][string]$Setup,
    [string]$Out = "target\windows-install-test\update"
)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$Setup = (Resolve-Path $Setup).Path

$dir = Join-Path $env:LOCALAPPDATA "Programs\SnoutCode"
$exe = Join-Path $dir "snout-code.exe"
$updates = Join-Path $env:LOCALAPPDATA "SnoutCode\cache\updates"
$lock = Join-Path $updates "swapping"
$logs = Join-Path $env:LOCALAPPDATA "SnoutCode\logs"
$log = Join-Path $logs "update.log"
$installerLog = Join-Path $logs "update-installer.log"
# The real app is far past this; a process waiting and showing nothing stays well under it.
$realAppBytes = 30MB

function ShowWhatHappened {
    foreach ($f in $log, $installerLog) {
        if (Test-Path $f) {
            Write-Host "--- $f"
            Get-Content $f -Tail 25 | ForEach-Object { Write-Host $_ }
            Copy-Item $f $Out -Force
        }
    }
    Write-Host "--- processes"
    Get-Process | Where-Object { $_.ProcessName -like "snout*" -or $_.ProcessName -like "*setup*" -or $_.ProcessName -like "*.tmp" } |
        ForEach-Object { Write-Host "$($_.ProcessName) $($_.Id) $([int]($_.WorkingSet64 / 1MB)) MB $($_.Path)" }
}
function Fail([string]$why) { Write-Host "FAIL: $why"; ShowWhatHappened; StopAll; exit 1 }
function StopAll {
    Get-Process | Where-Object { $_.ProcessName -like "snout-code*" } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}
function WaitFor([int]$seconds, [scriptblock]$done) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while (-not (& $done)) {
        if ((Get-Date) -gt $deadline) { return $false }
        Start-Sleep -Milliseconds 500
    }
    return $true
}
# The app as a real app: running from the install folder, and well past what a waiting process uses.
function RealApp {
    Get-Process -Name "snout-code" -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $exe -and $_.WorkingSet64 -gt $realAppBytes } | Select-Object -First 1
}
function Install {
    $p = Start-Process -FilePath $Setup -Wait -PassThru -ArgumentList @(
        "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/LOG=`"$Out\install.log`"")
    if ($p.ExitCode -ne 0) { Fail "the first install exited with $($p.ExitCode)" }
}

StopAll
Remove-Item $updates -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $log, $installerLog -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $updates, $logs | Out-Null
Write-Host "== Installing"
Install

# Two ways to tell the installer really ran: the program changed (a byte on the end of a program
# changes nothing about how it runs), and a file taken away.
Add-Content -Path $exe -Value "x" -NoNewline
$before = (Get-FileHash $exe).Hash
$lost = Get-ChildItem $dir -File | Where-Object { $_.Extension -notin ".exe", ".dat", ".msg" } | Select-Object -First 1
if (-not $lost) { Fail "no file to take away in $dir" }
$lostPath = $lost.FullName
Remove-Item $lostPath

Write-Host "== Starting the app"
$env:SNOUT_NO_UPDATE = "1"
$env:SNOUT_NO_FETCH = "1"
$app = Start-Process -FilePath $exe -PassThru
if (-not (WaitFor 90 { RealApp })) { Fail "the app did not start" }

# What a quit does: the waiter's program and its plan, then the app goes.
$program = Join-Path $updates "waiter\snout-code-update.exe"
New-Item -ItemType Directory -Force -Path (Split-Path $program) | Out-Null
Copy-Item $exe $program
$installer = Join-Path $updates (Split-Path $Setup -Leaf)
Copy-Item $Setup $installer
$config = Join-Path $updates "waiter.json"
@{
    app_pid = $app.Id
    mode = "Restart"
    version = "update-test"
    plan = @{ Installer = @{ installer = $installer; folder = $dir; exe = $exe } }
    updates = $updates
    log = $log
    direct_launch = $true
} | ConvertTo-Json -Depth 5 | Set-Content -Path $config -Encoding utf8NoBOM

Write-Host "== The update"
$env:SNOUT_UPDATE_WAITER = $config
$waiter = Start-Process -FilePath $program -PassThru
$env:SNOUT_UPDATE_WAITER = $null
Stop-Process -Id $app.Id -Force
$started = Get-Date
$waiterEnded = WaitFor 300 { $waiter.HasExited }
Write-Host "the waiter ended: $waiterEnded, after $([int]((Get-Date) - $started).TotalSeconds) s"
if (-not $waiterEnded) { Fail "the waiter never ended: the update hung" }

if (Test-Path $lock) { Fail "the lock is still there after the waiter ended" }
if (-not ((Get-Content $log -Raw) -match "exited: Some\(0\)")) { Fail "the installer did not succeed" }
if ((Get-FileHash $exe).Hash -eq $before) { Fail "the program was not replaced" }
if (-not (Test-Path $lostPath)) { Fail "what was lost did not come back: $lostPath" }
if (-not (WaitFor 90 { RealApp })) { Fail "the app did not come back as the real app" }
Write-Host "the update landed, and the app came back"
StopAll

# A lock left behind, by a waiter that was killed or by an older version, which wrote nothing in
# it: the app must start at once, not sit there showing nothing.
foreach ($heldBy in "", "999999") {
    Write-Host "== A launch with a lock left behind (`"$heldBy`")"
    Set-Content -Path $lock -Value $heldBy -NoNewline
    Start-Process -FilePath $exe | Out-Null
    if (-not (WaitFor 60 { RealApp })) { Fail "a lock nobody holds kept the app from starting" }
    if (Test-Path $lock) { Fail "the lock nobody holds was not removed" }
    StopAll
}

ShowWhatHappened
$env:SNOUT_NO_UPDATE = $null
$env:SNOUT_NO_FETCH = $null
$p = Start-Process -FilePath (Join-Path $dir "unins000.exe") -Wait -PassThru -ArgumentList @(
    "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART")
if ($p.ExitCode -ne 0) { Fail "the uninstaller exited with $($p.ExitCode)" }
WaitFor 60 { -not (Test-Path $dir) } | Out-Null
Write-Host "PASS"
