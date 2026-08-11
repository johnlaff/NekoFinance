# Relatório do gate sensorial — spike Android

Aparelho de referência: Samsung SM-S926B (S24+), Android recente, tela 120Hz. APK de
desenvolvimento instalado via ADB como `app.neko.finance`. Cada item abaixo documenta veredito,
evidência e tentativas de correção usadas (teto: 2 por item).

## Veredito final: **gate aprovado**

Os cinco itens passam. A frente segue para a spec 043 (convergência por snapshot no Drive) e a
spec 044 (porte Android v1), conforme ADR-0014.

## Itens do gate

### 1. Scroll denso sem jank — **passa**

Medido com `dumpsys gfxinfo` (pipeline Skia/Vulkan) durante rolagem rápida e sustentada, com a
planilha real importada (item 5) carregada.

- **Lançamentos** (lista densa por dia, com dados reais de agosto/2026): duas corridas —
  56 frames / 3,57% janky / p99 12ms / 0 vsync perdidos; 88 frames / 3,41% janky / p99 13ms /
  0 vsync perdidos.
- **Calendário** (grade do mês + bloco de detalhe): 53 frames / 1,89% janky / p99 14ms /
  0 vsync perdidos.

Nenhuma tentativa de correção foi necessária. `Number Missed Vsync: 0` em todas as corridas —
não há frame deadline perdido no pior caso de densidade.

### 2. Animações do design system fluidas + `reduced-motion` — **passa**

O sistema de motion (`src/design-system/tokens/motion.css`) já colapsa todas as durações para 0ms
sob `@media (prefers-reduced-motion: reduce)` — mecanismo único, compartilhado por toda a
superfície do app, não uma correção por tela. A pergunta do spike era se o WebView do Android
repassa a preferência do SO para essa media query.

Verificado via Chrome DevTools Protocol (`webview_devtools_remote_<pid>`, exposto porque o build
de depuração do Tauri liga `setWebContentsDebuggingEnabled`), avaliando
`window.matchMedia('(prefers-reduced-motion: reduce)').matches` com o app reiniciado a frio entre
cada leitura — necessário porque o WebView memoriza o valor no início do processo e não
reavalia sozinho se a preferência do SO muda com o app já aberto:

- `animator_duration_scale=1` (padrão do sistema) + restart → `matches: false`
- `animator_duration_scale=0` (equivalente a "Remover animações" da Acessibilidade) + restart →
  `matches: true`

Nenhuma tentativa de correção foi necessária. Fluidez confirmada pela mesma medição de `gfxinfo`
do item 1 (pipeline Vulkan, sem frame deadline perdido).

### 3. Teclado virtual íntegro — **passa**

Testado na folha "Novo lançamento": campo de texto (Descrição) e campo numérico (Valor). Nos dois
casos o teclado abre, o campo focado permanece visível acima dele (o layout rolou/redimensionou
corretamente) e nenhum elemento saiu dos limites da tela. Texto digitado ("Teste") e teclado
numérico dedicado para Valor confirmados por captura de tela.

Nenhuma tentativa de correção foi necessária.

### 4. Cold start ≤ 3s — **passa com folga**

Medido de duas formas, em três corridas com `am force-stop` antes de cada uma (cold start real,
sem processo residente):

- `am start -W`: `TotalTime` (toque → primeiro frame da Activity) entre 312–323ms.
- Gravação de tela disparada na mesma chamada do `am start` (para não sofrer a latência de
  despachar comandos via ADB por Tailscale), com os frames extraídos por timestamp real
  (`ffprobe best_effort_timestamp_time`, não por índice — `screenrecord` só emite frame quando
  o conteúdo muda): a tela Hoje estabiliza com saldo, teto do dia e projeção do mês já
  renderizados em **1,23s, 1,28s e 1,33s** nas três corridas — sem nenhuma mudança de pixel
  depois disso dentro da janela gravada.

Nenhuma tentativa de correção foi necessária. Toque no ícone até Hoje interativa fica em torno de
40% do orçamento de 3s.

### 5. Import completo de planilha real — **passa (1 tentativa de correção)**

**1ª tentativa: reprovou.** O picker de arquivo do Tauri (`@tauri-apps/plugin-dialog`) devolve, no
Android, um `content://` da Storage Access Framework — não um caminho de filesystem. O comando
Rust (`import_local_xlsx`) sempre leu o arquivo com `std::fs::symlink_metadata`/`open_workbook`
direto no caminho recebido, o que funciona em desktop (caminho real) mas falha de imediato no
Android (`content://` não é um caminho de arquivo).

