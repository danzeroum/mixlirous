# 15 — Teste com Usuarios Reais (Sprint 5, Tarefa 5.7)

## Objetivo
Validar que uma pessoa de fora consegue instalar e usar o Mixlirous sem ajuda do time.

## Perfil dos participantes

| ID | Persona | Descricao | Experiencia tecnica |
|---|---|---|---|
| P1 | Produtor musical | Cria remixes e mashups, usa Ableton | Alta |
| P2 | Podcaster | Edita episodios, precisa de cortes rapidos | Media |
| P3 | Musico hobby | Gravou demos em casa | Baixa |

## Checklist de teste

### Fase 1: Instalacao (< 5 min)
- [ ] Baixa o binario corretamente
- [ ] Consegue extrair/executar
- [ ] Navegador abre ou encontra localhost:8080
- [ ] Tela de onboarding aparece
- [ ] Aviso de privacidade aparece

### Fase 2: Primeiro remix (< 10 min)
- [ ] Encontra o botao de upload
- [ ] Upload de WAV/MP3 funciona
- [ ] Entende que precisa digitar um prompt
- [ ] O agente responde com proposta (< 30s)
- [ ] Entende a proposta e aprova/rejeita
- [ ] O render comeca e progresso e visivel
- [ ] Consegue ouvir o resultado

### Fase 3: Iteracao
- [ ] Tenta ajustar parametros manualmente
- [ ] Entende diferenca entre aprovar e rejeitar
- [ ] Faz segundo remix sem ajuda
- [ ] Exporta o resultado

### Fase 4: Problemas
- [ ] Confusoes catalogadas
- [ ] Sugestoes de melhoria
- [ ] Bugs registrados
- [ ] Satisfacao (1-5)

## Template de sessao

```markdown
## Sessao [ID]
**Data:** YYYY-MM-DD
**Participante:** P[1-3]
**Duracao total:** XX min

### Friction points
1. ...

### Sugestoes
1. ...

### Bugs
1. ...

### Satisfacao: X/5
```

## Criterios de aceite
- [ ] P1, P2 e P3 completam Fase 1 e 2 sem ajuda
- [ ] Nenhum bug critico encontrado
- [ ] Satisfacao media >= 3/5
- [ ] Pelo menos 1 sugestao acionavel
- [ ] Relatorio preenchido para cada participante
