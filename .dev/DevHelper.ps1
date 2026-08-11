# DevHelper — Mixlirous
# Funções PowerShell para evitar problemas de encoding e path
# Uso: . .\.dev\DevHelper.ps1

function Write-RustFile {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Content
    )
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    if (-not (Test-Path $Path)) {
        $cratesPath = "crates/$Path"
        if (Test-Path $cratesPath) { $Path = $cratesPath }
    }
    $dir = Split-Path $Path -Parent
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    [System.IO.File]::WriteAllText($Path, $Content, $utf8NoBom)
    Write-Host "Written (no BOM): $Path" -ForegroundColor Green
}

function Read-RustFile {
    param([Parameter(Mandatory)][string]$Path)
    if (-not (Test-Path $Path)) {
        $altPath = "crates/$Path"
        if (Test-Path $altPath) { $Path = $altPath }
        else { Write-Error "File not found: $Path (also tried $altPath)"; return $null }
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
    $result = cargo build --workspace 2>&1
    if ($LASTEXITCODE -eq 0) { Write-Host "BUILD OK" -ForegroundColor Green }
    else { Write-Host "BUILD FAILED" -ForegroundColor Red; $result | Select-Object -Last 10 }
    return $LASTEXITCODE -eq 0
}

function Test-NoBom {
    param([string[]]$Files)
    $issues = @()
    foreach ($f in $Files) {
        if (-not (Test-Path $f)) { continue }
        $bytes = [System.IO.File]::ReadAllBytes($f)
        if ($bytes.Length -ge 3 -and $bytes[0] -eq 239 -and $bytes[1] -eq 187 -and $bytes[2] -eq 191) {
            $issues += $f; Write-Host "BOM found: $f" -ForegroundColor Yellow
        }
    }
    if ($issues.Count -eq 0) { Write-Host "No BOM issues found" -ForegroundColor Green }
    return $issues
}

function Invoke-SafeCargo {
    param([Parameter(Mandatory)][string[]]$Arguments)
    $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "User") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "Machine")
    & cargo @Arguments 2>&1
}

function Test-CI {
    <#
    .SYNOPSIS
        Roda os mesmos checks do CI (fmt + clippy + build + test).
        Deve ser chamado ANTES de push para evitar CI vermelho.
    #>
    $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "User") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "Machine")
    
    Write-Host "=== 1/4 cargo fmt" -ForegroundColor Cyan
    cargo fmt --all -- --check 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAIL: fmt diff found. Running cargo fmt --all..." -ForegroundColor Yellow
        cargo fmt --all 2>&1 | Out-Null
        Write-Host "  fmt applied. Verify diffs and commit." -ForegroundColor Yellow
        return $false
    }
    Write-Host "  OK" -ForegroundColor Green

    Write-Host "=== 2/4 clippy -D warnings" -ForegroundColor Cyan
    cargo clippy --workspace -- -D warnings 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Host "FAIL: clippy issues found" -ForegroundColor Red; return $false }
    Write-Host "  OK" -ForegroundColor Green

    Write-Host "=== 3/4 cargo build" -ForegroundColor Cyan
    cargo build --workspace 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Host "FAIL: build broken" -ForegroundColor Red; return $false }
    Write-Host "  OK" -ForegroundColor Green

    Write-Host "=== 4/4 cargo test" -ForegroundColor Cyan
    $env:PROPTEST_CASES = "100"
    cargo test --workspace 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Host "FAIL: tests failing" -ForegroundColor Red; return $false }
    Write-Host "  OK" -ForegroundColor Green

    Write-Host "=== CI READY — pode fazer push" -ForegroundColor Green
    return $true
}

Write-Host "DevHelper loaded. Functions:" -ForegroundColor Cyan
Write-Host "  Write-RustFile, Read-RustFile, Find-RustFile" -ForegroundColor White
Write-Host "  Test-WorkspaceBuild, Test-NoBom, Invoke-SafeCargo" -ForegroundColor White
Write-Host "  Test-CI (fmt+clippy+build+test) — roda antes de push" -ForegroundColor White