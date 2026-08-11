# Mixlirous — Guia para Assistentes

> **LEIA `.dev/workspace.yaml` PRIMEIRO antes de editar qualquer coisa.**

## Descoberta Obrigatória

1. **Ler `.dev/workspace.yaml`** — paths, armadilhas, shortcuts, doc refs
2. **Ler `.dev/module-status.yaml`** — o que está feito/quebrado/pending
3. **Carregar `.dev/DevHelper.ps1`** — funções Write-RustFile, Read-RustFile, etc.

## Regras Absolutas

- **NUNCA** usar `Set-Content` para arquivos Rust (adiciona BOM)
- **SEMPRE** usar `Write-RustFile` do DevHelper
- **SEMPRE** prefixar paths novos com `crates/`
- **ATUALIZAR** `module-status.yaml` após cada tarefa concluída
- **NUNCA** commitar `.env`, `*.wav` ou arquivos temporários

## Estrutura

```
crates/
├── audio_core/      # DSP + domínio (biblioteca, zero I/O rede)
├── audio_agent/     # Loop ReAct, LlmProvider, prompt_guard
└── audio_api/       # API REST + SSE + worker + adapters
ui/                  # React + Vite (Upload, Canvas, HITL overlay)
docs/                # Arquitetura, contratos, roadmap
.dev/                # Metadados de desenvolvimento (commitado!)
```

## Comandos Essenciais

```powershell
. .\.dev\DevHelper.ps1                  # carregar helpers
Write-RustFile -Path "crates/..." -C $c  # escrever Rust sem BOM
Test-WorkspaceBuild                      # build workspace
Test-NoBom @("file1","file2")            # checar BOM
```