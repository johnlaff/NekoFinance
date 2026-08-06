# ADR-0008: o shell também tem uma `*View.ts` — de sessão, não de tela

ADR-0006 e ADR-0007 fixaram o funil de leitura/escrita `*View.ts` para as telas de
`src/screens/`. `App.tsx` e `shell/AppShell.tsx` ficaram fora desse contrato: eram as duas
últimas peças na allowlist legada (`eslint.lib-api-allowlist.mjs`) importando `lib/api`
diretamente, porque nenhuma delas é uma tela — são o nível de app.

## Decisão

`App.tsx` e `AppShell.tsx` migram para `src/shell/shellView.ts`, uma view de **sessão** própria,
não um hook. O critério de escolha: os três comandos que o shell lê (`checkAuthStatus`,
`getAppSetting`, `lastSyncAt`) não têm estado próprio nem ciclo de vida — são leituras pontuais
que o `useCommand` genérico já resolve, exatamente como uma tela leria as suas. Um hook em
`src/hooks/**` (a outra zona de exceção do funil, ver `useWriteBackPending.ts`) existe para
lógica que o `useCommand` sozinho não cobre — `useEffect`/`useState` orquestrando um fetch com
regras próprias. Nenhum dos três comandos do shell precisa disso; forçar um hook aqui teria sido
reimplementar o `useCommand` por baixo de uma API mais estreita.

`shellView.ts` segue o mesmo contrato de leitura de ADR-0006: tipos reexportados
(`AuthStatus`) e fetchers estáveis (`fetchAuthStatus`, `fetchAppSetting`, `fetchLastSyncAt`).
Sem escrita — nenhuma tela do shell grava direto no shim. O dado de forecast que `App.tsx` usa
para as dicas numéricas da nav (`fetchForecast`) não entra aqui: pertence ao domínio da tela
Hoje e já é dono de `hojeView.ts`, então `App.tsx` importa de lá — a mesma chave de cache
`get_forecast` que a Hoje e o Horizonte já compartilham.

## Consequências

- O funil de `no-restricted-imports` (`eslint.config.js`) ganha `src/shell/*View.ts` (e seu
  `.test.ts`) como zona de exceção, ao lado de `src/screens/*View.ts`.
- `src/App.tsx` e `src/shell/AppShell.tsx` saem de `eslint.lib-api-allowlist.mjs` — os dois
  últimos arquivos de nível de app que ainda importavam `lib/api` fora do funil.
- Se um dia o shell precisar de estado com ciclo de vida próprio (retry, polling, assinatura de
  evento), a decisão se reabre: `shellView.ts` continua servindo leitura simples, e o que exigir
  mais nasce como hook em `src/hooks/**`, não dentro da view.
