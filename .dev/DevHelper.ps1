# DevHelper — Mixlirous
# Funções PowerShell para evitar problemas de encoding e path
# Uso: . .\.dev\DevHelper.ps1

function Write-RustFile {
    <#
    .SYNOPSIS
        Escreve arquivo Rust sem BOM e com encoding UTF-8 correto.
    .DESCRIPTION
        Evita o bug do Set-Content que adiciona BOM UTF-8, quebra o compilador Rust.
        Se o path não tiver crates/ no início, tenta adicionar automaticamente.
    #>
    param(
        [Parameter(Mandatory)]
        [string]$Path,
        [Parameter(Mandatory)]
        [string]$Content
    )
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    
    # Se o path não existir e tiver crates/ no início, tentar com crates/
    if (-not (Test-Path $Path)) {
        $cratesPath = "crates/$Path"
        if (Test-Path $cratesPath) {
            $Path = $cratesPath
        }
    }
    
    $dir = Split-Path $Path -Parent
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
    }
    [System.IO.File]::WriteAllText($Path, $Content, $utf8NoBom)
    Write-Host "Written (no BOM): $Path" -ForegroundColor Green
}

function Read-RustFile {
    <#
    .SYNOPSIS
        Lê arquivo Rust corretamente.
    #>
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )
    if (-not (Test-Path $Path)) {
        $altPath = "crates/$Path"
        if (Test-Path $altPath) {
            $Path = $altPath
        } else {
            Write-Error "File not found: $Path (also tried $altPath)"
            return $null
        }
    }
    return [System.IO.File]::ReadAllText($Path, [System.Text.Encoding]::UTF8)
}

function Find-RustFile {
    <#
    .SYNOPSIS
        Encontra arquivo Rust no workspace, ignorando target/.
        Preferencia arquivos em crates/ se houver duplicatas.
    #>
    param(
        [Parameter(Mandatory)]
        [string]$Filter
    )
    $results = Get-ChildItem "C:\btv\mixlirous" -Recurse -Filter $Filter |
        Where-Object { $_.FullName -notmatch "target" } |
        Select-Object -ExpandProperty FullName
    
    # Se houver duplicatas (audio_api/ e crates/audio_api/), preferir crates/
    if ($results.Count -gt 1) {
        $cratesResults = $results | Where-Object { $_ -match "crates" }
        if ($cratesResults) {
            return $cratesResults
        }
    }
    
    return $results
}

function Test-WorkspaceBuild {
    <#
    .SYNOPSIS
        Verifica se o workspace compila.
    #>
    $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "User") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "Machine")
    $result = cargo build --workspace 2>&1
    $exitCode = $LASTEXITCODE
    if ($exitCode -eq 0) {
        Write-Host "BUILD OK" -ForegroundColor Green
    } else {
        Write-Host "BUILD FAILED" -ForegroundColor Red
        $result | Select-Object -Last 10
    }
    return $exitCode -eq 0
}

function Test-NoBom {
    <#
    .SYNOPSIS
        Verifica se arquivos Rust não têm BOM.
    #>
    param(
        [string[]]$Files
    )
    $issues = @()
    foreach ($f in $Files) {
        if (-not (Test-Path $f)) { continue }
        $bytes = [System.IO.File]::ReadAllBytes($f)
        if ($bytes.Length -ge 3 -and $bytes[0] -eq 239 -and $bytes[1] -eq 187 -and $bytes[2] -eq 191) {
            $issues += $f
            Write-Host "BOM found: $f" -ForegroundColor Yellow
        }
    }
    if ($issues.Count -eq 0) {
        Write-Host "No BOM issues found" -ForegroundColor Green
    }
    return $issues
}

function Invoke-SafeCargo {
    <#
    .SYNOPSIS
        Executa comando cargo com PATH correto para Rust GNU + MinGW.
    #>
    param(
        [Parameter(Mandatory)]
        [string[]]$Arguments
    )
    $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "User") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "Machine")
    & cargo @Arguments 2>&1
}

Write-Host "DevHelper loaded. Functions available:" -ForegroundColor Cyan
Write-Host "  Write-RustFile -Path <path> -Content <string>" -ForegroundColor White
Write-Host "  Read-RustFile -Path <path>" -ForegroundColor White
Write-Host "  Find-RustFile -Filter <name>" -ForegroundColor White
Write-Host "  Test-WorkspaceBuild" -ForegroundColor White
Write-Host "  Test-NoBom -Files @(<file1>, <file2>)" -ForegroundColor White
Write-Host "  Invoke-SafeCargo -Arguments <cargo args>" -ForegroundColor White