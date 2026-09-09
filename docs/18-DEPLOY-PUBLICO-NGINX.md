# 18 — Deploy Público: `mixlirous.danzeroum.com` no Nginx Compartilhado

> Runbook para publicar este projeto num domínio próprio, por trás do proxy
> reverso nginx que já roda no VPS e já serve ~15 outros domínios de produção.
>
> **Esse nginx vive fora deste repositório** (`/opt/btv/ingress`, projeto
> Compose `global-ingress`, container `btv-nginx-prod`). Da §5 em diante os
> comandos rodam no VPS, não aqui.

---

## 1. Topologia

```
Internet
   │  DNS: mixlirous.danzeroum.com  A  76.13.238.209  (TTL 300)
   ▼
btv-nginx-prod         projeto `global-ingress`, fora deste repositório
   │                   TLS termina aqui · auth_basic aqui
   │                   rede Docker: btv-prod-net
   ▼
mixlirous-api:8080     serviço remix_api + docker-compose.ingress.yml
   │                   rede Docker: mixlirous
   ▼
postgres:5432 · minio:9000
```

Duas redes Docker distintas se encontram num ponto só: o container
`mixlirous-api`, que participa das duas.

---

## 2. Pré-requisitos

- [ ] DNS propagado: `dig +short mixlirous.danzeroum.com` → `76.13.238.209`
- [ ] Acesso SSH ao VPS e ao diretório `/opt/btv/ingress` — obrigatório a partir da §5; este repositório sozinho não completa o processo
- [ ] `certbot` instalado no VPS (os outros 15 domínios já o usam)
- [ ] §3 lida antes de qualquer outra coisa

---

## 3. Pré-condição: o binário precisa estar de pé

O [`README.md`](../README.md) e a [auditoria](14-AUDITORIA-KIT.md) já afirmaram
que este projeto não compilava. **Isso está desatualizado** —
`cargo build --workspace`, `cargo clippy -- -D warnings` e `cargo fmt --check`
passam. O que **não** está verificado é o modo VPS: o CI de Docker só exercita
`CONFIG_ENV=local` (SQLite, disco), nunca Postgres + MinIO + `JWT_SECRET`, que
é o que este `docker-compose.yml` sobe.

Então confira no próprio VPS, antes de tocar no nginx:

```bash
cd mixlirous   # o clone deste repositório
docker compose -f docker-compose.yml -f docker-compose.ingress.yml ps
docker compose -f docker-compose.yml -f docker-compose.ingress.yml logs remix_api --tail 100
curl -sf localhost:8080/healthz     # espera {"status":"ok"} — contrato em 03-CONTRATOS-API.md §3.1
```

Se isso falhar, o problema é binário/config, não nginx. **Enquanto o
`remix_api` não responder em `/healthz`, o vhost novo devolve 502 — isso é
esperado e não é erro de configuração do proxy.**

---

## 4. Neste repositório: `docker-compose.ingress.yml`

O arquivo está na raiz. Ele faz três coisas: dá ao container um nome estável
(`mixlirous-api`) para o `proxy_pass` mirar, junta-o à rede `btv-prod-net`, e
recolhe a porta 8080 para o loopback.

```bash
docker compose -f docker-compose.yml -f docker-compose.ingress.yml up -d
```

> **Armadilha operacional.** Um `docker compose up -d` futuro sem o segundo
> `-f` recria o `remix_api` **fora** da rede do ingress. Não dá erro: o site
> simplesmente passa a responder 502, possivelmente semanas depois, sem
> ninguém ligar uma coisa à outra. Para não depender de lembrar, fixe no
> `.env` do VPS (que não é versionado):
>
> ```
> COMPOSE_FILE=docker-compose.yml:docker-compose.ingress.yml
> ```
>
> Com isso, `docker compose up -d` sozinho já inclui os dois arquivos.

---

## 5. No VPS: editar o `nginx.conf`

> **Raio de alcance.** `/opt/btv/ingress/nginx/nginx.conf` serve ~15 domínios
> de produção sem relação com este projeto. Um erro de sintaxe impede a
> recarga de **todos**, não só do novo. Backup antes, `nginx -t` depois de
> cada mudança, e só então recarregar.

```bash
cp /opt/btv/ingress/nginx/nginx.conf \
   /opt/btv/ingress/nginx/nginx.conf.bak.$(date +%s)
```

O arquivo é bind-mount read-only no container — editar no host e recarregar
basta, não precisa rebuild.

### 5.1 Fase 1 — só a porta 80

**Esta ordem não pode ser invertida.** Um bloco `listen 443 ssl` que aponta
para certificados inexistentes derruba o `nginx -t` inteiro, inclusive para os
outros 15 domínios. Primeiro sobe o :80 (que serve o desafio ACME), depois
emite o certificado, e só então o :443.

