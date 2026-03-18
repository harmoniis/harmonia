param(
    [ValidateSet("full-agent", "tui-client")]
    [string]$Profile = $(if ($env:HARMONIA_INSTALL_PROFILE) { $env:HARMONIA_INSTALL_PROFILE } else { "full-agent" })
)

$ErrorActionPreference = "Stop"
$env:HARMONIA_INSTALL_PROFILE = $Profile

function Write-Section([string]$Message, [string]$Color = "Cyan") {
    Write-Host "  $Message" -ForegroundColor $Color
}

function Write-Step([string]$Message) {
    Write-Section "[->] $Message" "Yellow"
}

function Write-Ok([string]$Message) {
    Write-Section "[ok] $Message" "Green"
}

function Write-Warn([string]$Message) {
    Write-Section "[!!] $Message" "Yellow"
}

function Get-LocalAppDataRoot {
    if ($env:LOCALAPPDATA) {
        return $env:LOCALAPPDATA
    }
    return Join-Path $env:USERPROFILE "AppData\Local"
}

function Get-InstallPaths {
    $localRoot = Get-LocalAppDataRoot
    return @{
        DataDir = if ($env:HARMONIA_DATA_DIR) { $env:HARMONIA_DATA_DIR } else { Join-Path $env:USERPROFILE ".harmoniis\harmonia" }
        BinDir = if ($env:HARMONIA_BIN_DIR) { $env:HARMONIA_BIN_DIR } else { Join-Path $localRoot "Harmonia\bin" }
        LibDir = if ($env:HARMONIA_LIB_DIR) { $env:HARMONIA_LIB_DIR } else { Join-Path $localRoot "Harmonia\lib" }
        ShareDir = if ($env:HARMONIA_SHARE_DIR) { $env:HARMONIA_SHARE_DIR } else { Join-Path $localRoot "Harmonia\share" }
        SourceCheckout = if ($env:HARMONIA_SOURCE_ROOT) { $env:HARMONIA_SOURCE_ROOT } else { Join-Path $localRoot "Harmonia\source-checkout" }
    }
}

function Get-RoleForProfile([string]$InstallProfile) {
    if ($InstallProfile -eq "tui-client") {
        return "tui-client"
    }
    return "agent"
}

function Get-NodeLabel {
    if ($env:HARMONIA_NODE_LABEL) {
        return ($env:HARMONIA_NODE_LABEL.ToLowerInvariant() -replace '[^a-z0-9._-]+', '-').Trim('.','_','-')
    }
    $raw = if ($env:COMPUTERNAME) { $env:COMPUTERNAME } else { "harmonia-node" }
    $label = ($raw.ToLowerInvariant() -replace '[^a-z0-9._-]+', '-').Trim('.','_','-')
    if ([string]::IsNullOrWhiteSpace($label)) {
        return "harmonia-node"
    }
    return $label
}

function Ensure-Dir([string]$Path) {
    New-Item -ItemType Directory -Force -Path $Path | Out-Null
}

function Copy-DirContents([string]$Source, [string]$Destination) {
    if (-not (Test-Path $Source)) {
        return
    }
    Ensure-Dir $Destination
    Get-ChildItem -Force $Source | ForEach-Object {
        Copy-Item $_.FullName $Destination -Recurse -Force
    }
}

function Add-UserPathEntry([string]$BinDir) {
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $entries = @()
    if ($userPath) {
        $entries = $userPath.Split(';') | Where-Object { $_ }
    }
    if ($entries -notcontains $BinDir) {
        $newUserPath = if ($userPath -and $userPath.Trim().Length -gt 0) {
            "$userPath;$BinDir"
        } else {
            $BinDir
        }
        [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
    }
    $sessionEntries = $env:Path.Split(';') | Where-Object { $_ }
    if ($sessionEntries -notcontains $BinDir) {
        $env:Path = "$BinDir;$env:Path"
    }
}

function Check-Rust {
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        $rv = (rustc --version) -replace '^rustc ', ''
        Write-Ok "Rust $rv"
        return
    }

    Write-Step "Installing Rust..."
    $rustup = Join-Path $env:TEMP "rustup-init.exe"
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustup
    & $rustup -y
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    Write-Ok "Rust installed"
}

