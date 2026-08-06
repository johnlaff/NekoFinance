# Release train e updater no Tauri v2

Este documento cobre duas frentes de engenharia para um app Tauri v2 desktop distribuído via GitHub Releases: o plugin de auto-update oficial (`@tauri-apps/plugin-updater` / `tauri-plugin-updater`) e a automação de build/release em GitHub Actions com `tauri-apps/tauri-action`. Todas as afirmações citam a fonte primária oficial ao lado.

## 1. Plugin de updater (Tauri v2)

### 1.1 Assinatura minisign

O par de chaves é gerado pelo próprio CLI do Tauri:

```
npm run tauri signer generate -- -w ~/.tauri/myapp.key
```

Isso produz uma chave privada (nunca deve ser compartilhada; perdê-la impede publicar novas atualizações para quem já instalou o app) e uma chave pública. A chave pública entra em `tauri.conf.json`, em `plugins.updater.pubkey`, **como conteúdo da chave, não como caminho de arquivo**:

```json
{
  "bundle": { "createUpdaterArtifacts": true },
  "plugins": {
    "updater": {
      "pubkey": "CONTEÚDO DO PUBLICKEY.PEM",
      "endpoints": [
        "https://releases.myapp.com/{{target}}/{{arch}}/{{current_version}}"
      ]
    }
  }
}
```

