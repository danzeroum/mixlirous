# .dev — Ferramentas de Desenvolvimento

Este diretório contém metadados e ferramentas usadas durante todo o
desenvolvimento do Mixlirous. **É commitado no repositório** para que
qualquer dev ou assistente (OpenCode) que clonar o projeto tenha acesso
imediato.

## Como usar (primeira vez)

```powershell
# 1. Carregar funções PowerShell
. .\.dev\DevHelper.ps1

# 2. Ler o mapa do workspace
cat .dev\workspace.yaml

# 3. Ver status dos módulos
cat .dev\module-status.yaml

# 4. Escrever arquivo Rust (sem BOM!)
Write-RustFile -Path "crates/audio_api/src/..." -Content $content

# 5. Verificar build
Test-WorkspaceBuild
```

## Arquivos

| Arquivo | Função | Quando ler |
|---|---|---|
| `workspace.yaml` | Paths, traps, shortcuts, doc refs | Início de cada sessão |
| `module-status.yaml` | % de conclusão de cada módulo | Para saber o que falta |
| `sprint-4-guide.yaml` | Padrões de resiliência/observabilidade | Sprint 4 |
| `DevHelper.ps1` | Write-RustFile, Read-RustFile, Find-RustFile, Test-WorkspaceBuild, Test-NoBom | Sempre que editar arquivos |
| `README.md` | Este arquivo | Primeira vez |

## Por que existe

Durante o desenvolvimento, enfrentamos problemas recorrentes:
- Set-Content adiciona BOM UTF-8 → compilador Rust rejeita
- PowerShell lida diferente com paths → arquivos no lugar errado
- Falta de referência rápida → perda de tempo lendo docs

Estas ferramentas resolvem esses problemas e são mantidas atualizadas
a cada sprint para refletir o estado real do repositório.