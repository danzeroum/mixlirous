#!/usr/bin/env bash
# Verifica que todo link markdown para outro .md resolve a um caminho real,
# relativo ao arquivo que contém o link. Uso: scripts/check-md-links.sh
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

broken=0
while IFS= read -r -d '' file; do
  dir=$(dirname "$file")
  while IFS= read -r target; do
    target=${target%%#*}
    [ -z "$target" ] && continue
    if [ ! -e "$dir/$target" ]; then
      echo "BROKEN: $file -> $target"
      broken=1
    fi
  done < <(grep -oE '\[[^]]+\]\(([^)]+\.md)[^)]*\)' "$file" | sed -E 's/.*\((.*)\)$/\1/')
done < <(find . -name '*.md' -not -path './.git/*' -print0)

if [ "$broken" -eq 1 ]; then
  echo "Links de documentação quebrados encontrados." >&2
  exit 1
fi
echo "Todos os links de documentação resolvem corretamente."
