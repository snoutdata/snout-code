# The installed app opens a folder inside WSL: it must connect to the distro, install its server
# there, and open the project on the Linux side, with the window on Windows.
#
#   wsl-test.ps1 -Setup <SnoutCode-…-setup.exe> -Distro <distro> [-Out <dir>]
param(
    [Parameter(Mandatory = $true)][string]$Setup,
    [Parameter(Mandatory = $true)][string]$Distro,
    [string]$Out = "target\windows-install-test\wsl"
)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Setup = (Resolve-Path $Setup).Path
$dir = Join-Path $env:LOCALAPPDATA "Programs\SnoutCode"
$exe = Join-Path $dir "snout-code.exe"
$recents = Join-Path $env:APPDATA "SnoutCode\remote.json"

function InDistro([string]$line) { wsl.exe -d $Distro -e sh -c $line }
function StopAll {
    Get-Process | Where-Object { $_.ProcessName -like "snout-code*" } | Stop-Process -Force -ErrorAction SilentlyContinue
    # The bracket keeps the pattern from matching this shell's own command line.
    InDistro "pkill -f '[s]nout-server' || true" | Out-Null
}
function Show {
    Write-Host "--- remote.json"
    if (Test-Path $recents) { Get-Content $recents | ForEach-Object { Write-Host $_ } }
    Write-Host "--- server in the distro"
    InDistro "ls -la ~/.snout-code/server/*/ 2>&1; pgrep -af '[s]nout-server'" | ForEach-Object { Write-Host $_ }
    Write-Host "--- processes"
    Get-Process | Where-Object { $_.ProcessName -like "snout*" } |
        ForEach-Object { Write-Host "$($_.ProcessName) $($_.Id) $([int]($_.WorkingSet64 / 1MB)) MB" }
}
function Fail([string]$why) { Write-Host "FAIL: $why"; Show; StopAll; exit 1 }
function WaitFor([int]$seconds, [scriptblock]$done) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while (-not (& $done)) {
        if ((Get-Date) -gt $deadline) { return $false }
        Start-Sleep -Seconds 1
    }
    return $true
}

StopAll
$p = Start-Process -FilePath $Setup -Wait -PassThru -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER")
if ($p.ExitCode -ne 0) { Fail "the install exited with $($p.ExitCode)" }
Remove-Item $recents -Force -ErrorAction SilentlyContinue
# The console runs Amp when the window opens, as a user who left it open would have it. Put back after.
$settings = Join-Path $env:APPDATA "SnoutCode\settings.json"
$kept = if (Test-Path $settings) { Get-Content $settings -Raw } else { $null }
New-Item -ItemType Directory -Force -Path (Split-Path $settings) | Out-Null
Set-Content -Path $settings -Value '{"console_open":true,"console_agent":"amp"}'
InDistro "rm -f /tmp/harness-answer" | Out-Null
InDistro "rm -rf ~/.snout-code; mkdir -p /tmp/wsl-demo; echo hello > /tmp/wsl-demo/notes.txt" | Out-Null

Write-Host "== Opening \\wsl.localhost\$Distro\tmp\wsl-demo"
$env:SNOUT_NO_UPDATE = "1"
$env:SNOUT_NO_FETCH = "1"
Start-Process -FilePath $exe -ArgumentList "\\wsl.localhost\$Distro\tmp\wsl-demo" | Out-Null

if (-not (WaitFor 180 { (InDistro "pgrep -f '[s]nout-server --daemon'") })) { Fail "the server never started in the distro" }
Write-Host "the server runs in $Distro"
if (-not (WaitFor 60 { (Test-Path $recents) -and ((Get-Content $recents -Raw) -match "/tmp/wsl-demo") })) {
    Fail "the project was not opened on the Linux side"
}
$opened = Get-Content $recents -Raw | ConvertFrom-Json
if ($opened.projects[0].host -ne "WSL: $Distro") { Fail "it opened on $($opened.projects[0].host)" }
if (-not (Get-Process -Name "snout-code" -ErrorAction SilentlyContinue | Where-Object { $_.WorkingSet64 -gt 30MB })) {
    Fail "the app is not running as the real app"
}
# An agent in the distro reaches the window: the API's port is forwarded through wsl.exe.
if (-not (WaitFor 120 { InDistro "grep -q wsl-demo /tmp/harness-answer && grep -q showToolWindow /tmp/harness-answer && echo yes" })) {
    InDistro "cat /tmp/harness-answer 2>&1" | ForEach-Object { Write-Host $_ }
    Fail "the agent in the distro did not reach the window"
}
Write-Host "the agent in $Distro reached the window"
InDistro "cut -c1-300 /tmp/harness-answer" | ForEach-Object { Write-Host $_ }
Show
StopAll
if ($null -ne $kept) { Set-Content -Path $settings -Value $kept } else { Remove-Item $settings -Force -ErrorAction SilentlyContinue }
$env:SNOUT_NO_UPDATE = $null
$env:SNOUT_NO_FETCH = $null
Start-Process -FilePath (Join-Path $dir "unins000.exe") -Wait -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART") | Out-Null
Write-Host "PASS"
# What ran last may have ended the server's shell, which is no failure of this test.
exit 0