Dentro do `http {}`, junto aos outros blocos por domínio:

```nginx
server { listen 80; server_name mixlirous.danzeroum.com;
    location /.well-known/acme-challenge/ { root /var/www/certbot; }
    location / { return 301 https://$host$request_uri; }
}
```

```bash
docker exec btv-nginx-prod nginx -t && docker exec btv-nginx-prod nginx -s reload
```

### 5.2 Emitir o certificado

```bash
certbot certonly --webroot -w /var/www/certbot -d mixlirous.danzeroum.com
```

Confirme que os arquivos existem antes de seguir:

```bash
ls /etc/letsencrypt/live/mixlirous.danzeroum.com/
# fullchain.pem  privkey.pem  ...
```

> Se for iterar na configuração, use `--dry-run` primeiro: o Let's Encrypt
> limita tentativas por domínio, e gastar o limite depurando digitação é
> desnecessário.

### 5.3 Fase 2 — o bloco 443

Só depois que os `.pem` da §5.2 existirem:

```nginx
server {
    listen 443 ssl;
    server_name mixlirous.danzeroum.com;

    ssl_certificate     /etc/letsencrypt/live/mixlirous.danzeroum.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/mixlirous.danzeroum.com/privkey.pem;

    add_header Strict-Transport-Security "max-age=63072000" always;
    location ~* (wp-login|login\.cgi|\.git|env) { return 444; }

    # Não é conveniência: a rota de diagnóstico aceita upload sem
    # autenticação de aplicação, e o stream SSE só ganhou extractor de auth
    # agora. Este bloco é a proteção de rede dos dois. Não publique este
    # vhost sem ele.
    auth_basic "Mixlirous";
    auth_basic_user_file /etc/nginx/.htpasswd;   # o mesmo já montado no ingress

    # SSE precisa de buffering desligado, senão o nginx segura os eventos.
    location ~ ^/api/v1/jobs/[^/]+/events$ {
        set $upstream "http://mixlirous-api:8080";
        proxy_pass $upstream;
        proxy_buffering off;
        proxy_read_timeout 3600s;
    }

    location / {
        set $upstream "http://mixlirous-api:8080";
        proxy_pass $upstream;

        # Casa com LIMITE_UPLOAD_BYTES em crates/audio_api/src/routes/uploads.rs
        # (a própria API agora impõe o mesmo teto na rota real de upload —
        # QA-0001: antes só o dev_router tinha o limite e o PUT real tomava
        # 413 com o default de 2 MB do axum). Divergir qualquer dos três
        # (nginx, rota real, dev_router) faz o upload morrer numa camada
        # sem mensagem da outra.
        client_max_body_size 100M;

        # Medido, não chutado: 180 s de áudio processam em 0,35 s no build
        # release (1,97 s com ?stretch, onde o resampling domina). 120 s dá
        # ~60× de folga e ainda cobre upload lento de arquivo grande.
        proxy_read_timeout 120s;
    }
}
```

O `set $upstream` seguido de `proxy_pass $upstream` é deliberado — não troque
por `proxy_pass http://mixlirous-api:8080` literal. Combinado com o
`resolver 127.0.0.11` que já está no `http {}`, ele força o nginx a resolver o
nome pelo DNS do Docker **a cada requisição**, em vez de uma vez no load da
configuração. Sem isso, o nginx se recusa a iniciar quando o container está
fora do ar, e continua mirando um IP velho se o container reiniciar.

```bash
docker exec btv-nginx-prod nginx -t && docker exec btv-nginx-prod nginx -s reload
```

### 5.4 Headers de segurança + compressão (P-01 do ciclo QA)

A §5.3 só declarava `Strict-Transport-Security` (HSTS). O ciclo WebQA
(`docs/qa/ACHADOS.md` QA-0002..QA-0007, QA-0013) revelou que a aplicação
dependia do dev server Vite para entregar headers como
`X-Content-Type-Options`, `X-Frame-Options`, `Content-Security-Policy`,
`Permissions-Policy`, e compressão `gzip` — todos ausentes no vhost de
produção. Esta seção adiciona o bloco que **fecha a paridade dev→prod**.

Adicionar ao `server { listen 443 ssl; ... }` da §5.3 (dentro do bloco,
antes do `location`):

