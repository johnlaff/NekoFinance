# ADR-0007: a View também é dona da escrita e da chave de cache

ADR-0006 fixou o funil de leitura: um `*View.ts` é o único lugar fora da allowlist legada que pode
traduzir os DTOs crus de `src/lib/api.ts` para as formas de domínio que a tela renderiza. Aquela
decisão não dizia nada sobre escrita nem sobre a convenção de cache do `useCommand` — cada tela
seguia livre para chamar comandos de escrita e montar sua própria string de chave direto no
componente.

`tagsView.ts` é a primeira migração a fechar essa lacuna (issue #328): a tela de Tags não importa
`lib/api` em nenhuma forma, nem em tipo.

## Decisão

Para uma tela migrada ao padrão completo, o `*View.ts` é a porta **inteira** do shim, não só da
leitura:

- **Leitura**: o fetcher estável (a função que o `useCommand` captura na primeira referência) e a
  chave de cache — `tagsScreenCacheKey(ym)` no caso de Tags — nascem na view. A tela nunca monta a
  string da chave por conta própria; ela só chama a função que a view expõe. Isso evita duas telas
  (ou uma tela e seu teste) divergindo sobre o formato da chave do mesmo comando.
- **Escrita**: cada comando de escrita do domínio (criar/atualizar tag, alternar uma régua) ganha um
  wrapper na view — `createTagCmd`, `updateTagCmd`, `toggleTagRuler` — mesmo quando o wrapper só
  repassa para a função do shim sem transformação adicional. O nome do wrapper é o vocabulário de
  domínio da tela (“alternar uma régua”), não o nome do comando Tauri por trás dele.
- **Invalidação**: a view não invalida cache. `invalidateCommands()` é infraestrutura genérica de
  `src/lib/useCommand.ts` (fora do funil — não é `lib/api`) e continua sendo chamada pela tela depois
  de um comando de escrita resolver. A view decide _o que_ escrever; a tela decide _quando_
  revalidar.

O reexport de tipos que ADR-0006 já previa (a view reexporta as formas do DTO que a tela e o teste
precisam) continua valendo — inclusive para os tipos usados só como fixture de teste
(`TagRulerEffects`, `TagRulerFlags`, `TagsScreenDto`, `TagsScreenTag`, `TagsScreenThirdParty`, no
caso de Tags).

## Por que ir além de ADR-0006

Sem essa extensão, uma tela migrada continuava tendo dois pontos de contato com o shim: a view (só
leitura) e o próprio componente (escrita + string de cache montada à mão). O componente seguia livre
para inventar uma segunda leitura do mesmo DTO no caminho de escrita — o mesmo risco que ADR-0006 já
via para dentro (a única mudança é qual metade do funil deixava passar).

## Consequências

- Migrar uma tela para este padrão completo (e não só mover os tipos para o `*View.ts`) é o critério
  para tirá-la da allowlist legada — `TagsScreen.tsx` e `TagsScreen.test.tsx` saem de
  `eslint.lib-api-allowlist.mjs` neste PR porque atingem o padrão completo, não o parcial.
  `scenariosView.ts` cobria, à época desta decisão, só a metade pura/leitura (`scenarios.tsx` seguia
  na allowlist) — estágio intermediário superado: hoje a view expõe também os wrappers de escrita e a
  allowlist está zerada (ADR-0011), tornando-a uma referência completa deste contrato.
- Um `*View.ts` migrado por este padrão expõe: tipos reexportados, `<nome>CacheKey(...)`,
  `<nome>Fetcher(...)`, e um wrapper por comando de escrita que o domínio da tela usa. A tela e seu
  teste só importam desses dois arquivos (`*View.ts` e a infra genérica de `useCommand`) — nunca de
  `lib/api`.
