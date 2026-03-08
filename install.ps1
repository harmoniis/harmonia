# Harmonia installer for Windows — https://harmoniis.com/harmonia/install.ps1
# Usage: iwr https://harmoniis.com/harmonia/install.ps1 -UseB | iex
$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "  Harmonia — Self-improving Common Lisp + Rust agent" -ForegroundColor Cyan
Write-Host ""

# Check / install Rust
if (Get-Command cargo -ErrorAction SilentlyContinue) {
    $rv = (rustc --version) -replace 'rustc ', ''
    Write-Host "  [ok] Rust $rv" -ForegroundColor Green
} else {
    Write-Host "  [->] Installing Rust..." -ForegroundColor Yellow
    $rustup = "$env:TEMP\rustup-init.exe"
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustup
    & $rustup -y
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    Write-Host "  [ok] Rust installed" -ForegroundColor Green
}

# Check / guide SBCL
if (Get-Command sbcl -ErrorAction SilentlyContinue) {
    Write-Host "  [ok] SBCL found" -ForegroundColor Green
} else {
    Write-Host "  [!!] SBCL not found." -ForegroundColor Red
    Write-Host ""
    Write-Host "  Install SBCL using one of:" -ForegroundColor Yellow
    Write-Host "    scoop install sbcl"
    Write-Host "    choco install sbcl"
    Write-Host "    https://www.sbcl.org/platform-table.html"
    Write-Host ""
    Write-Host "  After installing SBCL, re-run this script."
    exit 1
}

# Check / install Quicklisp
$qlPath = "$env:USERPROFILE\quicklisp\setup.lisp"
if (Test-Path $qlPath) {
    Write-Host "  [ok] Quicklisp" -ForegroundColor Green
} else {
    Write-Host "  [->] Installing Quicklisp..." -ForegroundColor Yellow
    $qlTemp = "$env:TEMP\quicklisp.lisp"
    Invoke-WebRequest -Uri "https://beta.quicklisp.org/quicklisp.lisp" -OutFile $qlTemp
    & sbcl --non-interactive --load $qlTemp --eval "(quicklisp-quickstart:install)" --eval "(ql:add-to-init-file)"
    Remove-Item $qlTemp -ErrorAction SilentlyContinue
    if (Test-Path $qlPath) {
        Write-Host "  [ok] Quicklisp installed" -ForegroundColor Green
    } else {
        Write-Host "  [!!] Quicklisp installation failed" -ForegroundColor Red
        exit 1
    }
}

# Install Harmonia (clone source)
$harmoniaRepo = if ($env:HARMONIA_REPO) { $env:HARMONIA_REPO } else { "https://github.com/harmoniis/harmonia.git" }
$harmoniaSrc = "$env:USERPROFILE\.harmoniis\harmonia\src"

if (Test-Path "$harmoniaSrc\.git") {
    Write-Host "  [->] Updating existing source..." -ForegroundColor Yellow
    Push-Location $harmoniaSrc
    git pull --ff-only
    Pop-Location
} else {
    Write-Host "  [->] Cloning Harmonia from $harmoniaRepo..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Force -Path (Split-Path $harmoniaSrc) | Out-Null
    git clone $harmoniaRepo $harmoniaSrc
}

Write-Host "  [->] Building Harmonia..." -ForegroundColor Yellow
Push-Location $harmoniaSrc
cargo build --release
Pop-Location

# Add to PATH via symlink or copy
$binDir = "$env:USERPROFILE\.local\bin"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
Copy-Item "$harmoniaSrc\target\release\harmonia.exe" "$binDir\harmonia.exe" -Force
if ($env:PATH -notlike "*$binDir*") {
    Write-Host "  [!!] Add $binDir to your PATH" -ForegroundColor Yellow
}
Write-Host "  [ok] Harmonia installed" -ForegroundColor Green

# Run setup
Write-Host ""
harmonia setup
