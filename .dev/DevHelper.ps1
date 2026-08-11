# DevHelper — Mixlirous
# PowerShell tools for Rust development
# Usage: . .\.dev\DevHelper.ps1

function Write-RustFile {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$Content)
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    if (-not (Test-Path $Path)) {
        $cratesPath = "crates/$Path"
        if (Test-Path $cratesPath) { $Path = $cratesPath }
    }
    $dir = Split-Path $Path -Parent
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    [System.IO.File]::WriteAllText($Path, $Content, $utf8NoBom)
    Write-Host "Written: $Path" -ForegroundColor Green
}

function Read-RustFile {
    param([Parameter(Mandatory)][string]$Path)
    if (-not (Test-Path $Path)) {
        $altPath = "crates/$Path"
        if (Test-Path $altPath) { $Path = $altPath }
        else { Write-Error "Not found: $Path"; return $null }
    }
    return [System.IO.File]::ReadAllText($Path, [System.Text.Encoding]::UTF8)
}

function Find-RustFile {
    param([Parameter(Mandatory)][string]$Filter)
    $results = Get-ChildItem "C:\btv\mixlirous" -Recurse -Filter $Filter |
        Where-Object { $_.FullName -notmatch "target" } |
        Select-Object -ExpandProperty FullName
    if ($results.Count -gt 1) {
        $cratesResults = $results | Where-Object { $_ -match "crates" }
        if ($cratesResults) { return $cratesResults }
    }
    return $results
}

function Test-WorkspaceBuild {
    $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "User") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "Machine")
    cargo build --workspace 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) { Write-Host "BUILD OK" -ForegroundColor Green; return $true }
    Write-Host "BUILD FAILED" -ForegroundColor Red; return $false
}

function Test-NoBom {
    param([string[]]$Files)
    $issues = @()
    foreach ($f in $Files) {
        if (-not (Test-Path $f)) { continue }
        $bytes = [System.IO.File]::ReadAllBytes($f)
        if ($bytes.Length -ge 3 -and $bytes[0] -eq 239 -and $bytes[1] -eq 187 -and $bytes[2] -eq 191) {
            $issues += $f; Write-Host "BOM: $f" -ForegroundColor Yellow
        }
    }
    if ($issues.Count -eq 0) { Write-Host "No BOM" -ForegroundColor Green }
    return $issues
}

function Test-CI {
    # Runs EXACT same checks as GitHub CI (ci-rust.yml)
    $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "User") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "Machine")
    $allOk = $true

    Write-Host "`n[1/4] cargo fmt --all --check" -ForegroundColor Cyan
    cargo fmt --all -- --check 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAIL - run: cargo fmt --all" -ForegroundColor Red
        $allOk = $false
    } else { Write-Host "  OK" -ForegroundColor Green }

    Write-Host "[2/4] cargo clippy --workspace --all-targets -- -D warnings" -ForegroundColor Cyan
    cargo clippy --workspace --all-targets -- -D warnings 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAIL - fix clippy issues" -ForegroundColor Red
        $allOk = $false
    } else { Write-Host "  OK" -ForegroundColor Green }

    Write-Host "[3/4] cargo build --workspace" -ForegroundColor Cyan
    cargo build --workspace 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAIL" -ForegroundColor Red
        $allOk = $false
    } else { Write-Host "  OK" -ForegroundColor Green }

    Write-Host "[4/4] cargo test --workspace" -ForegroundColor Cyan
    $env:PROPTEST_CASES = "100"
    cargo test --workspace 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAIL" -ForegroundColor Red
        $allOk = $false
    } else { Write-Host "  OK" -ForegroundColor Green }

    if ($allOk) {
        Write-Host "`nCI READY - safe to push" -ForegroundColor Green
    } else {
        Write-Host "`nCI WOULD FAIL - fix issues before push" -ForegroundColor Red
    }
    return $allOk
}

Write-Host "DevHelper loaded: Write-RustFile, Read-RustFile, Find-RustFile, Test-WorkspaceBuild, Test-NoBom, Test-CI" -ForegroundColor Cyan