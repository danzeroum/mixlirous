# 14 — Guia do Usuario

## Instalacao

### Opcao 1: Binario pre-compilado (recomendado)

1. Acesse a pagina de Releases no GitHub
2. Baixe o binario para seu sistema operacional:
   - **Linux**: `mixlirous-x86_64-linux.tar.gz`
   - **macOS (Apple Silicon)**: `mixlirous-aarch64-macos.tar.gz`
   - **macOS (Intel)**: `mixlirous-x86_64-macos.tar.gz`
   - **Windows**: `mixlirous-x86_64-windows.zip`
3. Extraia o arquivo
4. Execute `./mixlirous` (Linux/macOS) ou `mixlirous.exe` (Windows)
5. O navegador abre automaticamente em `http://localhost:8080`

### Opcao 2: Docker

```bash
docker compose -f docker-compose.local.yml build
docker compose -f docker-compose.local.yml up
# Acesse http://localhost:8080
```

### Opcao 3: Compilar do codigo-fonte

**Requisitos:** Rust 1.94+ (https://rustup.rs), Node.js 22+ (https://nodejs.org)

```bash
git clone https://github.com/danzeroum/mixlirous.git
cd mixlirous
cd ui && npm ci && npm run build && cd ..
cargo build --release
./target/release/audio_api
```

## Primeiro remix

### 1. Configurar o LLM (opcional)

**Ollama (local, sem saida de dados):**
```bash
ollama pull llama3.1
```
Edite `config/default.yaml`:
```yaml
llm:
  provider: "ollama"
  model: "llama3.1"
  base_url: "http://localhost:11434"
```

**DeepSeek (nuvem):**
```bash
export DEEPSEEK_API_KEY="sua-chave-aqui"
```

### 2. Fazer upload de uma faixa

Clique no painel lateral esquerdo, selecione um arquivo (WAV, MP3, FLAC ou OGG). A analise e automatica.

### 3. Criar um remix

Exemplos de prompt:
- "Crie um remix de 30 segundos com as partes mais energeticas"
- "Faca um medley com os melhores trechos, crossfade de 2 segundos"
- "Selecione os 20 segundos mais calmos da faixa"

### 4. Aprovar ou ajustar
- **Aprovar**: renderiza com os parametros sugeridos
- **Rejeitar**: o agente replaneja
- **Ajustar manualmente**: modifique antes de aprovar

### 5. Ouvir e exportar

O resultado aparece no canvas. Use o player para ouvir e exporte.

## Configuracao

| Variavel | Padrao | Descricao |
|---|---|---|
| `CONFIG_ENV` | `local` | Ambiente: local, staging, production |
| `MIXLIROUS_PORT` | `8080` | Porta HTTP |
| `MIXLIROUS_NO_BROWSER` | - | Impede abertura automatica do navegador |
| `JWT_SECRET` | - | Obrigatorio em production |

## Solucao de problemas

### Navegador nao abre automaticamente
Acesse `http://localhost:8080` manualmente.

### Erro "Frontend nao disponivel"
Execute `cd ui && npm run build` antes de rodar.

### Ollama nao detectado
```bash
curl http://localhost:11434/api/tags
ollama list
```

### Porta 8080 em uso
```bash
MIXLIROUS_PORT=3000 ./mixlirous
```

## Privacidade

- O **audio** nunca sai da sua maquina
- Em modo **Ollama local**, nenhum dado e enviado externamente
- Em modo **DeepSeek/OpenAI**, o prompt e metadados tecnicos sao enviados
- Veja `docs/08-SEGURANCA-MULTITENANCY.md`
