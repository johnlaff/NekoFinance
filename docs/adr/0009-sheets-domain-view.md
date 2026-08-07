# ADR-0009: `sheetsView.ts` é a porta de domínio do fluxo Sheets/write-back

O fluxo Sheets/write-back não vive numa tela só — está fatiado entre `features/sheets/`
(`GoogleSheetsPanel.tsx`, `LocalXlsxImport.tsx`, `WriteBackPreview.tsx`, `writeBack.ts`),
`screens/dashboard/WriteBackPending.tsx` (o atalho "Sincronizar" do dashboard) e a seção Conexão de
`SettingsScreen.tsx`. Nenhum desses arquivos é uma tela com um `*View.ts` próprio — o domínio (conexão
Google, import remoto/local, prévia e apply do write-back) é maior que qualquer um deles isoladamente.

## Decisão

`src/features/sheets/sheetsView.ts` é a porta completa (padrão ADR-0007: leitura + escrita, sem chave
de cache parametrizada porque nenhum fetcher deste domínio precisa de uma) do domínio Sheets/write-back,
no mesmo espírito de `lancamentosView.ts` para o domínio de lançamento (ver CONTEXT.md, "Compositor
estende a view de domínio"): o boundary é o domínio compartilhado, não o diretório de cada consumidor.

- **Leitura**: `fetchGoogleAuthStatus`, `fetchUserSpreadsheets`, `fetchSheetNames`,
  `fetchSheetPreviewCmd`, `fetchSheetMappings`, `fetchWriteBackEnabled`, `fetchWriteBackPreview`,
  `fetchEconomiaWriteBackPreview`, `fetchImportConflictsCount`, `fetchSheetsSetting`.
- **Escrita**: `connectGoogleCmd`, `disconnectGoogleCmd`, `detectSheetLayoutCmd`,
  `saveSheetMappingCmd`, `importSheetDataCmd`, `importEconomiaSheetCmd`, `importLocalXlsxCmd`,
  `applyWriteBackCmd`, `applyEconomiaWriteBackCmd`, `setSheetsSetting`.
- Reexporta os tipos de domínio (`AuthStatus`, `CellWrite`, `ImportDiagnostic`, `SheetInfo`,
  `SheetMappingEntry`, `SheetPreview`, `UserSpreadsheet`, `WriteBackPreviewResult`,
  `WriteBackResult`) e as chaves de preferência local do domínio (`LAST_IMPORT_KEY`,
  `LAST_SHEET_KEY`, `BG_SYNC_KEY`, `CLIENT_ID_KEY`, `NOTES_DEGRADED_KEY`).
- `eslint.config.js` ganha `src/features/sheets/sheetsView.ts` (+ teste) como zona de exceção do
  funil, ao lado de `src/screens/*View.ts` e `src/shell/*View.ts`.

`GoogleSheetsPanel.tsx`, `LocalXlsxImport.tsx`, `WriteBackPreview.tsx`,
`screens/dashboard/WriteBackPending.tsx` e a seção Conexão/OAuth de `SettingsScreen.tsx` importam só
de `sheetsView.ts` — nunca de `lib/api`, nem em tipo. Os cinco arquivos (+ seus testes) saem de
`eslint.lib-api-allowlist.mjs` neste PR.

O restante de `SettingsScreen.tsx` (backup, info do app, flags, consentimento da Mia, lembrete,
recência do sync) não é domínio Sheets — atinge o mesmo padrão completo estendendo
`src/screens/configView.ts` (que já existia como leitura pura do veredito da tela, `greetState`).
`getDailyBudget` já tinha porta própria em `tetoView.fetchDailyBudget` (ADR-0007); `SettingsScreen`
passa a lê-lo de lá em vez de repetir a leitura.

`hooks/useWriteBackPending.ts` continua fora deste contrato: já é zona de exceção pelo ADR-0006
(`src/hooks/**`) e opera no mesmo nível de funil que uma view — não duplica a tradução, só lê o shim
direto para o hook de estado do dashboard, sem transformação de domínio adicional.

## Por que não uma view por arquivo

Uma `googleSheetsPanelView.ts` e uma `writeBackPreviewView.ts` separadas duplicariam os fetchers de
prévia/apply do write-back (`WriteBackPreview.tsx` e `WriteBackPending.tsx` chamam exatamente os
mesmos `fetchWriteBackPreview`/`applyWriteBackCmd`) — o mesmo risco de duas leituras divergentes do
mesmo comando que ADR-0006 e ADR-0007 já fecharam para uma tela. O domínio é um só; a porta é uma só.

## Consequências

- Um novo comando do domínio Sheets (nova ação de import, novo campo de prévia) ganha um wrapper em
  `sheetsView.ts` e nada mais muda na lista de importadores.
- `WriteBackPreview.tsx` e `WriteBackPending.tsx` (dashboard) continuam livres para reusar o mesmo
  fluxo de prévia/apply sem reimplementar a leitura — já era a intenção do design original
  ("sem reimplementar o diff/apply"), agora garantida pelo funil em vez de por convenção.