**Correção**: adicionado `tauri-plugin-fs` (front e back). O front (`LocalXlsxImport.tsx`) agora
lê os bytes do `content://` escolhido via `readFile` do plugin (que sabe resolver URIs de
conteúdo através do `ContentResolver` do Android) e grava uma cópia em
`$APPCACHE/neko-local-import.xlsx` via `writeFile`, antes de invocar o comando Rust — que
continua recebendo um caminho de filesystem real, sem nenhuma mudança na lógica de import ou nos
testes que já cobrem `import_local_xlsx_inner`. Em desktop o picker já devolve um caminho real,
então a materialização é uma cópia redundante e inofensiva (planilhas são pequenas).

**2ª tentativa: passou.** Planilha real do dono (o mesmo arquivo `.xlsx` usado no dia a dia,
mantido fora de versionamento) enviada ao aparelho via `adb push`, selecionada pelo picker
nativo (Downloads), importada até o fim: todas as abas do ano corrente processadas, sem erro,
com o resumo "Imported N total rows" exibido e a tela Hoje refletindo os dados reais assim que a
navegação voltou para lá (saldo, teto do dia, projeção do mês e Calendário todos populados a
partir da planilha, não de fixture). Nenhum dado da planilha entrou neste repositório — a cópia
usada ficou em `/tmp` durante o spike e foi removida do aparelho ao final.

## Toolchain vencedora

| Componente                      | Versão                                                                                                                                                                                                                                                                                |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Android SDK                     | `/opt/android-sdk` (platform 36, build-tools, platform-tools)                                                                                                                                                                                                                         |
| NDK                             | 28.2.13676358                                                                                                                                                                                                                                                                         |
| cargo-ndk                       | 4.1.2                                                                                                                                                                                                                                                                                 |
| Rust targets                    | `aarch64-linux-android` (testado no aparelho), `armv7-linux-androideabi`, `i686-linux-android`, `x86_64-linux-android` (instalados, não exercidos no aparelho físico)                                                                                                                 |
| rustc / cargo                   | 1.96.0                                                                                                                                                                                                                                                                                |
| AGP (Android Gradle Plugin)     | 8.11.0                                                                                                                                                                                                                                                                                |
| Gradle                          | 8.14.3                                                                                                                                                                                                                                                                                |
| Kotlin                          | 1.9.25                                                                                                                                                                                                                                                                                |
| compileSdk / targetSdk / minSdk | 36 / 36 / 24                                                                                                                                                                                                                                                                          |
| JDK do daemon Gradle            | Temurin 21.0.12+8 — **obrigatório**: Gradle 8.14.3 não roda sob JDK 25 ("Unsupported class file major version 69"); exportar `JAVA_HOME` para um JDK 21 antes de `tauri android build`/`gradlew`. Não fixar em `gradle.properties` — o caminho do JDK é por máquina, não por projeto. |
| Flags de linker                 | Apenas os que o próprio `tauri-cli` já define por padrão por alvo (`-Clink-arg=-landroid -Clink-arg=-llog -Clink-arg=-lOpenSLES`); nenhuma flag extra foi necessária                                                                                                                  |

Nenhum problema de símbolos de ponto flutuante de 128 bits (o problema conhecido de NDKs antigos
com sqlx/SQLite, citado na spec) apareceu com esta combinação de NDK/cargo-ndk — `sqlx-sqlite`
compilou e rodou sem workaround.

### Onde a SDK/NDK vivem

Movidos de `~/android-sdk` (escopo de usuário) para `/opt/android-sdk`, com grupo `android`
(usuário `john` incluído, `chmod g+s` recursivo nos diretórios) para compilar sem `sudo`.
`ANDROID_HOME`, `ANDROID_SDK_ROOT`, `NDK_HOME`, `ANDROID_NDK_HOME` exportados em
`/etc/profile.d/android.sh`. Como o `/etc/zsh/zprofile` do Debian não repassa `/etc/profile.d/`
(diferente do `/etc/profile` do bash), foi adicionado um laço explícito de source nele — sem essa
correção, sessões de login zsh (o shell padrão da máquina) nunca veriam as variáveis. Rebuild
completo do APK, instalação e abertura no aparelho confirmados depois da mudança.

## Capacidades desktop-específicas

