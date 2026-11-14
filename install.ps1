# Obscura Unified Native Installer (Windows PowerShell)
# https://github.com/itskum47/obscura
#
# Usage:
#   irm https://raw.githubusercontent.com/itskum47/obscura/main/install.ps1 | iex
#   or with parameters:
#   .\install.ps1 -Version "v0.3.0" -Variant "default" -InstallDir "C:\Tools\Obscura"

[CmdletBinding()]
param (
    [Parameter(Mandatory=$false)]
    [string]$Version = $env:OBSCURA_VERSION,

    [Parameter(Mandatory=$false)]
    [ValidateSet("default", "stealth", "no-render", "no-render-stealth")]
    [string]$Variant = $(if ($env:OBSCURA_VARIANT) { $env:OBSCURA_VARIANT } else { "default" }),

    [Parameter(Mandatory=$false)]
    [string]$InstallDir = $env:OBSCURA_INSTALL_DIR,

    [Parameter(Mandatory=$false)]
    [switch]$Yes = $false
)

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Repo = "itskum47/obscura"

function Write-Info([string]$msg) {
    Write-Host "==> " -ForegroundColor Cyan -NoNewline
    Write-Host $msg
}

function Write-Success([string]$msg) {
    Write-Host "==> " -ForegroundColor Green -NoNewline
    Write-Host $msg -ForegroundColor Green
}

function Write-WarningMsg([string]$msg) {
    Write-Host "warning: $msg" -ForegroundColor Yellow
}

function Write-ErrorMsg([string]$msg) {
    Write-Host "error: $msg" -ForegroundColor Red
    exit 1
}

# Check Architecture
$Arch = $env:PROCESSOR_ARCHITECTURE
if ($Arch -notin @("AMD64", "x86_64")) {
    Write-ErrorMsg "Unsupported processor architecture: $Arch. Obscura prebuilt Windows binaries require x86_64 / AMD64."
}

# Resolve target directory
if (-not $InstallDir) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "Obscura\bin"
}

# Resolve target version
if (-not $Version -or $Version -eq "latest") {
    Write-Info "Resolving latest release tag from GitHub..."
    try {
        $ReleaseUri = "https://api.github.com/repos/$Repo/releases/latest"
        $Latest = Invoke-RestMethod -Uri $ReleaseUri -UseBasicParsing -Headers @{ "User-Agent" = "Obscura-PowerShell-Installer" }
        $Tag = $Latest.tag_name
    } catch {
        try {
            # Fallback to redirect header check
            $Req = [System.Net.WebRequest]::Create("https://github.com/$Repo/releases/latest")
            $Req.AllowAutoRedirect = $false
            $Resp = $Req.GetResponse()
            $Location = $Resp.GetResponseHeader("Location")
            $Resp.Close()
            if ($Location -match '/tag/(.+)$') {
                $Tag = $Matches[1]
            } else {
                throw "Could not parse redirect URL: $Location"
            }
        } catch {
            Write-ErrorMsg "Failed to resolve latest release tag from GitHub: $_. Please specify -Version manually."
        }
    }
} else {
    $Tag = $Version
    if (-not $Tag.StartsWith("v")) {
        $Tag = "v$Tag"
    }
}

# Determine asset filename
if ($Variant -eq "default") {
    $ArchiveName = "obscura-x86_64-windows.zip"
} else {
    $ArchiveName = "obscura-x86_64-windows-$Variant.zip"
}

$BaseUrl = if ($env:OBSCURA_DOWNLOAD_BASE_URL) { $env:OBSCURA_DOWNLOAD_BASE_URL } else { "https://github.com/$Repo/releases/download/$Tag" }
$DownloadUrl = "$BaseUrl/$ArchiveName"
$ChecksumsUrl = "$BaseUrl/checksums.txt"

Write-Info "Selected Obscura $Tag ($Variant variant) for Windows x86_64"
Write-Info "Archive: $ArchiveName"
Write-Info "Install target: $InstallDir"

