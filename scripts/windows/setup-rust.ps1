# Rust for a GitHub Actions Windows runner: rustup where the image has none (the arm64 image),
# then the stable toolchain with clippy and rustfmt for this machine's architecture.
$ErrorActionPreference = "Stop"
$arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64" } else { "x86_64" }
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    $init = Join-Path $env:RUNNER_TEMP "rustup-init.exe"
    Invoke-WebRequest "https://static.rust-lang.org/rustup/dist/$arch-pc-windows-msvc/rustup-init.exe" -OutFile $init
    & $init -y --no-modify-path --profile minimal --default-toolchain none
    if ($LASTEXITCODE -ne 0) { throw "rustup-init failed" }
    $env:PATH = "$cargoBin;$env:PATH"
    $cargoBin >> $env:GITHUB_PATH
}
rustup toolchain install stable --profile minimal --component clippy,rustfmt --target "$arch-pc-windows-msvc"
if ($LASTEXITCODE -ne 0) { throw "rustup failed" }
rustup default stable
rustc --version
