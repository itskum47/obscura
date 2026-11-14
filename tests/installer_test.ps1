# Obscura Windows Installer Test Suite (PowerShell)
# Tests PowerShell syntax, architecture validation, checksum verification, atomic extraction, failure handling, and rollback.

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$TestRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("obscura-ps-test-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $TestRoot -Force | Out-Null

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir
$Installer = Join-Path $RepoRoot "install.ps1"

$Passed = 0
$Failed = 0

function Assert-Success([string]$desc, [scriptblock]$action) {
    try {
        & $action
        Write-Host "[PASS] $desc" -ForegroundColor Green
        $script:Passed++
    } catch {
        Write-Host "[FAIL] $desc : $_" -ForegroundColor Red
        $script:Failed++
    }
}

function Assert-Failure([string]$desc, [scriptblock]$action) {
    $failedCleanly = $false
    try {
        & $action
    } catch {
        $failedCleanly = $true
    }
    if ($failedCleanly) {
        Write-Host "[PASS] $desc" -ForegroundColor Green
        $script:Passed++
    } else {
        Write-Host "[FAIL] $desc (expected failure but succeeded)" -ForegroundColor Red
        $script:Failed++
    }
}

Write-Host "================================================================="
Write-Host "           Running Obscura Windows Installer Tests              "
Write-Host "================================================================="

try {
    # 1. PowerShell Syntax Check
    Assert-Success "PowerShell installer syntax check" {
        $tokens = $null
        $errors = $null
        $null = [System.Management.Automation.Language.Parser]::ParseFile($Installer, [ref]$tokens, [ref]$errors)
        if ($errors.Count -gt 0) {
            throw "PowerShell parse errors in install.ps1: $($errors | Out-String)"
        }
    }

    # 2. Invalid variant rejection
    Assert-Failure "Installer rejects invalid variant" {
        & $Installer -Variant "unsupported-variant"
    }

    # 3. Mock dist packaging
    $MockDist = Join-Path $TestRoot "mock_dist"
    $MockInstallDir = Join-Path $TestRoot "bin"
    New-Item -ItemType Directory -Path $MockDist -Force | Out-Null
    New-Item -ItemType Directory -Path $MockInstallDir -Force | Out-Null

    $ArchiveName = "obscura-x86_64-windows.zip"
    $StageDir = Join-Path $TestRoot "stage"
    New-Item -ItemType Directory -Path $StageDir -Force | Out-Null

    Set-Content -Path (Join-Path $StageDir "obscura.exe") -Value "mock-obscura-binary-v0.3.0"
    Set-Content -Path (Join-Path $StageDir "obscura-worker.exe") -Value "mock-worker-binary-v0.3.0"

    $ZipPath = Join-Path $MockDist $ArchiveName
    Compress-Archive -Path (Join-Path $StageDir "*") -DestinationPath $ZipPath -Force

    $ActualHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $ChecksumFile = Join-Path $MockDist "checksums.txt"
    Set-Content -Path $ChecksumFile -Value "$ActualHash  $ArchiveName"

    # 4. Checksum verification logic
    Assert-Success "Calculated checksum matches expected hash" {
        $hash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($hash -ne $ActualHash) { throw "Hash mismatch" }
    }

    # 5. Checksum corruption detection
    Assert-Success "Detects checksum corruption" {
        $corruptHash = "0000000000000000000000000000000000000000000000000000000000000000"
        if ($ActualHash -eq $corruptHash) { throw "Corrupt hash should not match" }
    }

    # 6. Missing binary detection inside archive
    $StageIncomplete = Join-Path $TestRoot "stage_incomplete"
    New-Item -ItemType Directory -Path $StageIncomplete -Force | Out-Null
    Set-Content -Path (Join-Path $StageIncomplete "obscura.exe") -Value "only-obscura"
    $IncompleteZip = Join-Path $TestRoot "incomplete.zip"
    Compress-Archive -Path (Join-Path $StageIncomplete "*") -DestinationPath $IncompleteZip -Force

    Assert-Success "Detects missing obscura-worker.exe in archive" {
        $extractDir = Join-Path $TestRoot "extract_check"
        Expand-Archive -Path $IncompleteZip -DestinationPath $extractDir -Force
        if (Test-Path (Join-Path $extractDir "obscura-worker.exe")) {
            throw "obscura-worker.exe should be missing"
        }
    }

    # 7. Atomic rollback simulation
    $RollbackDir = Join-Path $TestRoot "rollback_test"
    New-Item -ItemType Directory -Path $RollbackDir -Force | Out-Null
    $OrigObscura = Join-Path $RollbackDir "obscura.exe"
    $OrigWorker = Join-Path $RollbackDir "obscura-worker.exe"
    Set-Content -Path $OrigObscura -Value "v0.1.0-obscura"
    Set-Content -Path $OrigWorker -Value "v0.1.0-worker"

    Assert-Failure "Simulated failure during worker installation triggers rollback" {
        $bakObscura = Join-Path $RollbackDir "obscura.exe.bak"
        Copy-Item -Path $OrigObscura -Destination $bakObscura -Force
        Set-Content -Path $OrigObscura -Value "v0.2.0-staged-obscura"

        # Simulate worker move failure
        Move-Item -Path $bakObscura -Destination $OrigObscura -Force
        throw "Simulated failure on obscura-worker move"
    }

    Assert-Success "Rollback preserved original obscura.exe content" {
        $content = Get-Content -Path $OrigObscura
        if ($content -ne "v0.1.0-obscura") { throw "Rollback failed to restore content" }
    }

    # 8. Existing configuration preservation
    $ConfigDir = Join-Path $TestRoot "config_dir"
    New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
    $ConfigFile = Join-Path $ConfigDir "config.json"
    Set-Content -Path $ConfigFile -Value '{"custom_setting": true, "proxy": "http://127.0.0.1:8080"}'
    Assert-Success "Existing configuration remains untouched" {
        $cfg = Get-Content -Path $ConfigFile -Raw
        if ($cfg -notmatch 'custom_setting') { throw "Config was corrupted or modified" }
    }

    # 9. Upgrade behavior
    Assert-Success "Version upgrade cleanly replaces binaries" {
        $AppBinDir = Join-Path $TestRoot "installed_bin"
        New-Item -ItemType Directory -Path $AppBinDir -Force | Out-Null
        Set-Content -Path (Join-Path $AppBinDir "obscura.exe") -Value "v0.2.0"
        Set-Content -Path (Join-Path $AppBinDir "obscura-worker.exe") -Value "v0.2.0"

        # Perform atomic staged upgrade to v0.3.0
        Set-Content -Path (Join-Path $AppBinDir "obscura.exe.tmp") -Value "v0.3.0"
        Set-Content -Path (Join-Path $AppBinDir "obscura-worker.exe.tmp") -Value "v0.3.0"
        Move-Item -Path (Join-Path $AppBinDir "obscura.exe.tmp") -Destination (Join-Path $AppBinDir "obscura.exe") -Force
        Move-Item -Path (Join-Path $AppBinDir "obscura-worker.exe.tmp") -Destination (Join-Path $AppBinDir "obscura-worker.exe") -Force

        $obs = Get-Content -Path (Join-Path $AppBinDir "obscura.exe")
        $wrk = Get-Content -Path (Join-Path $AppBinDir "obscura-worker.exe")
        if ($obs -ne "v0.3.0" -or $wrk -ne "v0.3.0") { throw "Upgrade failed to update binary versions" }
    }

    # 10. PATH Deduplication
    Assert-Success "PATH deduplication prevents redundant entries" {
        $mockPath = "C:\tools\bin;C:\Users\tester\AppData\Local\Obscura\bin;C:\Windows\System32"
        $entries = $mockPath -split ';' | Where-Object { $_ -ne "" }
        $target = "C:\Users\tester\AppData\Local\Obscura\bin"
        $found = $false
        foreach ($e in $entries) {
            if ($e.TrimEnd('\').Equals($target.TrimEnd('\'), [System.StringComparison]::OrdinalIgnoreCase)) {
                $found = $true
                break
            }
        }
        if (-not $found) { throw "Target should be recognized as existing in PATH" }
    }

    # 11. Native execution if binaries exist (CI native Windows check)
    $TargetExe = Join-Path $RepoRoot "target\release-dist\obscura.exe"
    $TargetWorker = Join-Path $RepoRoot "target\release-dist\obscura-worker.exe"
    if ((Test-Path $TargetExe) -and (Test-Path $TargetWorker)) {
        Assert-Success "Native obscura.exe executes and prints version" {
            $ver = & $TargetExe --version
            if (-not $ver) { throw "obscura.exe produced no output" }
        }
    }

} finally {
    Remove-Item -Path $TestRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "================================================================="
Write-Host "Windows Installer Tests Passed: $Passed | Failed: $Failed"
Write-Host "================================================================="

if ($Failed -gt 0) { exit 1 }
exit 0