function Check-Sbcl {
    if (Get-Command sbcl -ErrorAction SilentlyContinue) {
        Write-Ok "SBCL found"
        return
    }

    Write-Section "[!!] SBCL not found." "Red"
    Write-Host ""
    Write-Host "  Install SBCL using one of:" -ForegroundColor Yellow
    Write-Host "    scoop install sbcl"
    Write-Host "    choco install sbcl"
    Write-Host "    https://www.sbcl.org/platform-table.html"
    Write-Host ""
    throw "SBCL is required"
}

function Check-Quicklisp {
    $qlPath = Join-Path $env:USERPROFILE "quicklisp\setup.lisp"
    if (Test-Path $qlPath) {
        Write-Ok "Quicklisp"
        return
    }

    Write-Step "Installing Quicklisp..."
    $qlTemp = Join-Path $env:TEMP "quicklisp.lisp"
    Invoke-WebRequest -Uri "https://beta.quicklisp.org/quicklisp.lisp" -OutFile $qlTemp
    & sbcl --non-interactive --load $qlTemp --eval "(quicklisp-quickstart:install)" --eval "(ql:add-to-init-file)" | Out-Null
    Remove-Item $qlTemp -ErrorAction SilentlyContinue
    if (-not (Test-Path $qlPath)) {
        throw "Quicklisp installation failed"
    }
    Write-Ok "Quicklisp installed"
}

function Install-Binaries([string]$ReleaseDir, [string]$BinDir) {
    Ensure-Dir $BinDir
    Remove-Item (Join-Path $BinDir "harmonia.exe") -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $BinDir "harmonia-phoenix.exe") -Force -ErrorAction SilentlyContinue

    $binMappings = @(
        @{ Source = "harmonia.exe"; Dest = "harmonia.exe" },
        @{ Source = "harmonia-phoenix.exe"; Dest = "harmonia-phoenix.exe" },
        @{ Source = "phoenix.exe"; Dest = "harmonia-phoenix.exe" }
    )

    foreach ($mapping in $binMappings) {
        $source = Join-Path $ReleaseDir $mapping.Source
        if (Test-Path $source) {
            Copy-Item $source (Join-Path $BinDir $mapping.Dest) -Force
        }
    }

    if (-not (Test-Path (Join-Path $BinDir "harmonia.exe"))) {
        throw "harmonia.exe missing after install"
    }
}

function Install-Libraries([string]$ReleaseDir, [string]$LibDir) {
    Ensure-Dir $LibDir
    Get-ChildItem $LibDir -Filter "*.dll" -ErrorAction SilentlyContinue | Where-Object {
        $_.Name -like "libharmonia_*" -or $_.Name -like "harmonia_*"
    } | Remove-Item -Force -ErrorAction SilentlyContinue

    $libs = Get-ChildItem $ReleaseDir -Filter "*.dll" | Where-Object {
        ($_.Name -like "libharmonia_*" -or $_.Name -like "harmonia_*") -and
        # iMessage (BlueBubbles) only works on macOS — skip on Windows
        $_.Name -notlike "*imessage*"
    }
    if (-not $libs) {
        throw "no runtime libraries found in $ReleaseDir"
    }

    foreach ($lib in $libs) {
        Copy-Item $lib.FullName $LibDir -Force
    }
}

function Install-ShareTree([string]$Root, [string]$ShareDir) {
    Ensure-Dir $ShareDir
    foreach ($name in @("src", "config", "doc")) {
        $target = Join-Path $ShareDir $name
        if (Test-Path $target) {
            Remove-Item $target -Recurse -Force
        }
        Copy-DirContents (Join-Path $Root $name) $target
    }
}

function Ensure-DataDirs([string]$DataDir) {
    Ensure-Dir $DataDir
    Ensure-Dir (Join-Path $DataDir "state")
    Ensure-Dir (Join-Path $DataDir "frontends")
    Ensure-Dir (Join-Path $DataDir "config")
    Ensure-Dir (Join-Path $DataDir "nodes")
}

