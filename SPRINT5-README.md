# Sprint 5 — Empacotamento e Lancamento

## Resumo das mudancas

| Tarefa | Arquivo | Descricao |
|---|---|---|
| **5.1** | `crates/audio_api/src/embed.rs` | Frontend embutido via rust-embed com SPA fallback |
| **5.1** | `crates/audio_api/Cargo.toml` | +rust-embed, +mime_guess |
| **5.2** | `crates/audio_api/src/first_boot.rs` | Setup automatico, deteccao Ollama/Docker, banner |
| **5.2** | `crates/audio_api/src/main.rs` | Integracao embed + first_boot + porta configuravel + auto-browser |
| **5.3** | `.cargo-dist.toml` | Config cargo-dist (Linux/macOS/Windows) |
| **5.3** | `.github/workflows/release.yml` | CI de release via tag push |
| **5.4** | `ui/src/components/WelcomeOnboarding.tsx` | Onboarding de 5 passos |
| **5.4** | `ui/src/App.tsx` | Integracao onboarding + privacidade com localStorage |
| **5.5** | `docs/14-GUIA-DO-USUARIO.md` | Guia de instalacao, uso e troubleshooting |
| **5.6** | `ui/src/components/PrivacyNotice.tsx` | Aviso de privacidade LLM |
| **5.7** | `docs/15-TESTE-COM-USUARIOS.md` | Checklist e template de teste com usuarios |
| **Infra** | `Dockerfile` | 3-stage: Node + Rust + runtime com frontend embutido |
| **Infra** | `ui/vite.config.ts` | `base: './'` para paths relativos |
| **Infra** | `ui/dist/index.html` | Placeholder para compilacao |

## Como aplicar no repositorio

Estes arquivos devem ser copiados para o repositorio `mixlirous`, sobrescrevendo os arquivos existentes com o mesmo caminho.

```bash
# Exemplo:
cp crates/audio_api/src/embed.rs    /path/to/mixlirous/crates/audio_api/src/embed.rs
cp crates/audio_api/src/first_boot.rs /path/to/mixlirous/crates/audio_api/src/first_boot.rs
cp crates/audio_api/src/main.rs      /path/to/mixlirous/crates/audio_api/src/main.rs
cp crates/audio_api/Cargo.toml         /path/to/mixlirous/crates/audio_api/Cargo.toml
# ... etc
```

## Validacao

- 447 testes passando, 0 falhas, 1 ignorado
- `cargo fmt` limpo
- `cargo clippy -- -D warnings` limpo
- Release binary compila com frontend embutido

## Dependencias novas

```toml
mime_guess = "2"
rust-embed = { version = "8", features = ["compression"] }
webbrowser = "1"
ureq = "3"
```