# Create secure temporary directory
$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("obscura-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null

try {
    $ZipPath = Join-Path $TempDir $ArchiveName
    $ChecksumsPath = Join-Path $TempDir "checksums.txt"

    Write-Info "Downloading $ArchiveName..."
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing -Headers @{ "User-Agent" = "Obscura-PowerShell-Installer" }

    Write-Info "Downloading checksums.txt..."
    Invoke-WebRequest -Uri $ChecksumsUrl -OutFile $ChecksumsPath -UseBasicParsing -Headers @{ "User-Agent" = "Obscura-PowerShell-Installer" }

    # Verify SHA-256 Checksum
    Write-Info "Verifying SHA-256 checksum..."
    $ChecksumLines = Get-Content -Path $ChecksumsPath
    $ExpectedHash = ""
    foreach ($Line in $ChecksumLines) {
        $Parts = $Line.Trim() -split '\s+'
        if ($Parts.Length -ge 2 -and $Parts[1] -eq $ArchiveName) {
            $ExpectedHash = $Parts[0].ToLowerInvariant()
            break
        }
    }

    if (-not $ExpectedHash) {
        Write-ErrorMsg "Archive $ArchiveName was not found in release checksums.txt. Aborting installation."
    }

    $ActualHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ExpectedHash -ne $ActualHash) {
        Write-ErrorMsg "Checksum verification failed!`n  Expected: $ExpectedHash`n  Computed: $ActualHash`nAborting."
    }
    Write-Success "Checksum verified: $ActualHash"

    # Extract Archive
    Write-Info "Extracting binaries..."
    $ExtractDir = Join-Path $TempDir "extracted"
    Expand-Archive -Path $ZipPath -DestinationPath $ExtractDir -Force

    $ExtractedObscura = Join-Path $ExtractDir "obscura.exe"
    $ExtractedWorker = Join-Path $ExtractDir "obscura-worker.exe"

    if (-not (Test-Path $ExtractedObscura)) {
        Write-ErrorMsg "Corrupted archive: obscura.exe is missing from $ArchiveName"
    }
    if (-not (Test-Path $ExtractedWorker)) {
        Write-ErrorMsg "Corrupted archive: obscura-worker.exe is missing from $ArchiveName"
    }

    # Ensure destination directory exists
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Multi-binary atomic installation with transactional rollback
    $DstObscura = Join-Path $InstallDir "obscura.exe"
    $BakObscura = Join-Path $InstallDir "obscura.exe.bak"
    $TmpObscura = Join-Path $InstallDir "obscura.exe.tmp"

    $DstWorker = Join-Path $InstallDir "obscura-worker.exe"
    $BakWorker = Join-Path $InstallDir "obscura-worker.exe.bak"
    $TmpWorker = Join-Path $InstallDir "obscura-worker.exe.tmp"

    Write-Info "Installing 'obscura.exe' and 'obscura-worker.exe' to $InstallDir..."

    # Step 1: Backup existing binaries
    if (Test-Path $DstObscura) { Copy-Item -Path $DstObscura -Destination $BakObscura -Force }
    if (Test-Path $DstWorker) { Copy-Item -Path $DstWorker -Destination $BakWorker -Force }

    # Step 2: Stage temporary files
    Copy-Item -Path $ExtractedObscura -Destination $TmpObscura -Force
    Copy-Item -Path $ExtractedWorker -Destination $TmpWorker -Force

    # Step 3: Install obscura.exe
    try {
        Move-Item -Path $TmpObscura -Destination $DstObscura -Force
    } catch {
        Write-WarningMsg "Failed to install obscura.exe. Rolling back..."
        if (Test-Path $BakObscura) { Move-Item -Path $BakObscura -Destination $DstObscura -Force }
        Remove-Item -Path $TmpObscura, $TmpWorker, $BakWorker -Force -ErrorAction SilentlyContinue
        throw $_
    }

    # Step 4: Install obscura-worker.exe (rolls back obscura.exe if worker fails)
    try {
        Move-Item -Path $TmpWorker -Destination $DstWorker -Force
    } catch {
        Write-WarningMsg "Failed to install obscura-worker.exe. Rolling back entire installation..."
        if (Test-Path $BakObscura) {
            Move-Item -Path $BakObscura -Destination $DstObscura -Force
        } else {
            Remove-Item -Path $DstObscura -Force -ErrorAction SilentlyContinue
        }
        if (Test-Path $BakWorker) {
            Move-Item -Path $BakWorker -Destination $DstWorker -Force
        }
        Remove-Item -Path $TmpWorker -Force -ErrorAction SilentlyContinue
        throw $_
    }

    # Step 5: Success - clean up backups
    Remove-Item -Path $BakObscura, $BakWorker -Force -ErrorAction SilentlyContinue

    Write-Success "Obscura binaries successfully installed!"

    # Configure User PATH (case-insensitive deduplication)
    $UserPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
    $PathEntries = if ($UserPath) { $UserPath -split ';' | Where-Object { $_ -ne "" } } else { @() }
    $InPath = $false
    foreach ($Entry in $PathEntries) {
        if ($Entry.TrimEnd('\').Equals($InstallDir.TrimEnd('\'), [System.StringComparison]::OrdinalIgnoreCase)) {
            $InPath = $true
            break
        }
    }

    if (-not $InPath) {
        Write-Info "Adding $InstallDir to User PATH..."
        $NewPath = if ($UserPath) { "$UserPath;$InstallDir" } else { $InstallDir }
        [Environment]::SetEnvironmentVariable("Path", $NewPath, [EnvironmentVariableTarget]::User)
        $env:Path = "$env:Path;$InstallDir"
        Write-Success "PATH updated for current session and future sessions."
    }

    # Verification run
    $InstalledExe = Join-Path $InstallDir "obscura.exe"
    if (Test-Path $InstalledExe) {
        $InstalledVer = & $InstalledExe --version
        Write-Success "Verified executable: $InstalledVer"
    }

    Write-Host ""
    Write-Host "========================================================================" -ForegroundColor Cyan
    Write-Host "           Obscura has been successfully installed!" -ForegroundColor Green
    Write-Host "========================================================================" -ForegroundColor Cyan
    Write-Host "Installed location: $InstallDir"
    Write-Host "Binaries:           obscura.exe, obscura-worker.exe"
    Write-Host ""
    Write-Host "Quick Start:"
    Write-Host "  1. Run diagnostics:      obscura doctor"
    Write-Host "  2. Fetch a webpage:      obscura fetch https://example.com --eval `"document.title`""
    Write-Host "  3. Start CDP server:     obscura serve"
    Write-Host "  4. Launch MCP server:    obscura mcp"
    Write-Host ""

} finally {
    if (Test-Path $TempDir) {
        Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
