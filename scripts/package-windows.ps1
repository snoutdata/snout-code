# Build SnoutCode's Windows installer: an Inno Setup setup program, unsigned.
#
#   scripts/package-windows.ps1 [-ServerDist <dir>] [-Ghostty] [-Sign]
#
# Runs on Windows, for the machine's own architecture (x64 or arm64), with Rust, a JDK 21 in
# JAVA_HOME (the engine's Java runtime is jlinked from it, so Java is never needed on the user's
# machine) and Inno Setup 6 (ISCC.exe on PATH or in its default folder). Writes to
# target\windows-package\:
#   SnoutCode-<version>-windows-<x64|arm64>-setup.exe   the installer
#   root\                                              the installed tree it is made of
#   SHA256SUMS
#
# -ServerDist names the folder of the Linux servers Remote SSH installs on remote machines
# (scripts/build-server-linux.sh writes them to target\snout-server-dist); without it the
# installer leaves them out. -Ghostty builds the Ghostty terminal engine in (needs Zig 0.15).
#
# -Sign signs snout-code.exe, the installer and its uninstaller with Azure Trusted Signing
# (scripts/windows/sign.ps1, which names what it needs). Off by default: the builds are unsigned
# for now.
param(
    [string]$ServerDist = "",
    [switch]$Ghostty,
    [switch]$Sign
)
$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Out = Join-Path $Root "target\windows-package"
$Work = Join-Path $Out "work"
$Tree = Join-Path $Out "root"
$Version = (Select-String -Path (Join-Path $Root "Cargo.toml") -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$Arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }
Write-Host "== SnoutCode $Version for Windows $Arch"

if (-not $env:JAVA_HOME -or -not (Test-Path "$env:JAVA_HOME\jmods")) {
    throw "JAVA_HOME must name a full JDK 21 (with jmods) to link the engine's Java runtime from."
}
$iscc = (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $iscc = @("${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe", "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe") |
        Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { throw "Inno Setup 6 is not installed (ISCC.exe)." }

Remove-Item -Recurse -Force $Work, $Tree -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Work, $Tree | Out-Null

# --- The program ---------------------------------------------------------------------------

Write-Host "== Building snout-code.exe"
$features = @()
if ($Ghostty) { $features = @("--features", "ghostty") }
$env:CARGO_PROFILE_RELEASE_STRIP = "true"
cargo build --locked --release -p snout-app @features --manifest-path (Join-Path $Root "Cargo.toml")
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root "target" }
Copy-Item (Join-Path $targetDir "release\snout-code.exe") $Tree
$signScript = Join-Path $Root "scripts\windows\sign.ps1"
if ($Sign) { & $signScript (Join-Path $Tree "snout-code.exe") }

# --- The engine ----------------------------------------------------------------------------

Write-Host "== Building the Java engine"
$engineDist = Join-Path $Work "engine-dist"
Push-Location (Join-Path $Root "engine")
try {
    .\gradlew.bat --no-daemon --quiet :sync-cli:snoutDist "-PsnoutDistDir=$engineDist"
    if ($LASTEXITCODE -ne 0) { throw "building the engine failed" }
} finally {
    Pop-Location
}

# What never runs on Windows: jdtls's launch scripts, its feature archives, the configurations
# for macOS and Linux and for jdtls's syntax server, and the native launcher fragments of macOS
# and Linux. The runtime stays whole: it is also the JDK that Gradle, run configurations and the
# debugger fall back to when the machine has none.
$engine = Join-Path $Tree "engine"
robocopy $engineDist $engine /E /NFL /NDL /NJH /NJS /NP `
    /XD (Join-Path $engineDist "jdtls\bin") (Join-Path $engineDist "jdtls\features") `
        (Join-Path $engineDist "jdtls\config_mac") (Join-Path $engineDist "jdtls\config_mac_arm") `
        (Join-Path $engineDist "jdtls\config_linux") (Join-Path $engineDist "jdtls\config_linux_arm") `
        (Join-Path $engineDist "jdtls\config_ss_mac") (Join-Path $engineDist "jdtls\config_ss_mac_arm") `
        (Join-Path $engineDist "jdtls\config_ss_linux") (Join-Path $engineDist "jdtls\config_ss_linux_arm") `
        (Join-Path $engineDist "jdtls\config_ss_win") `
    /XF "org.eclipse.equinox.launcher.cocoa.*" "org.eclipse.equinox.launcher.gtk.*" "snout-engine" | Out-Null
if ($LASTEXITCODE -ge 8) { throw "copying the engine failed (robocopy $LASTEXITCODE)" }
$global:LASTEXITCODE = 0
Copy-Item (Join-Path $Root "engine\NOTICE") (Join-Path $engine "NOTICE")
$check = Start-Process -FilePath (Join-Path $engine "runtime\bin\java.exe") -ArgumentList "-version" -NoNewWindow -Wait -PassThru `
    -RedirectStandardError (Join-Path $Work "java-version.txt")
if ($check.ExitCode -ne 0) { throw "The engine's runtime does not start." }

# --- Remote SSH's servers --------------------------------------------------------------------

if ($ServerDist) {
    $server = Join-Path $Tree "server"
    New-Item -ItemType Directory -Force -Path $server | Out-Null
    foreach ($f in "snout-server-linux-x86_64", "snout-server-linux-aarch64", "SHA256SUMS") {
        Copy-Item (Join-Path $ServerDist $f) $server
    }
} else {
    Write-Host "No -ServerDist: the installer leaves out the Linux servers (Remote SSH asks for them)."
}

Copy-Item (Join-Path $Root "LICENSE") (Join-Path $Tree "LICENSE.txt")
Copy-Item (Join-Path $Root "NOTICE") (Join-Path $Tree "NOTICE.txt")

# --- The installer -------------------------------------------------------------------------

Write-Host "== The installer"
Get-ChildItem $Out -Filter "SnoutCode-*-setup.exe" | Remove-Item -Force
$signArgs = @()
if ($Sign) {
    # Inno Setup signs the setup program and the uninstaller it writes with this command ($f: the file).
    $signArgs = @("/DSignTool=snoutsign", "/Ssnoutsign=pwsh -NoProfile -File `"$signScript`" `$f")
}
& $iscc /Q "/DAppVersion=$Version" "/DArch=$Arch" "/DSourceRoot=$Tree" "/DRepoRoot=$Root" "/O$Out" @signArgs `
    (Join-Path $Root "scripts\windows\snout-code.iss")
if ($LASTEXITCODE -ne 0) { throw "ISCC failed ($LASTEXITCODE)" }
$setup = Get-ChildItem $Out -Filter "SnoutCode-$Version-windows-$Arch-setup.exe" | Select-Object -First 1
if (-not $setup) { throw "no installer was written" }
$hash = (Get-FileHash -Algorithm SHA256 $setup.FullName).Hash.ToLowerInvariant()
"$hash  $($setup.Name)" | Set-Content -Encoding ascii (Join-Path $Out "SHA256SUMS")
Write-Host ("{0} ({1:N1} MB)" -f $setup.FullName, ($setup.Length / 1MB))