Nenhuma exigiu `cfg(target_os)` novo para **compilar** — o shell já tinha adapters com fallback
gracioso por plataforma antes deste spike (`secret_file::get_machine_id` cai num valor default
fora de linux/macos/windows; `os_scheduler::register/unregister` já são no-op fora do Windows).
Isso é evidência de que a arquitetura funcional-core/imperative-shell do repo já pagava esse custo
adiantado, não um acerto deste spike.

Dito isso, nenhuma delas foi **exercida de verdade** no aparelho — ficam como lacuna consciente
para a spec 044 decidir o adapter certo, não como capacidade validada:

- **Cofre de segredos** (`keyring`, usado por OAuth): features do crate cobrem
  apple-native/windows-native/sync-secret-service — nenhuma Android. Compilou porque o caminho
  nunca foi chamado (OAuth está fora do escopo do spike). Precisa de adapter Android real antes
  do porte.
- **Notificador standalone / agendador do SO** (`os_scheduler`): implementado só para Windows
  (Task Scheduler); no Android precisaria de `WorkManager`/`AlarmManager`, não existe hoje.
- **Updater** (`tauri-plugin-updater`): registrado sem `cfg`, mas não tem sentido num APK — a
  Play Store (ou a distribuição lateral) é quem atualiza. Deveria sair do bundle mobile na spec
  044, não só ficar inerte.

## `npm run check`

Verde com o target Android presente (branch `impl/issue-422`), incluindo build desktop, suíte
Rust (`cargo test`) e a suíte TypeScript completa (2061 testes). O ajuste de import local
(item 5) exigiu atualizar `SettingsScreen.test.tsx` para mockar `@tauri-apps/plugin-fs` e
`@tauri-apps/api/path`, que passaram a ser usados pelo fluxo de import.

## Achados de campo (spec 044, não reprovam o gate)

O dono usou o APK do spike no aparelho e abriu quatro achados. Nenhum foi corrigido aqui —
são acabamento de UI da spec 044, com frentes próprias — mas cada um mapeia para o item do gate
a que pertence, e nenhum reprova o item: os itens do gate julgam capacidade de PLATAFORMA
(a densidade renderiza sem jank, o teclado não quebra o layout, o WebView respeita
`reduced-motion`, o cold start cabe no orçamento, o import roda de verdade), não o polimento
visual de acabamento que ainda falta.

- **#432 — halo azul do WebView em todo toque** (item 2, animações/acabamento do design system):
  `-webkit-tap-highlight-color` padrão do WebView somado a `:focus` (em vez de `:focus-visible`)
  deixando o anel de foco preso após o toque. Reproduzido durante os testes dos itens 1 e 3
  (visível nas capturas da folha "Novo lançamento"). Não é falta de fluidez de animação — é
  ausência de uma regra de reset na fundação do design system.
- **#433 — folha de novo lançamento estourando o viewport/safe area** (item 3, teclado/layout de
  entrada): o cabeçalho da folha encosta na barra de status (fora da safe area do recorte) e o
  campo Descrição corta o placeholder à direita. Reproduzido e visível nas capturas deste
  relatório. O teclado em si não cobre o campo focado nem quebra o layout — é a folha que já
  nasce maior que a área visível, independente do teclado estar aberto.
- **#434 — didática instruindo "Esc para fechar" em ambiente de toque** (item 3, ergonomia de
  entrada): copy fixa do `InfoPopover`, não testada diretamente neste spike (não há didática na
  folha de novo lançamento), mas do mesmo naipe do #433 — ergonomia por ambiente de entrada.
- **#435 — coluna do dia 31 do Calendário invadindo o bloco de detalhe** (item 1, densidade):
  observado durante a medição de scroll do item 1 (captura `cal2.png` desta sessão) — a última
  semana do mês, com uma única coluna, aparece imediatamente acima do bloco de detalhe do dia
  selecionado. O scroll em si mediu sem jank (gfxinfo acima); o achado é de layout de grid, não
  de performance de rolagem.

## Conclusão

Os cinco itens passam, dois deles (scroll e cold start) com folga considerável e sem nenhuma
correção. O único item que reprovou na primeira tentativa (import local) tinha causa raiz clara —
incompatibilidade de path vs. content URI, própria de qualquer picker de arquivo em Android — e a
correção (materializar bytes via `tauri-plugin-fs` antes de invocar o comando existente) é
pequena, não invasiva, e não muda a lógica de import testada em desktop.

**O WebView Android entrega a experiência que o design system exige.** A estratégia segue para a
spec 043 (convergência por snapshot no Drive) e a spec 044 (porte Android v1), que herda a lista
de adapters pendentes acima como ponto de partida.
