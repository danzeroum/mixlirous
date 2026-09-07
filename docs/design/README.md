# docs/design — sistema de cor do Mixlirous

Fonte de verdade da identidade visual aplicada ao produto. Estes documentos são
normativos: onde divergirem de qualquer outro documento, **estes prevalecem** em
matéria de cor.

| Arquivo | Conteúdo |
|---|---|
| [`00-PALETA-E-ORIGEM.md`](./00-PALETA-E-ORIGEM.md) | De onde cada cor veio, amostragem dos materiais de marca, decisões de composição e critério de aceitação |
| [`01-TOKENS.md`](./01-TOKENS.md) | Tabela completa de tokens com contraste WCAG calculado e resumo operacional por papel |
| [`02-MAPA-MIGRACAO.md`](./02-MAPA-MIGRACAO.md) | Mapa normativo classe Tailwind antiga → token, hex literais e inventário de arquivos afetados |
| [`03-REGRAS-SEMANTICAS.md`](./03-REGRAS-SEMANTICAS.md) | Nove regras invioláveis de uso, incluindo as guardas automatizadas de lint |
| [`tokens/theme.css`](./tokens/theme.css) | Bloco `@theme` canônico → copiar para `ui/src/index.css` |
| [`tokens/theme.ts`](./tokens/theme.ts) | Constantes para Canvas 2D / React Flow → copiar para `ui/src/lib/theme.ts` |

## Ordem de leitura para quem vai implementar

1. `00-PALETA-E-ORIGEM.md` — entender por que a paleta é essa
2. `03-REGRAS-SEMANTICAS.md` — entender o que não pode ser feito
3. `02-MAPA-MIGRACAO.md` — executar a substituição
4. `01-TOKENS.md` — consultar durante a execução, sempre que houver dúvida de degrau

## Resumo em uma frase

Base **neutra-quente**, ação primária em **teal** (amostrado do CTA da landing),
IA em **violeta** (amostrado do banner do poster), modo manual em **azul-ciano**,
coral **exclusivo de marca**, âmbar para **atenção**, e um vermelho de erro
deslocado para não colidir com o coral.
