# Spec 040 — Release train automatizado e auto-update assinado

## Problem Statement

Cortar uma release do Neko Finance é um ritual manual: o mantenedor decide a versão de
cabeça, escreve (ou esquece) o changelog, cria a tag à mão e torce para lembrar de todos os
passos. E o app instalado no Windows nunca fica sabendo de versões novas — atualizar é
baixar o exe de novo por conta própria, o que na prática significa rodar builds velhos por
semanas.

## Solution

Um trem de release que anda sozinho até o último vagão: cada PR mergeado no main entra
automaticamente numa Release PR viva (release-please) que acumula changelog e decide a
versão semântica; mergear essa PR cria uma release **draft** e despacha o build que assina
os artefatos do updater, e um único clique humano de publish cria a tag e serve o update. O app
instalado (NSIS) checa em silêncio no launch e oferece a atualização com um convite calmo —
baixa, instala e reabre só quando o usuário aceita. ADR-0012 registra as decisões.

## User Stories

1. Como mantenedor, quero que a versão semântica seja derivada dos títulos dos PRs, para nunca decidir bump de cabeça.
2. Como mantenedor, quero um changelog acumulado automaticamente a cada merge, para que ele nunca dessincronize da tag.
3. Como mantenedor, quero que mergear a Release PR seja o único gesto para draft + build, para que nenhum passo manual seja esquecível.
4. Como mantenedor, quero que a release nasça como draft, para que nenhum update chegue ao app antes do CI verde.
5. Como mantenedor, quero publicar a release com um clique após conferir o build, para manter o gate humano sobre o que vira "latest".
6. Como mantenedor, quero um check de CI que reprove título de PR fora do Conventional Commits, para que a disciplina seja invariante verificável e não memória.
7. Como mantenedor, quero que PRs de automação também passem pelo check de título, para que o changelog não corrompa silenciosamente.
8. Como usuário do app instalado, quero que o app cheque updates em background no launch, para ficar atualizado sem esforço.
9. Como usuário, quero um convite discreto quando houver update, para decidir eu mesmo o momento de atualizar.
10. Como usuário, quero ver progresso do download e saber que o app vai reiniciar, para que o fechamento nunca seja surpresa.
11. Como usuário, quero que o app instale via modo passivo e reabra sozinho após meu aceite, para que atualizar seja um gesto só.
12. Como usuário offline, quero que a checagem falhe em silêncio, para que um app local-first nunca reclame de rede.
13. Como usuário, quero ver na tela de Configurações a versão atual e o estado do update, para saber onde estou sem esperar convite.
14. Como usuário, quero poder disparar uma checagem manual nas Configurações, para não depender só do launch.
15. Como usuário do exe portátil, quero continuar baixando releases publicadas, mesmo sem canal de auto-update.
16. Como mantenedor, quero que só builds assinados com a chave do updater sejam servidos, para que o app em campo rejeite artefato adulterado.
17. Como mantenedor, quero a chave privada protegida por senha e fora do repo, com backup separado da senha, para que um fator vazado não baste.
18. Como mantenedor, quero um runbook de rotação de chave documentado, para não descobrir a limitação da release-ponte sob pressão.
19. Como mantenedor, quero que o build local (cargo-xwin) continue existindo para teste rápido, com fronteira clara de que release nasce só no CI.
20. Como mantenedor, quero que a release publicada não seja pre-release, para que `/releases/latest` a sirva sem segundo passo esquecível.

## Implementation Decisions

Decisões vindas do ADR-0012 e da grelha; o dossiê citado vive em
`docs/research/release-train-e-updater-tauri.md`.

- **Trilho 1 — o train.** Workflow do release-please (action pinada por SHA) mantém a
  Release PR; ao mergear, cria release **draft** nomeada `v*.*.*` (sem `prerelease`) e
  despacha o workflow de release por `workflow_dispatch` — draft não cria tag (ela nasce
  no publish), e evento criado com `GITHUB_TOKEN` não dispara workflow de tag. O workflow
  de release anexa os artefatos ao draft.
  Check de título de PR (semantic-pull-request, pinada por SHA) vira required check.
  O prompt do runner headless de issues passa a exigir o prefixo no título do PR.