```nginx
# === P-01: headers de segurança + gzip (paridade com dev server) ===

# Compressão gzip: ativa para todos os Content-Types textuais. Em dev o
# Vite cobre via plugin (gzipPlugin em ui/vite.config.ts); em prod o
# nginx faz o serviço. Sem isto, a home de ~1 KB viaja sem compressão
# (QA-0003 aceito como limitação do dev server — em prod tem que ter).
gzip on;
gzip_vary on;
gzip_min_length 100;
gzip_proxied any;
gzip_comp_level 6;
gzip_types
    text/plain
    text/html
    text/css
    text/xml
    application/json
    application/javascript
    application/xml
    image/svg+xml;

# Headers de segurança — sempre (even on 4xx/5xx). O `always` é
# obrigatório porque sem ele o nginx só seta em 2xx, e a suíte WebQA
# verifica em todas as respostas, incluindo 404.
#
# ATENÇÃO: a CSP abaixo NÃO inclui o hash do axe-core que existe em
# dev (vite.config.ts) apenas para desbloquear o teste de a11y. Em
# produção, a suíte WebQA não roda contra a URL pública — não há
# motivo para permitir inline scripts. O hash foi explicitamente
# removido aqui para evitar a exceção de ferramenta de teste virar
# regra de produção.
add_header Strict-Transport-Security "max-age=63072000" always;
add_header X-Content-Type-Options "nosniff" always;
add_header X-Frame-Options "DENY" always;
add_header Referrer-Policy "strict-origin-when-cross-origin" always;
add_header Permissions-Policy "camera=(), microphone=(), geolocation=(), payment=(), usb=(), magnetometer=(), gyroscope=(), accelerometer=()" always;

# CSP: default-src 'self' é a postura correta. 'unsafe-inline' em
# style-src porque o build do Vite às vezes gera CSS inline em
# critical-path; sem isto a renderização quebra. Sem 'unsafe-inline'
# em script-src — todos os scripts vêm de arquivos externos
# (assets/*.js com hash no nome).
add_header Content-Security-Policy "default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' wss:; media-src 'self' blob:; frame-ancestors 'none'" always;

# Remove headers que vazam versão de servidor (QA-0002 — test_nao_expoe_versao).
server_tokens off;
```

#### Por que `wss:` e não `ws:` em `connect-src`

Em produção, todo tráfego é HTTPS. O `connect-src 'self' wss:` permite
WebSocket seguro (que o HMR do Vite usa em dev, mas em prod é só SSE
sobre HTTP). `ws:` (insecure) seria um downgrade implícito se a URL
mudasse — não há motivo para permitir.

#### Por que `gzip_min_length 100` e não o default (20)

Em dev, o limiar é 100 porque a home tem ~1 KB — esse número garante
que o gzip middleware do Vite tenha motivo para comprimir. Em prod, o
default 20 bytes já comprime tudo, mas o overhead do header gzip em
respostas minúsculas (favicon, robots.txt) é maior que o ganho. 100
bytes é o ponto onde gzip passa a valer a pena.

#### Como validar

```bash
# De fora do servidor (substitua pelo domínio real):
curl -sI https://mixlirous.danzeroum.com/ | grep -iE "strict-transport|x-content-type|x-frame|content-security|permissions-policy|referrer-policy"
# Esperado: 6 linhas, todas com os valores da configuração acima.

curl -sI -H "Accept-Encoding: gzip" https://mixlirous.danzeroum.com/ | grep -i "content-encoding"
# Esperado: content-encoding: gzip

curl -sI https://mixlirous.danzeroum.com/ | grep -iE "^server:"
# Esperado: vazio (server_tokens off remove o header).
```

#### Pendência que esta seção fecha

- **P-01** (BACKLOG.md): nginx de produção só declarava HSTS — agora
  tem a paridade completa com o dev server.
- **QA-0003** (gzip na home): aceito como limitação do Vite preview;
  em produção este bloco cobre.
- **QA-0021** (erro 500 vaza detalhe): em produção, o nginx por
  default tem `proxy_intercept_errors off` — deixa o axum responder
  `application/problem+json` (QA-0006). Não precisa de `error_page`
  customizado.

---

## 6. Verificação, nesta ordem

| # | O que prova | Comando | Esperado |
| --- | --- | --- | --- |
| 1 | DNS | `dig +short mixlirous.danzeroum.com` | `76.13.238.209` |
| 2 | App de pé, independente do nginx | `curl -sf localhost:8080/healthz` | `{"status":"ok"}` |
| 3 | **Rede, antes de tocar no nginx** | `docker exec btv-nginx-prod wget -qO- http://mixlirous-api:8080/healthz` | `{"status":"ok"}` |
| 4 | Redirect (após fase 1) | `curl -I http://mixlirous.danzeroum.com/` | `301` para `https://` |
| 5 | Certificado | `ls /etc/letsencrypt/live/mixlirous.danzeroum.com/` | `fullchain.pem`, `privkey.pem` |
| 6 | `auth_basic` ativo | `curl -sI https://mixlirous.danzeroum.com/healthz` | `401` |
| 7 | Com credencial | `curl -su USUARIO https://mixlirous.danzeroum.com/healthz` | `{"status":"ok"}` |
| 8 | 8080 fechada para fora | `curl --max-time 5 http://76.13.238.209:8080/healthz` **de outra máquina** | **falha** (timeout/recusa) |
| 9 | **Os outros 15 domínios** | `curl -sI https://<cada-um-dos-outros>/` | igual a antes |