`createUpdaterArtifacts: true` instrui o bundler do Tauri a gerar os artefatos assinados do updater; para apps migrando de versões antigas do Tauri o valor é `"v1Compatible"`. (Fonte: [v2.tauri.app/plugin/updater](https://v2.tauri.app/plugin/updater/), verificado 2026-08.)

Durante o build, a assinatura acontece automaticamente ao exportar duas variáveis de ambiente **reais** (arquivos `.env` não funcionam para isso):

- `TAURI_SIGNING_PRIVATE_KEY` — caminho ou conteúdo da chave privada.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — senha da chave (pode ser string vazia se a chave não tiver senha).

Fonte: [v2.tauri.app/plugin/updater](https://v2.tauri.app/plugin/updater/), verificado 2026-08.

### 1.2 Formato do manifesto (`latest.json`) e endpoints dinâmicos

O manifesto estático usado por um endpoint fixo segue este formato, com todos os campos obrigatórios:

```json
{
  "version": "1.0.0",
  "notes": "Update notes",
  "pub_date": "2024-01-15T10:30:00Z",
  "platforms": {
    "linux-x86_64": {
      "signature": "conteúdo do arquivo .sig gerado no build",
      "url": "https://cdn.example.com/app.AppImage"
    }
  }
}
```

O campo `signature` é o **conteúdo** do arquivo `.sig` produzido no build — nunca um caminho ou URL. Para endpoints dinâmicos (um servidor que decide o que responder por requisição, em vez de servir um JSON estático), a URL do endpoint aceita três placeholders substituídos pelo cliente antes da requisição:

- `{{current_version}}` — versão instalada do app solicitante.
- `{{target}}` — nome do SO (`linux`, `windows`, `darwin`).
- `{{arch}}` — arquitetura (`x86_64`, `i686`, `aarch64`, `armv7`).

Fonte: [v2.tauri.app/plugin/updater](https://v2.tauri.app/plugin/updater/), verificado 2026-08.

### 1.3 Compatibilidade com Windows/NSIS

O updater funciona com o instalador NSIS (o formato de instalador usado neste repositório). O modo de instalação é configurável em `tauri.conf.json` sob `plugins.updater.windows.installMode`, com três opções:

- **`passive`** (padrão) — janela pequena com barra de progresso; a instalação ocorre sem exigir interação do usuário.
- **`basicUi`** — exige interação do usuário para concluir a instalação.
- **`quiet`** — sem feedback visual algum; só funciona para instalações por-usuário ou instalações privilegiadas específicas.

Comportamento da instância em execução: no Windows, **o aplicativo é encerrado automaticamente quando o passo de instalação é executado**, por limitação dos instaladores Windows. Em outras plataformas, o app pode esperar o usuário reiniciar manualmente ou solicitar que ele escolha quando reiniciar.

Permissão exigida em `src-tauri/capabilities/default.json`:

```json
{ "permissions": ["updater:default"] }
```

O conjunto `updater:default` concede `allow-check`, `allow-download`, `allow-install` e `allow-download-and-install`.

Fonte: [v2.tauri.app/plugin/updater](https://v2.tauri.app/plugin/updater/), verificado 2026-08.

### 1.4 Fluxo de uso (JS/Rust)

API JS principal (`@tauri-apps/plugin-updater`):

```javascript
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

const update = await check();
if (update) {
  await update.downloadAndInstall((event) => {
    /* progresso */
  });
  await relaunch();
}
```

Além de `downloadAndInstall()` (combinado), os métodos `download()` e `install()` existem separadamente para quem quer controlar as duas fases (ex.: baixar em segundo plano e só instalar com confirmação explícita do usuário). A documentação oficial mostra padrões tanto para checagem disparada pelo usuário quanto para monitoramento em segundo plano via tarefas assíncronas, sem prescrever checagem no launch versus checagem periódica como obrigatória — a escolha fica a critério do app. Fonte: [v2.tauri.app/plugin/updater](https://v2.tauri.app/plugin/updater/), verificado 2026-08.

### 1.5 Limitações e issues abertas relevantes (verificado 2026-08)

Levantamento em [github.com/tauri-apps/plugins-workspace/issues](https://github.com/tauri-apps/plugins-workspace/issues) filtrando por "updater" no título:

- [#3506](https://github.com/tauri-apps/plugins-workspace/issues/3506) — no macOS, o `.app` atualizado pode ser instalado com permissões `0700` (herdadas do `TempDir` do processo de atualização), tornando-se inacessível para outros usuários da mesma máquina.
- [#3505](https://github.com/tauri-apps/plugins-workspace/issues/3505) — no macOS, uma instalação que falha pode **apagar o app do usuário**: o backup vive num `TempDir` autodestrutivo e não há restauração automática em caso de falha.
- [#3300](https://github.com/tauri-apps/plugins-workspace/issues/3300) — no Windows, `updater > installerArgs` precisa ser reestruturado para suportar múltiplos tipos de instalador (relevante porque este repositório usa NSIS).
- [#2998](https://github.com/tauri-apps/plugins-workspace/issues/2998) — `check()` sempre retorna um objeto e nunca `null`, mesmo quando não há atualização disponível — inconsistência de API a considerar ao escrever a lógica de checagem.

Nenhuma dessas issues bloqueia o uso básico do plugin em Windows/NSIS, mas #3300 e #2998 afetam diretamente decisões de implementação para este repositório (tipo de instalador único NSIS, e tratamento defensivo do retorno de `check()`).

## 2. Release train em GitHub Actions

### 2.1 O que `tauri-apps/tauri-action` automatiza

A action oficial builda o app Tauri como binário nativo para macOS, Linux e Windows e, opcionalmente, publica os artefatos em uma GitHub Release — cobrindo compilação e gestão de assets de release em um único passo de workflow. Fonte: [github.com/tauri-apps/tauri-action](https://github.com/tauri-apps/tauri-action), verificado 2026-08.

Inputs documentados relevantes:

| Input                                | Efeito                                                             |
| ------------------------------------ | ------------------------------------------------------------------ |
| `tagName`                            | Tag da release (aceita substituição `__VERSION__`)                 |
| `releaseName` / `releaseBody`        | Título e corpo da release                                          |
| `releaseId`                          | Publica em uma release já existente, por ID                        |
| `releaseDraft` (padrão `false`)      | Cria a release como rascunho                                       |
| `prerelease` (padrão `false`)        | Marca como pre-release                                             |
| `args`                               | Argumentos extras passados ao build do Tauri                       |
| `uploadUpdaterJson` (padrão `true`)  | Gera e publica o manifesto do updater                              |
| `updaterJsonPreferNsis`              | Em Windows, prioriza o bundle NSIS sobre WiX ao montar o manifesto |
| `uploadPlainBinary` (padrão `false`) | Inclui o executável não empacotado como asset                      |
| `projectPath`                        | Raiz do projeto Tauri (padrão `./`)                                |

Variáveis de ambiente: `GITHUB_TOKEN` é obrigatória para as operações de API do GitHub; `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` são opcionais e habilitam a assinatura do updater durante o mesmo build. Fonte: [github.com/tauri-apps/tauri-action](https://github.com/tauri-apps/tauri-action), verificado 2026-08.

O workflow atual deste repositório (`.github/workflows/release.yml`) já usa essa action com `uploadUpdaterJson: false` — a assinatura do updater está desligada intencionalmente até as chaves de release serem configuradas — e mantém `releaseDraft: true` / `prerelease: true`.

### 2.2 Cross-compile Windows via cargo-xwin: não é um fluxo suportado pela action

A documentação da action **não menciona suporte nativo a cross-compile de Windows a partir de Linux via `cargo-xwin`**. Ela é descrita e exemplificada em torno de runners específicos por plataforma (`macos-latest`, `ubuntu-22.04`, `windows-latest`), com especificação de `target` documentada apenas para as duas arquiteturas de macOS (`aarch64-apple-darwin`, `x86_64-apple-darwin`). Fonte: [github.com/tauri-apps/tauri-action](https://github.com/tauri-apps/tauri-action), verificado 2026-08.

Isso confirma o desenho já adotado neste repositório: o cross-compile via `cargo-xwin`/MSVC (`scripts/build-windows.sh`, usado para o exe portátil single-file) é um fluxo **customizado, fora da tauri-action**, mantido só para build local/manual a partir de Linux/WSL2. O workflow de release (`release.yml`) já usa a alternativa oficial — um runner `windows-latest` nativo na mesma matriz que builda `ubuntu-24.04` — que é o caminho suportado pela action para produzir o instalador NSIS assinado e o `latest.json` do updater. Trade-off documentado pela GitHub Actions: runners Windows/macOS consomem minutos de cota em taxa multiplicada em relação a runners Linux (ver [docs.github.com — Usage limits, billing, and administration](https://docs.github.com/en/actions/administering-github-actions/usage-limits-billing-and-administration)), e não compartilham a mesma imagem de cache de dependências de forma direta com o job Linux — o custo de runner Windows nativo é a contrapartida de obter suporte de primeira classe da tauri-action (assinatura, geração de `latest.json`, upload de asset) sem workaround.

### 2.3 Versionamento e changelog: release-please vs. git-cliff vs. tag manual

**release-please** ([github.com/googleapis/release-please](https://github.com/googleapis/release-please), verificado 2026-08) lê Conventional Commits do histórico (`fix:` → patch, `feat:` → minor, `!`/`BREAKING CHANGE` → major) e mantém uma "Release PR" viva que se atualiza a cada merge, acumulando o changelog. Ao mergear essa PR, a ferramenta atualiza o `CHANGELOG.md`, cria a tag de versão e publica uma GitHub Release — esse evento de release é o gatilho natural para um workflow de build subsequente (`on: release: types: [published]`, ou por tag via `on: push: tags:`). Suporta repositórios single-package simples e, via configuração de manifesto, múltiplos artefatos numa mesma árvore.

**git-cliff** ([github.com/orhun/git-cliff](https://github.com/orhun/git-cliff), verificado 2026-08) gera changelog a partir do log Git (Conventional Commits ou parsers customizados por regex), configurado via `cliff.toml`. Ao contrário do release-please, não abre nem mantém uma PR automaticamente — é um passo explícito de CI ou de linha de comando, disparado intencionalmente por quem está cortando a release. O changelog gerado é então commitado ou publicado manualmente como parte do processo.

**Tag manual** — o mantenedor decide a versão e escreve o changelog à mão antes de criar a tag; nenhuma ferramenta infere o bump de versão a partir das mensagens de commit.

Para um repositório de mantenedor único, com PRs em português: release-please exige que os commits (ou ao menos os títulos de PR squash-merged) sigam Conventional Commits em inglês nos prefixos estruturais (`feat:`, `fix:`, `chore:` — o restante da mensagem pode ser em qualquer idioma), o que combina com squash-merge de PR único. Em troca, elimina decisão manual de versão semântica e garante que o changelog nunca fique dessincronizado da tag. git-cliff é mais flexível (não exige nenhuma disciplina de commit-title) mas não fecha o loop sozinho: alguém ainda decide a versão e dispara o passo de geração — mais controle, mais trabalho manual por release. Tag manual é o caminho de menor fricção para builds esporádicos, mas não escala para o objetivo de habilitar updates automáticos assinados, pois não força nenhuma disciplina de changelog nem gatilho determinístico de workflow — cada release depende de o mantenedor lembrar de todos os passos manualmente.

Nenhuma das três ferramentas está atualmente configurada em `.github/workflows/` deste repositório — o gatilho de release hoje é só `push: tags: v*.*.*` mais `workflow_dispatch` manual (`release.yml`), equivalente ao modelo de tag manual.

### 2.4 GitHub Releases como servidor de update

O GitHub documenta uma URL estável para o asset mais recente de uma release publicada: `/releases/latest/download/<nome-do-asset>` — um link direto que sempre resolve para o asset com aquele nome na release mais recente. Fonte: [docs.github.com — Linking to releases](https://docs.github.com/en/repositories/releasing-projects-on-github/linking-to-releases), verificado 2026-08. A documentação consultada não especifica o comportamento desse sufixo frente a releases marcadas como rascunho ou pre-release; o comportamento observado do GitHub é que `/releases/latest` (a rota da qual o sufixo `/download/` deriva) **ignora rascunhos e pre-releases**, resolvendo apenas a release mais recente que não tenha nenhuma das duas flags — o que já é compatível com o padrão draft→publish deste repositório (`releaseDraft: true`, `prerelease: true` em `release.yml`), mas essa parte específica do comportamento não foi confirmada em texto explícito da documentação oficial consultada.

Limites de taxa da API REST relevantes para uma checagem periódica de update (verificado 2026-08, fonte: [docs.github.com — Rate limits for the REST API](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)):

- Requisições não autenticadas: 60 por hora, por IP de origem.
- Requisições autenticadas: 5.000 por hora (linha de base); até 15.000/hora para GitHub Apps/OAuth Apps de organizações GitHub Enterprise Cloud.
- Limite secundário: no máximo 100 requisições concorrentes e no máximo 900 pontos por minuto por endpoint REST; a maioria das requisições GET (como buscar dados de release) custa 1 ponto.

Implicação prática: um app desktop checando updates via API REST sem autenticação (o caso comum de um cliente distribuído publicamente) compartilha o teto de 60 req/hora **por IP**, não por instalação — em redes com NAT compartilhado (ex.: uma empresa inteira atrás do mesmo IP) esse teto pode ser atingido por checagens de múltiplos usuários somadas. Servir o manifesto do updater via a URL estática `/releases/latest/download/latest.json` (download de asset, não uma chamada à API REST) não está sujeito a esse limite de requisições da API — é servido como qualquer outro download de arquivo estático do GitHub.

O fluxo draft→publish é a forma documentada de preparar uma release sem servi-la a clientes: `releaseDraft: true` cria a release sem publicá-la (invisível em `/releases/latest`), e o mantenedor publica manualmente depois que o CI/gate correspondente fecha verde. `prerelease: true` cumpre um papel adicional — mesmo publicada, uma release marcada como pre-release não aparece em `/releases/latest`, permitindo campanhas de teste (beta) sem expor a build a `/releases/latest/download/`.

### 2.5 Gestão da chave privada do updater em repositório público

A chave privada minisign deve ser armazenada como **GitHub Actions secret** (`TAURI_SIGNING_PRIVATE_KEY`, e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` se a chave tiver senha) — nunca commitada, mesmo cifrada, num repositório público. A própria documentação do plugin adverte que perder essa chave impede publicar novas atualizações para a base de usuários já instalada, e que ela nunca deve ser compartilhada. Fonte: [v2.tauri.app/plugin/updater](https://v2.tauri.app/plugin/updater/), verificado 2026-08.

Sobre **rotação de chave**: a documentação oficial do updater (v2) não cobre um procedimento de rotação. O padrão de rotação mais citado na comunidade — descrito na issue [tauri-apps/tauri#7585](https://github.com/tauri-apps/tauri/issues/7585) — segue esta sequência: um app em campo na v0.0.1 confia na Chave A; ao publicar v0.0.2, o build ainda é **assinado com a Chave A**, mas o `pubkey` declarado em `tauri.conf.json` já passa a ser o da Chave B; o cliente em campo aceita v0.0.2 porque a assinatura ainda bate com a chave que ele conhece (A); só a partir da v0.0.3 (já assinada com a Chave B) o `tauri.conf.json` precisa declarar definitivamente o `pubkey` B. Ou seja: **um app antigo que confia apenas na chave antiga NÃO consegue validar um update assinado direto com uma chave nova** — a transição exige uma release intermediária assinada com a chave antiga que já anuncia a chave nova, closing o gap; pular essa etapa intermediária quebra o update para quem está na versão anterior à troca.

Essa mesma issue propõe suporte nativo a múltiplas chaves públicas válidas simultaneamente (`pubkey` como lista, com fallback de verificação da primeira para a segunda) — funcionalidade que eliminaria a necessidade da release intermediária, mas que **não está implementada na versão atual do plugin** (verificado 2026-08). Até que isso mude, qualquer rotação de chave neste repositório precisa passar por essa release-ponte assinada com a chave antiga.

## Resumo do que muda o desenho do pipeline deste repositório

- O runner `windows-latest` nativo já em uso em `release.yml` é o caminho correto — `tauri-action` não documenta nem suporta `cargo-xwin`/cross-compile de Windows a partir de Linux; esse fluxo (`scripts/build-windows.sh`) deve continuar restrito a build manual/local, nunca ser portado para o job de release.
- `uploadUpdaterJson: false` no `release.yml` atual é consistente com a ausência de `TAURI_SIGNING_PRIVATE_KEY`/pubkey configurados — ativar o updater requer gerar o par de chaves (`tauri signer generate`), publicar o pubkey em `tauri.conf.json` (`plugins.updater.pubkey`, conteúdo, não caminho) e guardar a chave privada como secret do GitHub Actions.
- Windows/NSIS é compatível com o updater; o app é encerrado automaticamente durante a instalação (limitação do próprio instalador Windows) — não há como manter o processo vivo nesse passo.
- release-please é o melhor encaixe para mantenedor único: fecha o loop versão↔changelog↔tag↔gatilho de release sozinho, desde que os prefixos estruturais de commit/PR (`feat:`/`fix:`) fiquem em inglês mesmo com corpo em português; git-cliff exige disparo manual e não substitui decisão de versão.
- A URL estática `/releases/latest/download/latest.json` não está sujeita ao rate limit da API REST (60/h não-autenticado); o fluxo `releaseDraft: true` + `prerelease: true` já usado no repo é a forma correta de não vazar update antes do gate verde.
- Rotação de chave do updater NÃO tem suporte nativo no plugin hoje — exige uma release-ponte assinada com a chave antiga que já anuncia a chave nova antes de trocar definitivamente; documentar esse runbook agora evita descobrir a limitação sob pressão, no dia em que a chave precisar trocar.
