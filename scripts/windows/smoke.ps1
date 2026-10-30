# Start SnoutCode on Windows and check that it works: on a small Maven project, with a scratch
# user profile (its own %APPDATA% and %LOCALAPPDATA%) and no Java on PATH. With -Jdk that JDK is
# where SnoutCode keeps the JDKs it downloads (as if its Download button had fetched it), and
# the Java engine must start on the project; the main window, the File menu of the in-window
# menu bar and Settings are then shot. Without -Jdk the app must open the project and offer to
# download a JDK. Screenshots and the app's log go to -Out.
#
#   smoke.ps1 -Bin <snout-code.exe> -Out <dir> [-Jdk <home>] [-Wait <seconds>]
param(
    [Parameter(Mandatory = $true)][string]$Bin,
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Jdk = "",
    [int]$Wait = 240
)
$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$tag = if ($Jdk) { "jdk" } else { "no-jdk" }
$log = Join-Path $Out "snout-code-$tag.log"

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class SmokeInput {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int cmd);
    public static void Click(int x, int y) {
        SetCursorPos(x, y);
        mouse_event(0x0002, 0, 0, 0, UIntPtr.Zero);
        mouse_event(0x0004, 0, 0, 0, UIntPtr.Zero);
    }
}
"@
[SmokeInput]::SetProcessDPIAware() | Out-Null
# The arm64 runner's desktop shows Windows's first-run privacy screen over everything: out of
# the way, so the app's window can be seen.
Get-Process WWAHost -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

function Shot([string]$name) {
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
    $file = Join-Path $Out "$name.png"
    $bmp.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "screenshot $file ($($bounds.Width)x$($bounds.Height))"
}