function Write-NodeIdentity([string]$DataDir) {
    $label = Get-NodeLabel
    $role = Get-RoleForProfile $Profile
    $nodeDir = Join-Path (Join-Path $DataDir "nodes") $label
    Ensure-Dir $nodeDir
    Ensure-Dir (Join-Path $nodeDir "sessions")
    Ensure-Dir (Join-Path $nodeDir "pairings")
    Ensure-Dir (Join-Path $nodeDir "memory")
    $identity = @{
        label = $label
        hostname = $label
        role = $role
        install_profile = $Profile
    } | ConvertTo-Json
    Set-Content -Path (Join-Path $DataDir "config\node.json") -Value $identity
    Set-Content -Path (Join-Path $nodeDir "node.json") -Value $identity
}

function Install-FromArtifactRoot([string]$ArtifactRoot, [string]$Version) {
    $paths = Get-InstallPaths

    Write-Step "Installing Harmonia v$Version"
    Write-Host "    profile:   $Profile"
    Write-Host "    user data: $($paths.DataDir)"
    Write-Host "    binaries:  $($paths.BinDir)"
    Write-Host "    libraries: $($paths.LibDir)"
    Write-Host "    shared:    $($paths.ShareDir)"

    Install-Binaries (Join-Path $ArtifactRoot "bin") $paths.BinDir
    Install-Libraries (Join-Path $ArtifactRoot "lib") $paths.LibDir
    Install-ShareTree $ArtifactRoot $paths.ShareDir
    Ensure-DataDirs $paths.DataDir
    Write-NodeIdentity $paths.DataDir
    Add-UserPathEntry $paths.BinDir

    $versionText = & (Join-Path $paths.BinDir "harmonia.exe") --version 2>$null
    Write-Ok "Harmonia installed"
    Write-Host "    version:   $versionText"
    Write-Host "    binary:    $(Join-Path $paths.BinDir 'harmonia.exe')"
    Write-Host "    libraries: $($paths.LibDir)"
    Write-Host "    shared:    $($paths.ShareDir)"
    Write-Host "    user data: $($paths.DataDir)"
    Write-Host "    profile:   $Profile"
}

function Get-RepoVersion([string]$RepoRoot) {
    $cargoToml = Join-Path $RepoRoot "Cargo.toml"
    if (-not (Test-Path $cargoToml)) {
        throw "missing Cargo.toml at $RepoRoot"
    }
    $line = Select-String -Path $cargoToml -Pattern '^version = "([^"]+)"' | Select-Object -First 1
    if (-not $line) {
        throw "could not determine version from $cargoToml"
    }
    return $line.Matches[0].Groups[1].Value
}

function Install-FromLocalRepo([string]$RepoRoot) {
    $version = Get-RepoVersion $RepoRoot
    Write-Step "Building Harmonia v$version from local repo..."
    Push-Location $RepoRoot
    try {
        cargo build --workspace --release
    } finally {
        Pop-Location
    }

    Install-FromArtifactRoot $RepoRoot $version
}

function Get-LatestReleaseVersion {
    $repo = if ($env:HARMONIA_REPO) { $env:HARMONIA_REPO } else { "harmoniis/harmonia" }
    $apiUrl = if ($repo -match '^https://github\.com/(.+?)(?:\.git)?/?$') {
        "https://api.github.com/repos/$($Matches[1])/releases/latest"
    } else {
        "https://api.github.com/repos/$repo/releases/latest"
    }
    $release = Invoke-RestMethod -Uri $apiUrl
    return $release.tag_name -replace '^harmonia-v', ''
}

