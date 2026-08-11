# Mixlirous — Guia para Assistentes

> **LEIAM `.dev/workspace.yaml` PRIMEIRO antes de qualquer ação.**

## Descoberta Obrigatória

1. **Ler `.dev/workspace.yaml`** — contém paths, issues conhecidos, sprint atual
2. **Ler `.dev/module-status.yaml`** — contém status de cada módulo
3. **Carregar `.dev/DevHelper.ps1`** — contém funções `Write-RustFile`, `Read-RustFile`, etc.

## Regras Fundamentais

- **NUNCA** usar `Set-Content` para arquivos Rust (adiciona BOM, quebra compilador)
- **SEMPRE** usar `Write-RustFile` do DevHelper
- **SEMPRE** prefixar paths com `crates/` (ex: `crates/audio_api/src/...`)
- **NUNCA** commitar `.dev/`
- **ATUALIZAR** metadados após cada tarefa

## Estrutura do Workspace

```
mixlirous/
├── crates/
│   ├── audio_core/      # DSP + domínio
│   ├── audio_agent/     # Loop ReAct
│   └── audio_api/       # API REST + rotas + adapters
├── ui/                  # Frontend React
├── docs/                # Documentação
└── .dev/                # Metadados de desenvolvimento (este guia)
```

## Comandos Úteis

```powershell
# Carregar helpers
. .\.dev\DevHelper.ps1

# Escrever arquivo Rust (sem BOM)
Write-RustFile -Path "crates/audio_api/src/..." -Content $content

# Verificar build
Test-WorkspaceBuild

# Verificar BOM
Test-NoBom -Files @("crates/...")