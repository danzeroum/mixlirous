#!/usr/bin/env bash
#
# Importa labels, milestones e o backlog inicial para o repositório.
#
# Requisitos: gh CLI autenticado (gh auth login) e permissão de escrita.
# Uso:
#   bash backlog/import-issues.sh                      # repositório atual
#   REPO=danzeroum/mixlirous bash backlog/import-issues.sh
#   DRY_RUN=1 bash backlog/import-issues.sh            # só mostra o que faria
#
set -euo pipefail

REPO="${REPO:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}"
CSV="$(dirname "$0")/issues.csv"
DRY_RUN="${DRY_RUN:-0}"

say()  { printf '\033[1;34m▸\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!\033[0m %s\n' "$*"; }

run() {
  if [[ "$DRY_RUN" == "1" ]]; then
    printf '   [dry-run] %s\n' "$*"
  else
    "$@" >/dev/null 2>&1 || warn "falhou (pode já existir): $*"
  fi
}

say "Repositório: $REPO"
[[ -f "$CSV" ]] || { echo "CSV não encontrado: $CSV"; exit 1; }

# ─────────────────────────────── labels ───────────────────────────────
say "Criando labels..."

create_label() { run gh label create "$1" --repo "$REPO" --color "$2" --description "$3" --force; }

create_label "area/dsp"        "1d76db" "Motor de áudio e algoritmos"
create_label "area/domain"     "0e8a16" "Modelo de domínio e tipos"
create_label "area/agent"      "5319e7" "Agente LLM e orquestração"
create_label "area/api"        "006b75" "API, rotas, persistência"
create_label "area/ui"         "d93f0b" "Frontend e design"
create_label "area/infra"      "bfd4f2" "Infra, CI/CD, observabilidade"
create_label "area/docs"       "c5def5" "Documentação"

create_label "type/feat"       "0e8a16" "Nova funcionalidade"
create_label "type/fix"        "d73a4a" "Correção"
create_label "type/test"       "fbca04" "Testes"
create_label "type/chore"      "cfd3d7" "Infra, dependências, tarefas"
create_label "type/spike"      "d4c5f9" "Investigação ou decisão"

create_label "prio/p0"         "b60205" "Bloqueante"
create_label "prio/p1"         "d93f0b" "Importante"
create_label "prio/p2"         "fbca04" "Desejável"

create_label "pillar/validation" "5319e7" "Pilar: validação de contrato"
create_label "pillar/atomicity"  "5319e7" "Pilar: atomicidade e recuperação"
create_label "pillar/tracing"    "5319e7" "Pilar: rastreabilidade"
create_label "pillar/queue"      "5319e7" "Pilar: idempotência da fila"

create_label "status/blocked"      "e4e669" "Bloqueado por decisão ou dependência"
create_label "status/needs-design" "e4e669" "Aguardando design"
create_label "status/needs-review" "e4e669" "Aguardando revisão"

# ───────────────────────────── milestones ─────────────────────────────
say "Criando milestones..."

create_milestone() {
  if [[ "$DRY_RUN" == "1" ]]; then
    printf '   [dry-run] milestone: %s\n' "$1"
    return
  fi
  gh api "repos/$REPO/milestones" -f title="$1" -f description="$2" >/dev/null 2>&1 \
    || warn "milestone já existe: $1"
}

create_milestone "S0 Fundação"     "Fazer o kit compilar; CI verde"
create_milestone "S1 Contratos"    "API, persistência, fila, SSE"
create_milestone "S2 Motor DSP"    "Do WAV de entrada ao WAV de saída"
create_milestone "S3 Agente e UI"  "ReAct, HITL, canvas"
create_milestone "S4 Resiliência"  "Recovery, observabilidade, MLOps"
create_milestone "S5 Lançamento"   "Empacotamento e distribuição"

# ─────────────────────────────── issues ───────────────────────────────
say "Criando issues a partir de $CSV..."

python3 - "$CSV" "$REPO" "$DRY_RUN" <<'PY'
import csv, subprocess, sys

csv_path, repo, dry = sys.argv[1], sys.argv[2], sys.argv[3] == "1"
created = 0

with open(csv_path, newline="", encoding="utf-8") as fh:
    for row in csv.DictReader(fh):
        title = row["title"].strip()
        if not title:
            continue
        cmd = ["gh", "issue", "create", "--repo", repo,
               "--title", title,
               "--body", row["body"].strip()]
        for label in filter(None, (l.strip() for l in row["labels"].split(","))):
            cmd += ["--label", label]
        if row.get("milestone", "").strip():
            cmd += ["--milestone", row["milestone"].strip()]

        if dry:
            print(f"   [dry-run] issue: {title}")
        else:
            res = subprocess.run(cmd, capture_output=True, text=True)
            if res.returncode == 0:
                print(f"   ✓ {title}")
            else:
                print(f"   ! falhou: {title}\n     {res.stderr.strip()}")
        created += 1

print(f"\n{created} issues processadas.")
PY

say "Pronto."
echo
echo "Próximos passos:"
echo "  1. Criar um Project (board) e adicionar as issues do milestone 'S0 Fundação'"
echo "  2. Proteger a branch main: PR obrigatório, CI verde, 1 aprovação"
echo "  3. Resolver as issues [ADR] — elas bloqueiam decisões das sprints seguintes"