function Download-ReleaseArtifact([string]$Version) {
    $repo = if ($env:HARMONIA_REPO) { $env:HARMONIA_REPO } else { "https://github.com/harmoniis/harmonia" }
    if ($repo -notmatch '^https://') {
        $repo = "https://github.com/$repo"
    }
    $repo = $repo -replace '\.git$', ''
    $tmpRoot = Join-Path $env:TEMP ("harmonia-install-" + [guid]::NewGuid().ToString("N"))
    Ensure-Dir $tmpRoot

    $tarball = "harmonia-$Version-windows-x86_64.tar.gz"
    $tarballPath = Join-Path $tmpRoot $tarball
    $url = "$repo/releases/download/harmonia-v$Version/$tarball"
    Write-Step "Downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $tarballPath

    $checksumPath = "$tarballPath.sha256"
    try {
        Invoke-WebRequest -Uri "$url.sha256" -OutFile $checksumPath | Out-Null
        $expected = ((Get-Content $checksumPath | Select-Object -First 1) -split '\s+')[0].Trim()
        $actual = (Get-FileHash -Algorithm SHA256 $tarballPath).Hash.ToLowerInvariant()
        if ($expected.ToLowerInvariant() -ne $actual) {
            throw "checksum mismatch for $tarball"
        }
    } catch {
        Write-Warn "Checksum verification skipped: $($_.Exception.Message)"
    }

    Write-Step "Extracting $tarball"
    tar -xzf $tarballPath -C $tmpRoot
    $artifactRoot = Get-ChildItem $tmpRoot -Directory | Where-Object { $_.Name -like "harmonia-*" } | Select-Object -First 1
    if (-not $artifactRoot) {
        throw "extraction failed"
    }
    return @{
        TempRoot = $tmpRoot
        ArtifactRoot = $artifactRoot.FullName
    }
}

function Install-FromReleaseArtifact {
    $version = if ($env:HARMONIA_VERSION) { $env:HARMONIA_VERSION } else { Get-LatestReleaseVersion }
    $download = Download-ReleaseArtifact $version
    try {
        Install-FromArtifactRoot $download.ArtifactRoot $version
    } finally {
        Remove-Item $download.TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Install-FromSourceCheckout {
    $paths = Get-InstallPaths
    $repo = if ($env:HARMONIA_REPO) { $env:HARMONIA_REPO } else { "https://github.com/harmoniis/harmonia.git" }
    Ensure-Dir (Split-Path $paths.SourceCheckout)

    if (Test-Path (Join-Path $paths.SourceCheckout ".git")) {
        Write-Step "Updating source checkout..."
        Push-Location $paths.SourceCheckout
        try {
            git pull --ff-only
        } finally {
            Pop-Location
        }
    } else {
        Write-Step "Cloning $repo ..."
        git clone $repo $paths.SourceCheckout
    }

    Install-FromLocalRepo $paths.SourceCheckout
}

function Install-Harmonia {
    $localRepo = $PSScriptRoot
    if ((Test-Path (Join-Path $localRepo "Cargo.toml")) -and (Test-Path (Join-Path $localRepo "src")) -and (Test-Path (Join-Path $localRepo "scripts"))) {
        Install-FromLocalRepo $localRepo
        return
    }

    $mode = if ($env:HARMONIA_INSTALL_MODE) { $env:HARMONIA_INSTALL_MODE } else { "binary" }
    switch ($mode) {
        "binary" {
            try {
                Install-FromReleaseArtifact
            } catch {
                Write-Warn "Binary install failed, falling back to source build: $($_.Exception.Message)"
                Install-FromSourceCheckout
            }
        }
        "source" {
            Install-FromSourceCheckout
        }
        default {
            throw "Invalid HARMONIA_INSTALL_MODE=$mode (use binary or source)"
        }
    }
}

Write-Host ""
Write-Section "Harmonia — Self-improving Common Lisp + Rust agent"
Write-Host ""

Check-Rust
if ($Profile -eq "full-agent") {
    Check-Sbcl
    Check-Quicklisp
} else {
    Write-Step "Install profile: tui-client (skipping local SBCL/Quicklisp bootstrap)"
}
Install-Harmonia

Write-Host ""
Write-Host "  Next:"
if ($Profile -eq "tui-client") {
    Write-Host "    On the agent node: harmonia pairing invite"
    Write-Host "    harmonia"
} else {
    Write-Host "    harmonia setup"
    Write-Host "    harmonia start"
}