O passo 3 é o que separa "problema de rede Docker" de "problema de nginx" —
fazê-lo antes de editar o `nginx.conf` economiza uma depuração inteira. O
passo 9 é o mais importante e o mais fácil de pular por pressa.

---

## 7. Troubleshooting

| Sintoma | Causa provável | Ação |
| --- | --- | --- |
| `nginx -t` falha ao adicionar o bloco 443 | Certificado ainda não existe — fez a §5.3 antes da §5.2 | Confirme os `.pem` antes de editar; se já editou, remova o bloco 443, recarregue e volte à §5.2 |
| `502 Bad Gateway` | App fora do ar, container fora de `btv-prod-net`, ou nome/porta errados | §3 (app de pé?) e verificação 3 (rede?) |
| `502` que apareceu sozinho, sem ninguém mexer | `docker compose up -d` rodado sem o segundo `-f` | Ver a armadilha da §4; reaplique com os dois `-f` |
| `504 Gateway Timeout` no upload | Faixa muito longa ou upload lento | O teto da aplicação é 4 min; acima disso ela recusa com 413 e mensagem. 504 aponta para rede lenta, não DSP |
| `413` no upload | `client_max_body_size` menor que o limite da aplicação | Os dois precisam bater: 100M dos dois lados |
| Conexão recusada / timeout | DNS não propagou | `dig +short`; o TTL é 300 s |
| Fecha sem resposta (444) | Caiu no `default_server` — `server_name` não bateu | Confira a digitação exata do domínio |
| Aviso de certificado no navegador | Bloco 443 ativado antes de o certbot terminar | Refaça §5.2, confirme os arquivos, então §5.3 |
| certbot falha no desafio HTTP | Fase 1 não foi recarregada antes | `nginx -t && nginx -s reload` **antes** do certbot |
| `/api/v1/dev/slice` dá 404 | `MIXLIROUS_DEV_SLICE` não está em `1`, ou `CONFIG_ENV=production` | É a trava funcionando. Ver §8 |

---

## 8. A rota de diagnóstico

`GET /api/v1/dev/slice` serve uma página de escuta A/B: sobe uma faixa, o
pipeline de DSP roda de forma síncrona e o resultado volta para comparar com o
original ali mesmo. Existe porque o motor funciona mas não há fila, worker nem
UI ligada a ele — sem isso, julgar o resultado exigiria `docker cp` e um player
de sistema. Ver `crates/audio_api/src/routes/dev_slice.rs`.

**É descartável de propósito** e sai quando o produto existir.

Duas travas, independentes:

1. A rota só é registrada com `MIXLIROUS_DEV_SLICE=1`, e nunca sob
   `CONFIG_ENV=production` (aí o boot registra um `ERROR` e segue sem ela).
2. Ela **não** tem autenticação de aplicação. Quem protege é o `auth_basic`
   da §5.3.

Formatos aceitos: WAV, FLAC, AIFF, MP3, AAC e M4A. Limites: 100 MB e 4 minutos
— o teto de duração existe porque este VPS é compartilhado, e a saída em mono
`f32` custa ~46 MB por cópia a 4 min, com várias cópias vivas ao mesmo tempo
durante o pipeline.

Pela linha de comando, sem a página:

```bash
curl -u USUARIO -F file=@faixa.wav \
  "https://mixlirous.danzeroum.com/api/v1/dev/slice?format=wav" -o resultado.wav
```

> **`no_beats_detected`.** Se o aviso aparecer, o detector de batidas não
> encontrou nada e o pipeline usou o PCM bruto: só a masterização agiu, a
> remontagem por blocos não. Isso acontece em **todas** as fixtures sintéticas
> do repositório (`detect_beat_frames` exige `onset > 0.1` e o material
> sintético não chega lá — issue #27). Com faixa real o comportamento pode
> ser outro; é justamente o que esta página serve para descobrir.

---

## 9. Rollback

No VPS:

```bash
cp /opt/btv/ingress/nginx/nginx.conf.bak.<timestamp> /opt/btv/ingress/nginx/nginx.conf
docker exec btv-nginx-prod nginx -t && docker exec btv-nginx-prod nginx -s reload
```

Neste repositório, desfazer a exposição é parar de passar o segundo `-f`
(e remover `COMPOSE_FILE` do `.env`, se tiver sido usado):

```bash
docker compose -f docker-compose.yml up -d
```