# A scratch profile: SnoutCode's folders, the home folder and Java all fresh.
$home_ = Join-Path ([System.IO.Path]::GetTempPath()) ("snout-home-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Force -Path "$home_\AppData\Roaming", "$home_\AppData\Local" | Out-Null
$env:USERPROFILE = $home_
$env:HOME = $null
$env:APPDATA = "$home_\AppData\Roaming"
$env:LOCALAPPDATA = "$home_\AppData\Local"
$env:JAVA_HOME = $null
$env:PATH = (($env:PATH -split ';') | Where-Object { $_ -and $_ -notmatch 'jdk|java' }) -join ';'
if ($Jdk) {
    $jdks = "$home_\AppData\Local\SnoutCode\jdks"
    New-Item -ItemType Directory -Force -Path $jdks | Out-Null
    New-Item -ItemType Junction -Path "$jdks\jdk-21" -Target $Jdk | Out-Null
}

# A Maven project with one class, which the app opens at it.
$project = "$home_\hello"
New-Item -ItemType Directory -Force -Path "$project\src\main\java\demo" | Out-Null
@"
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>demo</groupId>
  <artifactId>hello</artifactId>
  <version>1.0</version>
  <properties>
    <maven.compiler.release>21</maven.compiler.release>
  </properties>
</project>
"@ | Set-Content -Encoding utf8 "$project\pom.xml"
$file = "$project\src\main\java\demo\Hello.java"
@"
package demo;

public class Hello {
    public static void main(String[] args) {
        System.out.println("Hello from SnoutCode on Windows");
    }
}
"@ | Set-Content -Encoding utf8 $file
Push-Location $project
git init -q; git add -A; git -c user.email=t@example.com -c user.name=T commit -qm init
Pop-Location

$env:SNOUT_LOG = "warn"
$env:SNOUT_WINDOW_ORIGIN = "0,0"
$env:SNOUT_NO_FETCH = "1"
$env:SNOUT_STARTUP_TRACE = "1"
Write-Host "== SnoutCode on Windows $(if ($Jdk) { 'with a JDK' }): $Bin"
$app = Start-Process -FilePath $Bin -ArgumentList "`"$file`"" -PassThru `
    -RedirectStandardError $log -RedirectStandardOutput (Join-Path $Out "stdout-$tag.log")
$deadline = (Get-Date).AddSeconds($Wait)

function Fail([string]$why) {
    Write-Host "FAIL ($tag): $why"
    if (Test-Path $log) { Get-Content $log | Where-Object { $_ -notmatch 'UI thread was busy' } | Select-Object -Last 60 }
    try { Shot "failed-$tag" } catch {}
    Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
    exit 1
}

function WaitFor([string]$pattern, [string]$why) {
    while ($true) {
        if ((Test-Path $log) -and (Select-String -Path $log -Pattern $pattern -Quiet)) { return }
        if ($app.HasExited) { Fail "the app exited ($($app.ExitCode))" }
        if ((Get-Date) -gt $deadline) { Fail $why }
        Start-Sleep -Milliseconds 500
    }
}

WaitFor 'main window painted' "no window after $Wait s"
Select-String -Path $log -Pattern 'main window painted' | Select-Object -First 1 | ForEach-Object { Write-Host $_.Line }
# Maximized, so the whole window and its title bar's buttons are on the runner's small screen.
for ($i = 0; $i -lt 20 -and $app.MainWindowHandle -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 250; $app.Refresh() }
[SmokeInput]::ShowWindow($app.MainWindowHandle, 3) | Out-Null
[SmokeInput]::SetForegroundWindow($app.MainWindowHandle) | Out-Null
WaitFor '\[project\] read' "the project was not read"
Select-String -Path $log -Pattern '\[project\] read' | ForEach-Object { Write-Host $_.Line }

if (-not $Jdk) {
    WaitFor '\[java\] no JDK fits' "no JDK download was offered"
    Select-String -Path $log -Pattern '\[java\] no JDK fits' | ForEach-Object { Write-Host $_.Line }
    Start-Sleep -Seconds 2
    Shot "no-jdk"
} else {
    WaitFor '\[java\] started the engine' "the Java engine did not start in $Wait s"
    Select-String -Path $log -Pattern '\[java\] started the engine' | ForEach-Object { Write-Host $_.Line }
    $engine = Get-CimInstance Win32_Process -Filter "Name = 'java.exe'" |
        Where-Object { $_.CommandLine -match 'com.snoutcode.engine.cli.SnoutEngine' } | Select-Object -First 1
    if (-not $engine) { Fail "no engine process" }
    Write-Host "engine: pid $($engine.ProcessId), parent $($engine.ParentProcessId)"
    for ($i = 0; $i -lt 90; $i++) {
        if (Select-String -Path $log -Pattern '\[java\] readiness: .*[Rr]eady|\[java\] first diagnostics' -Quiet) { break }
        Start-Sleep -Seconds 1
    }
    Select-String -Path $log -Pattern '\[java\] readiness|\[java\] first diagnostics' | Select-Object -Last 2 | ForEach-Object { Write-Host $_.Line }
    # What the Java processes listen on: loopback only, or Windows Firewall asks the user.
    $java = @(Get-CimInstance Win32_Process -Filter "Name = 'java.exe'" | ForEach-Object { $_.ProcessId })
    $open = @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue |
        Where-Object { $java -contains $_.OwningProcess -and $_.LocalAddress -notin @('127.0.0.1', '::1') })
    foreach ($c in $open) { Write-Host "listening beyond loopback: java pid $($c.OwningProcess) on $($c.LocalAddress):$($c.LocalPort)" }
    Start-Sleep -Seconds 3
    Shot "main"
    # The window is at the screen's top left; File is the first menu of its title bar.
    [SmokeInput]::Click(31, 18)
    Start-Sleep -Milliseconds 1500
    Shot "menu"
    [System.Windows.Forms.SendKeys]::SendWait("{ESC}")
    Start-Sleep -Milliseconds 500
    [System.Windows.Forms.SendKeys]::SendWait("^,")
    Start-Sleep -Seconds 2
    Shot "settings"
    [System.Windows.Forms.SendKeys]::SendWait("{ESC}")
}

if ($app.HasExited) { Fail "the app exited ($($app.ExitCode))" }

# Closing the window quits (asking first only when something would be lost or stopped): the
# app and the processes it started must go.
$engines = @(Get-CimInstance Win32_Process -Filter "Name = 'java.exe'" |
    Where-Object { $_.CommandLine -match 'com.snoutcode.engine.cli.SnoutEngine' } | ForEach-Object { $_.ProcessId })
Stop-Process -Id $app.Id -Force
Start-Sleep -Seconds 3
$left = @($engines | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
if ($left.Count -gt 0) { Fail "the engine outlived the app: $($left -join ', ')" }
Write-Host "PASS ($tag): SnoutCode opened $project and $(if ($Jdk) { 'started the Java engine' } else { 'offered to download a JDK' }); nothing it started outlived it"