- **Trilho 2 — o updater** (depende do trilho 1 produzir artefato assinado). Par de chaves
  minisign gerado pelo CLI do Tauri; pubkey em `plugins.updater.pubkey` na config do Tauri;
  privada + senha só como GitHub Actions secrets (`TAURI_SIGNING_PRIVATE_KEY`,
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`); original em `~/.tauri/`, backups separados no
  gerenciador de senhas. `createUpdaterArtifacts: true`, `uploadUpdaterJson: true`,
  `updaterJsonPreferNsis: true`; endpoint fixo `/releases/latest/download/latest.json`.
  Permissão `updater:default` (+ `process:default` para relaunch) nas capabilities.
- **Alvo do update é o NSIS instalado** com `installMode: passive`; o exe portátil segue
  publicado como cortesia, sem canal de update.
- **Fluxo no app**: view module `updaterView` no padrão dos `*View.ts` (ADR-0007) com a
  máquina de estados do update; o plugin do Tauri entra como adapter injetado na borda.
  Estados: ocioso → checando → disponível → baixando → pronto para reiniciar → erro.
  Checagem em background no launch; convite único e calmo (um convite por estado,
  ui-standards); download com progresso; `relaunch()` só após aceite. Tratamento defensivo
  do retorno de `check()` (pode vir objeto sem update — issue conhecida do plugin);
  qualquer falha de rede resolve para o estado ocioso sem UI de erro.
- **Configurações** mostra versão atual (já existe), estado do update e ação de checagem
  manual — mesma máquina de estados do `updaterView`.
- **Copy método-neutra e em pt-BR**, primeira letra maiúscula, no registro do design system.
- **Runbook de rotação de chave** (release-ponte assinada com a chave antiga anunciando a
  pubkey nova) entra na documentação de release do repo.
- **Fronteira de build documentada**: release nasce só no CI (`windows-latest` +
  tauri-action); cargo-xwin é ciclo local de teste e nunca produz artefato de release.

## Testing Decisions

- Bom teste aqui é o que exercita **comportamento externo da máquina de update** — dado um
  adapter fake que responde X, o estado observável vira Y — nunca detalhes de implementação
  do plugin.
- `updaterView` é o módulo testado: transições da máquina (update disponível, sem update,
  objeto de `check()` sem update real, falha de rede → silêncio, progresso de download,
  erro de instalação), com adapter fake. Prior art: `configView.test.ts` e os demais
  `*View.test.ts`.
- A UI (convite + bloco nas Configurações) entra no smoke visual Playwright existente;
  asserção de copy por texto, nunca por screenshot.
- Workflows não têm teste de unidade: o gate é o lint de workflows existente; a validação
  fim-a-fim é a primeira release real do trilho 1 (esperada e verificada como parte da
  entrega) e, para o trilho 2, um ciclo completo de update numa máquina Windows
  (versão N instalada recebe N+1).
- Config do Tauri e capabilities são validadas pelo próprio build.

## Out of Scope

- Canal beta / pre-releases servidas a um anel de teste.
- Auto-update do exe portátil (mecanismo custom de auto-substituição).
- Rotação efetiva de chave (só o runbook entra).
- Publicação automática da release (o gate humano fica; migrar a publish automático é
  decisão futura, com histórico do train).
- Assinatura de código Windows (Authenticode/SmartScreen) — independente da assinatura
  minisign do updater.
- Builds macOS.

## Further Notes

- Publicar release marcada como pre-release é falha silenciosa: `/releases/latest` a
  ignora e o update nunca chega — por isso o flag sai do workflow.
- No Windows, o instalador encerra o app durante a instalação; todo o desenho do fluxo
  (consentimento antes do download-e-instala) existe para esse fechamento nunca ser
  surpresa.
- Perder a chave privada estranda permanentemente a base instalada — não há rotação nativa
  no plugin; a release-ponte é o único caminho.
