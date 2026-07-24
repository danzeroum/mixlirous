#!/usr/bin/env bash
# Verifica que todo link markdown para outro .md resolve a um caminho real,
# relativo ao arquivo que contém o link. Uso: scripts/check-md-links.sh
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

PRUNE=(-path './.git' -o -path './node_modules' -o -path '*/node_modules' -o -path './target' -o -path './ui/dist')

broken=0
while IFS= read -r -d '' file; do
  dir=$(dirname "$file")
  while IFS= read -r target; do
    target=${target%%#*}
    [ -z "$target" ] && continue
    case "$target" in
      http://*|https://*) continue ;;
    esac
    if [ ! -e "$dir/$target" ]; then
      echo "BROKEN: $file -> $target"
      broken=1
    fi
  done < <(grep -oE '\[[^]]+\]\(([^)]+\.md)[^)]*\)' "$file" | sed -E 's/.*\((.*)\)$/\1/')
done < <(find . \( "${PRUNE[@]}" \) -prune -o -name '*.md' -print0)

if [ "$broken" -eq 1 ]; then
  echo "Links de documentação quebrados encontrados." >&2
  exit 1
fi
echo "Todos os links de documentação resolvem corretamente."
