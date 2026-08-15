# Changelog

All notable changes to Neko Finance are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), entries are written
for humans, and versions follow [SemVer](https://semver.org/).

## [0.3.1](https://github.com/johnlaff/NekoFinance/compare/v0.3.0...v0.3.1) (2026-08-15)


### Bug Fixes

* repõe no changelog o conserto do OAuth dos builds de release ([#465](https://github.com/johnlaff/NekoFinance/issues/465)) ([361dbc1](https://github.com/johnlaff/NekoFinance/commit/361dbc1f666301fd08358eb9df760ae54e44d009))

## [0.3.0](https://github.com/johnlaff/NekoFinance/compare/v0.2.1...v0.3.0) (2026-08-14)


### Features

* adiciona o check-out do snapshot no Drive ao abrir o app, com restauração atômica ([#443](https://github.com/johnlaff/NekoFinance/issues/443)) ([3676994](https://github.com/johnlaff/NekoFinance/commit/3676994255e3c0b794e0982c152f42a18d8655f7))
* **android:** adiciona o target Android e aprova o gate sensorial do spike (spec 042) ([#440](https://github.com/johnlaff/NekoFinance/issues/440)) ([d83b154](https://github.com/johnlaff/NekoFinance/commit/d83b154d43bf59d0580da09c7b57eced95085bf5))
* **android:** instala o app de produção com as onze telas no aparelho ([#441](https://github.com/johnlaff/NekoFinance/issues/441)) ([5ad3af6](https://github.com/johnlaff/NekoFinance/commit/5ad3af6191175887ca89b7ea22ac17cadd26f45f))
* liga os gatilhos automáticos de check-in e check-out do snapshot no Drive ([#449](https://github.com/johnlaff/NekoFinance/issues/449)) ([df829ad](https://github.com/johnlaff/NekoFinance/commit/df829adcd2c5b33dfe263abed72eaea4300d4398))
* primeiro check-in manual do snapshot no Drive, com manifest e lease ([#439](https://github.com/johnlaff/NekoFinance/issues/439)) ([71aaff1](https://github.com/johnlaff/NekoFinance/commit/71aaff18532577a46eaa84e6f7b9700e0f521ea7))
* resolve conflito de sincronização entre aparelhos com os gestos dos dois lados antes da escolha ([#447](https://github.com/johnlaff/NekoFinance/issues/447)) ([b679a38](https://github.com/johnlaff/NekoFinance/commit/b679a38230d06121823095e811ab4d3f71130e8e))


### Bug Fixes

* **android:** corrige o crash de abertura e verifica a Mia no aparelho real ([#445](https://github.com/johnlaff/NekoFinance/issues/445)) ([e24fbdb](https://github.com/johnlaff/NekoFinance/commit/e24fbdb91cf4b63678eb5e63a0b45f7aa344b08a))
* cobre o refresh do token OAuth pelo mesmo teto de espera do check-out no boot ([#461](https://github.com/johnlaff/NekoFinance/issues/461)) ([250bf3b](https://github.com/johnlaff/NekoFinance/commit/250bf3bed2c4e8f1d857e821a5534a0e7d354cbf)), closes [#460](https://github.com/johnlaff/NekoFinance/issues/460)
* estreita a guarda do próprio device_id no check-out para a janela base+1 ([#448](https://github.com/johnlaff/NekoFinance/issues/448)) ([cf22a7e](https://github.com/johnlaff/NekoFinance/commit/cf22a7e658a738e9b6be6a4b63dc4e8bb23d324d))
* fecha follow-ups pendentes do snapshot no Drive (issue [#446](https://github.com/johnlaff/NekoFinance/issues/446)) ([#459](https://github.com/johnlaff/NekoFinance/issues/459)) ([2d31963](https://github.com/johnlaff/NekoFinance/commit/2d319634c5bbfbb55e46b4741534c7b832684775))
* neutraliza o realce de toque do Android e ajusta a copy de fechar do InfoPopover por ambiente ([#437](https://github.com/johnlaff/NekoFinance/issues/437)) ([207cad1](https://github.com/johnlaff/NekoFinance/commit/207cad1080b9829651e184d07951254b950d9d94))
* robustece o check-out do snapshot no boot contra rede lenta e reabertura falha ([#452](https://github.com/johnlaff/NekoFinance/issues/452)) ([28ff444](https://github.com/johnlaff/NekoFinance/commit/28ff4441f94c5359516b3f6249383c9c31f35182))

## [0.2.1](https://github.com/johnlaff/NekoFinance/compare/v0.2.0...v0.2.1) (2026-08-09)


### Bug Fixes

* ancora o release train no commit da release 0.2.0 (bootstrap-sha) ([#396](https://github.com/johnlaff/NekoFinance/issues/396)) ([d9dc92e](https://github.com/johnlaff/NekoFinance/commit/d9dc92e1e9a8c57d853c6e4510c6e8464941416e))
* draft sem tag e dispatch explícito do build no release train ([#394](https://github.com/johnlaff/NekoFinance/issues/394)) ([3ac44a2](https://github.com/johnlaff/NekoFinance/commit/3ac44a2f73af865d3ca5e9e349f5264b299d94f4))

## [0.2.0](https://github.com/johnlaff/NekoFinance/compare/neko-finance-v0.1.0...neko-finance-v0.2.0) (2026-08-09)


### Features

* adiciona o bloco de estado do update nas Configurações ([#391](https://github.com/johnlaff/NekoFinance/issues/391)) ([ac214d5](https://github.com/johnlaff/NekoFinance/commit/ac214d56ca74f7d77d319fb8a53df1f6d3a27e14))
* assinatura do updater no pipeline de release ([#387](https://github.com/johnlaff/NekoFinance/issues/387)) ([102c353](https://github.com/johnlaff/NekoFinance/commit/102c3530b47043c28229084aa6e00cc1bae9a468))
* bootstrap Neko Finance — local-first personal finance desktop app ([ff0e18c](https://github.com/johnlaff/NekoFinance/commit/ff0e18cbce88a0797127717eeb400d09ac20f7f5))
* **brand+perf:** real app icon and startup/render fluidity ([35f861b](https://github.com/johnlaff/NekoFinance/commit/35f861bdf3969e189a6625e33d77a0a2e3ad17ee))
* **build:** single-file Windows exe — MSVC/cargo-xwin as the local default ([82c635b](https://github.com/johnlaff/NekoFinance/commit/82c635bfe7f3a72c3ab8a32ffc2e318fb84d3c8f))
* catálogo de evals e o runner de bancada — a conversa medida pelo código que a serve ([#260](https://github.com/johnlaff/NekoFinance/issues/260)) ([9f23af7](https://github.com/johnlaff/NekoFinance/commit/9f23af7f7b6566407b9ffda95531798b609ed286))
* consentimento fail-closed e a chave no cofre do sistema ([#258](https://github.com/johnlaff/NekoFinance/issues/258)) ([7fe7675](https://github.com/johnlaff/NekoFinance/commit/7fe7675fbba2a5c85e1b1cc8e6c6f243c11d4df9)), closes [#237](https://github.com/johnlaff/NekoFinance/issues/237)
* convenção de notas #dividir:/#reembolso: com Entrada de reembolso net-zero (plano 023) ([#43](https://github.com/johnlaff/NekoFinance/issues/43)) ([61a41c1](https://github.com/johnlaff/NekoFinance/commit/61a41c1fd949ba357e6ad3b4dc312df8de8ebd21))
* convite de update no launch ([#390](https://github.com/johnlaff/NekoFinance/issues/390)) ([9026016](https://github.com/johnlaff/NekoFinance/commit/9026016e4f7dfab05c8668ca769c6f2eab0ce798)), closes [#382](https://github.com/johnlaff/NekoFinance/issues/382)
* Diário por categoria + calendário de vencimentos + parcelas (plano 045) ([#68](https://github.com/johnlaff/NekoFinance/issues/68)) ([26ea4c9](https://github.com/johnlaff/NekoFinance/commit/26ea4c9e4187adf48073eb9b1f30bedd11a0e579))
* **dogfooding:** leitor fiel da planilha — seed, guardrail anual, previsibilidade, notas ([#11](https://github.com/johnlaff/NekoFinance/issues/11)) ([45f687f](https://github.com/johnlaff/NekoFinance/commit/45f687f287cb5e6bc9523cc3ceba093ea97d3c4c))
* dupla-realidade na UX — teto do Diário + quick-add credit-first + badge Economizado (plano 038) ([#60](https://github.com/johnlaff/NekoFinance/issues/60)) ([63e2081](https://github.com/johnlaff/NekoFinance/commit/63e20816942d10a68bc86d4dbba918b4ca79d445))
* editar/excluir lançamentos, séries de recorrência e totais por titular (plano 015) ([#27](https://github.com/johnlaff/NekoFinance/issues/27)) ([362a1a7](https://github.com/johnlaff/NekoFinance/commit/362a1a7edc8d5c43202a6cd1d23f4339f9ed82b7))
* **engine:** align 5-type finance buckets ([#91](https://github.com/johnlaff/NekoFinance/issues/91)) ([a0c0ebd](https://github.com/johnlaff/NekoFinance/commit/a0c0ebd82e789e5301e114967b65238f1ea07031))
* fachada da conversa — as análises temporais ([#247](https://github.com/johnlaff/NekoFinance/issues/247)) ([0bb6563](https://github.com/johnlaff/NekoFinance/commit/0bb65632b2b079c8cfb37b4c70834911895bb7f9)), closes [#232](https://github.com/johnlaff/NekoFinance/issues/232)
* fachada da conversa — cenário efêmero, capítulos do método e manifesto de paridade ([#252](https://github.com/johnlaff/NekoFinance/issues/252)) ([5b6e905](https://github.com/johnlaff/NekoFinance/commit/5b6e90517f1fbbd7652c5ad548a10e97c5e62836)), closes [#234](https://github.com/johnlaff/NekoFinance/issues/234)
* fachada da conversa — o recorte de lançamentos, as tags e os compromissos ([#251](https://github.com/johnlaff/NekoFinance/issues/251)) ([23722d1](https://github.com/johnlaff/NekoFinance/commit/23722d182664a5a2d5ee3aeae8e0b256c17dfd78))
* fachada da conversa — porta única, envelope e as ferramentas de estado ([#246](https://github.com/johnlaff/NekoFinance/issues/246)) ([cf3ebc3](https://github.com/johnlaff/NekoFinance/commit/cf3ebc3250dd1490f7708b347307f3fa978b9850)), closes [#231](https://github.com/johnlaff/NekoFinance/issues/231)
* facilidade diária — lembrete via agendador do SO + "Sincronizar" de 1 clique (plano 039) ([#61](https://github.com/johnlaff/NekoFinance/issues/61)) ([d3922d2](https://github.com/johnlaff/NekoFinance/commit/d3922d2294de4f8e10895f5521298b3aa5ae48a5))
* **forecast:** daily projection view on the dashboard (spec 005) ([54841ab](https://github.com/johnlaff/NekoFinance/commit/54841ab2e6175cb0ad77bef06b21fb92dd14b858))
* guardas de segurança do write-back (Fase 2, PR-A — flag ainda desligado) — plano 028 ([#47](https://github.com/johnlaff/NekoFinance/issues/47)) ([c5f9095](https://github.com/johnlaff/NekoFinance/commit/c5f909516842c6940157e90354714574721d2d75))
* habilita o write-back aprovado por humano (Fase 2, PR-B — flip do flag) — plano 028 ([#48](https://github.com/johnlaff/NekoFinance/issues/48)) ([bf92101](https://github.com/johnlaff/NekoFinance/commit/bf92101fe14dfc62727856a1a73c34ed4d10759d))
* **import:** add 5-type section classifier ([#90](https://github.com/johnlaff/NekoFinance/issues/90)) ([8fd40a9](https://github.com/johnlaff/NekoFinance/commit/8fd40a960cdbb7ac21af79e45bc9153b858c7585))
* importar atribuição por titular e método crédito das notas de célula (plano 004) ([#29](https://github.com/johnlaff/NekoFinance/issues/29)) ([884e753](https://github.com/johnlaff/NekoFinance/commit/884e753d8dab64a066e6e17d6b4f48f5f13de715))
* itemização — editar partes (passado/previsto/novo) + write-back round-trip (plano 036) ([#59](https://github.com/johnlaff/NekoFinance/issues/59)) ([407cdb0](https://github.com/johnlaff/NekoFinance/commit/407cdb04322a97e4ac6644d3b19bf2949a05bd0b))
* itemização — ver a quebra de Saídas/Entradas (passado/previsto/novo) (plano 035) ([#58](https://github.com/johnlaff/NekoFinance/issues/58)) ([61c0524](https://github.com/johnlaff/NekoFinance/commit/61c0524e96f5668362179caeec593045f33913b3))
* laço da conversa — a rodada completa e as 12 invariantes ([#257](https://github.com/johnlaff/NekoFinance/issues/257)) ([2bcdf20](https://github.com/johnlaff/NekoFinance/commit/2bcdf20819fbddea380bdf80d9bd5807794ea6a4))
* **lancamentos:** show classified line item badges ([#92](https://github.com/johnlaff/NekoFinance/issues/92)) ([4ee968c](https://github.com/johnlaff/NekoFinance/commit/4ee968c9729edb280352c3325e549253353b37bc))
* lançar Economia manualmente (transferência para a reserva) — plano 003 ([#28](https://github.com/johnlaff/NekoFinance/issues/28)) ([866a676](https://github.com/johnlaff/NekoFinance/commit/866a6762edff0c9947b36e79dbf56a51a2e56b46))
* lembrete diário (notificação do SO) + indicador "última vez que lançou" (plano 030) ([#51](https://github.com/johnlaff/NekoFinance/issues/51)) ([21ac5cb](https://github.com/johnlaff/NekoFinance/commit/21ac5cbc51fdb3b8a3fec9e17fd2f21a551b7bbf))
* motion 2026 — stagger nas listas + durações nos tokens + fix de flash (plano 018) ([#40](https://github.com/johnlaff/NekoFinance/issues/40)) ([51afe33](https://github.com/johnlaff/NekoFinance/commit/51afe33d0626ef887dff17a2af85d17ff27097a9))
* nav/IA enxuta para o check-in diário + aparo de redundância (plano 016) ([#39](https://github.com/johnlaff/NekoFinance/issues/39)) ([77155b9](https://github.com/johnlaff/NekoFinance/commit/77155b9c1da59babf147475a1901af7fdad9cf8d))
* onda Mia — a conversa com recibo auditável ([#229](https://github.com/johnlaff/NekoFinance/issues/229)) ([06c9b20](https://github.com/johnlaff/NekoFinance/commit/06c9b20a729dcf08c48cbc4bad3e6418039823a2))
* **pockets:** liquidity-aware accounts — projected balance counts only liquid cash (spec 007) ([#8](https://github.com/johnlaff/NekoFinance/issues/8)) ([1aef73d](https://github.com/johnlaff/NekoFinance/commit/1aef73d96aec35896417970ddd35e9825a44ebdc))
* prompt de sistema — o núcleo do método como prefixo estável ([#259](https://github.com/johnlaff/NekoFinance/issues/259)) ([035270c](https://github.com/johnlaff/NekoFinance/commit/035270cb948d51f0ce2000e2d09b9e84966e8b0e)), closes [#238](https://github.com/johnlaff/NekoFinance/issues/238)
* quick-add diário sem fricção — descrição + 5 tipos + atalho global (plano 029) ([#50](https://github.com/johnlaff/NekoFinance/issues/50)) ([039e416](https://github.com/johnlaff/NekoFinance/commit/039e416e755f07595acddb515a82cf007692a69f))
* reconcile design system with current app + sync to claude.ai/design ([#83](https://github.com/johnlaff/NekoFinance/issues/83)) ([39b74eb](https://github.com/johnlaff/NekoFinance/commit/39b74ebf483ec056ca1161022d02636d1f4f1216))
* redesign the desktop app end-to-end — re-architected IA, 9 reimagined screens, zero-warning quality bar ([#84](https://github.com/johnlaff/NekoFinance/issues/84)) ([2ee07a8](https://github.com/johnlaff/NekoFinance/commit/2ee07a892bd50ab86ba00946304a5e7646d3a05d))
* sincronização em segundo plano com o Sheets — Fase 1 read-side (plano 026) ([#45](https://github.com/johnlaff/NekoFinance/issues/45)) ([7041f6e](https://github.com/johnlaff/NekoFinance/commit/7041f6e6762b170be3a83809aa7a6b31d278ab41))
* **sync:** write auto-derived economia to the Economia tab ([50fc25e](https://github.com/johnlaff/NekoFinance/commit/50fc25eb5a71650277f150a97fb93a76dd32bbd6))
* toggle "Ignorar nos cálculos" nas tags (plano 034) ([#55](https://github.com/johnlaff/NekoFinance/issues/55)) ([e62ecb6](https://github.com/johnlaff/NekoFinance/commit/e62ecb61827c79897314ee3786c20808915d2bb7))
* **ui:** motion layer, circular-reveal theme switch, command cache (spec 006) ([0bbc782](https://github.com/johnlaff/NekoFinance/commit/0bbc7823a3cf4f4c7c70ddb0ba7b0f4c38fb7435))
* update skills ([#227](https://github.com/johnlaff/NekoFinance/issues/227)) ([950cc35](https://github.com/johnlaff/NekoFinance/commit/950cc35039fee135e19d8aa8d22144473474e941))
* update skills ([#82](https://github.com/johnlaff/NekoFinance/issues/82)) ([9a97e0d](https://github.com/johnlaff/NekoFinance/commit/9a97e0d8373738483c2e3f3eee9eeb5ccea72fe8))
* views perdidas da planilha — grade dos 12 meses + Economia 2 anos lado-a-lado (plano 044) ([#67](https://github.com/johnlaff/NekoFinance/issues/67)) ([3eeb2fe](https://github.com/johnlaff/NekoFinance/commit/3eeb2fe54a75cde0f58992e5df4115204087509c))
* visão fiel da planilha — mês/Totais/Anual/tags + engine 5 tipos, OAuth/write-back ([#13](https://github.com/johnlaff/NekoFinance/issues/13)) ([abed72d](https://github.com/johnlaff/NekoFinance/commit/abed72d850c2cf8612d9a1db57e0d88a519d0abe))
* write-back visível no dashboard — pendentes + conflitos (plano 031) ([#52](https://github.com/johnlaff/NekoFinance/issues/52)) ([72ff81a](https://github.com/johnlaff/NekoFinance/commit/72ff81ab2a3db2900a796aa879c865d65f10ea0a))


### Bug Fixes

* aderência — piso de reserva = custo de vida × meses + Economizado% unificado (plano 033) ([#54](https://github.com/johnlaff/NekoFinance/issues/54)) ([431ec26](https://github.com/johnlaff/NekoFinance/commit/431ec269906b1a42c2633b1f0162d5f27b6b6041))
* atomicidade do update_transaction + ABS no baseline de reserva (plano 049) ([#74](https://github.com/johnlaff/NekoFinance/issues/74)) ([0856655](https://github.com/johnlaff/NekoFinance/commit/08566558c723b86639a99733d2addaff3fd898ee))
* **build:** normalize line endings via .gitattributes ([e2a8bb1](https://github.com/johnlaff/NekoFinance/commit/e2a8bb1c99d0c02b8773bcfc50c2fd86a23ab949))
* bumpa o Cargo.lock junto do Cargo.toml no release train ([#393](https://github.com/johnlaff/NekoFinance/issues/393)) ([460d498](https://github.com/johnlaff/NekoFinance/commit/460d498abaebd53b0cdf2104effb549f19823ed9))
* bundle de correções — daily_spend SUM(ABS) + guarda amount + audit/clamp (plano 053) ([#79](https://github.com/johnlaff/NekoFinance/issues/79)) ([131d5fe](https://github.com/johnlaff/NekoFinance/commit/131d5fe90523d9381203c1fff2942e651045661c))
* **cenários:** eventos hipotéticos de hoje entram no encadeamento de saldo ([#159](https://github.com/johnlaff/NekoFinance/issues/159)) ([5dc88e6](https://github.com/johnlaff/NekoFinance/commit/5dc88e61bd4a4adba8a58306c08d82c665c40226))
* **cenários:** valida a fronteira do empréstimo e bloqueia confirmação com prévia ilegível ([#160](https://github.com/johnlaff/NekoFinance/issues/160)) ([2e90091](https://github.com/johnlaff/NekoFinance/commit/2e9009178943296033ee984ccb6324840b5b1dfe))
* cinco arestas de correção do engine/import/efeitos (plano 007) ([#33](https://github.com/johnlaff/NekoFinance/issues/33)) ([0a5083f](https://github.com/johnlaff/NekoFinance/commit/0a5083f55948f5b6fcc9a291af8189eac01545a4))
* **compose:** use credit payment method for cartao ([#87](https://github.com/johnlaff/NekoFinance/issues/87)) ([1617d46](https://github.com/johnlaff/NekoFinance/commit/1617d468e9d90445e92b385c1401d447f8237e6a))
* correções de fluxo P1/P2 no write-back e sync (plano 032) ([#53](https://github.com/johnlaff/NekoFinance/issues/53)) ([5e3c23b](https://github.com/johnlaff/NekoFinance/commit/5e3c23b5516ca39270479a0d6588dca821a184c1))
* correções forecast/import — is_projection (hoje/edição/checksum) + sinal do month_grid (plano 041) ([#64](https://github.com/johnlaff/NekoFinance/issues/64)) ([c7eb35d](https://github.com/johnlaff/NekoFinance/commit/c7eb35df27abf5455ad5565fac450cdfd7db7468))
* correções P1 do Pacote D — projeção de Saldo, audit do lump de cartão, 1º-jan (plano 037) ([#57](https://github.com/johnlaff/NekoFinance/issues/57)) ([0e87fe0](https://github.com/johnlaff/NekoFinance/commit/0e87fe0093767dabb776263207acaac199e81ecd))
* economia=Saída finalizado — aba Economia vira anotação (sem double-count) (plano 052) ([#78](https://github.com/johnlaff/NekoFinance/issues/78)) ([875f5be](https://github.com/johnlaff/NekoFinance/commit/875f5bef824b8e7426691786e67e2bd02cc1cd29))
* endurecimento de segurança (token fail-closed, accept timeout, CSP, privacy-scan) — plano 013 ([#34](https://github.com/johnlaff/NekoFinance/issues/34)) ([061e6c2](https://github.com/johnlaff/NekoFinance/commit/061e6c216a7cf6cbefdbf1bd2d818ddec176d481))
* fase "Operar" exige reserva &gt;= 6 meses (piso do método) — plano 006 ([#32](https://github.com/johnlaff/NekoFinance/issues/32)) ([04ef4a6](https://github.com/johnlaff/NekoFinance/commit/04ef4a6263fd5ea80f053b93c5e147cc4227ff80))
* fidelidade de anotação — cabeçalhos de seção no round-trip + termômetro −R$500 (plano 048) ([#72](https://github.com/johnlaff/NekoFinance/issues/72)) ([2132297](https://github.com/johnlaff/NekoFinance/commit/2132297b650ea5d5c715e4b32fbbf047ab69c1e0))
* formata docs/version-matrix.md com prettier (destrava o format:check do CI) ([#112](https://github.com/johnlaff/NekoFinance/issues/112)) ([cd732ae](https://github.com/johnlaff/NekoFinance/commit/cd732aebca39b2e1b16ed9fd2ba712198f6023bb))
* guardrail de poupança usa a Economia registrada, não o superávit-líquido (plano 005) ([#30](https://github.com/johnlaff/NekoFinance/issues/30)) ([4853c38](https://github.com/johnlaff/NekoFinance/commit/4853c3821f395f2c78adf995c95a67d4739a9532))
* integridade de edição/UX — editar/apagar importado + itens em série + copy (plano 043) ([#66](https://github.com/johnlaff/NekoFinance/issues/66)) ([c160a10](https://github.com/johnlaff/NekoFinance/commit/c160a1070661132b2adab99ee28ac9ca33b70385))
* **lancamentos:** default to monthly view ([#88](https://github.com/johnlaff/NekoFinance/issues/88)) ([24aaa84](https://github.com/johnlaff/NekoFinance/commit/24aaa8453dd15c09578973d8940655e322814707))
* **lancamentos:** items of income rows show the Entrada badge ([#105](https://github.com/johnlaff/NekoFinance/issues/105)) ([ffefd15](https://github.com/johnlaff/NekoFinance/commit/ffefd151ec8c737b51f05db9e35efeabb78034c3))
* limpeza de órfãos no delete (P1) + 4 correções de fluxo (plano 047) ([#71](https://github.com/johnlaff/NekoFinance/issues/71)) ([3299b74](https://github.com/johnlaff/NekoFinance/commit/3299b743336a89a9e2bda1be79f8f10d439dee64))
* nits finais de consistência — SUM(ABS) no daily_ceiling + paridade tag-Ignorar no annual savings (plano 054) ([#80](https://github.com/johnlaff/NekoFinance/issues/80)) ([6bae118](https://github.com/johnlaff/NekoFinance/commit/6bae118f444b151c30cf5bb7cbc2acc98befaa3f))
* parse and write-back all side-by-side Economia year blocks ([#23](https://github.com/johnlaff/NekoFinance/issues/23)) ([bb815dd](https://github.com/johnlaff/NekoFinance/commit/bb815dd6f0b195567fd78f28464a97af79e556c7))
* Performance = fórmula da planilha (Entradas − (Saídas + Diário)) — plano 040 ([#63](https://github.com/johnlaff/NekoFinance/issues/63)) ([562b1e6](https://github.com/johnlaff/NekoFinance/commit/562b1e6915845ba8436d36858f13006a0a1c9832))
* Performance = income − cost_of_living (economia=Saída, decisão TRAVADA) — plano 051 ([#76](https://github.com/johnlaff/NekoFinance/issues/76)) ([5ada3ae](https://github.com/johnlaff/NekoFinance/commit/5ada3ae8197aa965c9408d0b8d073c4d4ac66abc))
* Performance volta a descontar a economia (fiel à planilha) — plano 046 (corrige o 040) ([#70](https://github.com/johnlaff/NekoFinance/issues/70)) ([2785325](https://github.com/johnlaff/NekoFinance/commit/2785325939edbf2f19f2f3916a001ff9656f2a05))
* restore Google Sheets auto-sync (bake OAuth secret for background refresh + wire Reconnect) ([#85](https://github.com/johnlaff/NekoFinance/issues/85)) ([da2d3e9](https://github.com/johnlaff/NekoFinance/commit/da2d3e98791dc94ea1dba2a9d85e53ed5b8f0d02))
* revisão adversarial (rodadas 7–8) — privacidade, fidelidade do método e polimento de UI/UX/docs ([#14](https://github.com/johnlaff/NekoFinance/issues/14)) ([b0a26b5](https://github.com/johnlaff/NekoFinance/commit/b0a26b5e3b81854a9db88fe92caa67538e4af406))
* revisão completa da app (rodada 9) — bugs, atomicidade, segurança, a11y e CI/CD ([#21](https://github.com/johnlaff/NekoFinance/issues/21)) ([d183bbf](https://github.com/johnlaff/NekoFinance/commit/d183bbf340d24bad4c1246b0a5efffcde3b84d6c))
* round-trip manual→write-back→re-import sem duplicata + 2 nits (plano 055) ([#81](https://github.com/johnlaff/NekoFinance/issues/81)) ([fb496c8](https://github.com/johnlaff/NekoFinance/commit/fb496c8aac948365f10b19bbe37ecac35d92af6f))
* **screens:** restore historical balances ([#89](https://github.com/johnlaff/NekoFinance/issues/89)) ([2b22468](https://github.com/johnlaff/NekoFinance/commit/2b22468b286dc5f6d2d9cc4833071abbbcb99e30))
* staleness sempre-on no write-back de Economia + saw_december no parse (plano 050) ([#75](https://github.com/johnlaff/NekoFinance/issues/75)) ([17edfcb](https://github.com/johnlaff/NekoFinance/commit/17edfcbd072539a8a8214749c24d627297ad5cb7))
* WCAG AA contrast + landmarks (a11y batch, plan 017) ([#26](https://github.com/johnlaff/NekoFinance/issues/26)) ([e486f90](https://github.com/johnlaff/NekoFinance/commit/e486f90d23cb58d0f38dc394bfa08b93eb1ea0de))
* wrap sheet import in a single SQLite transaction ([#24](https://github.com/johnlaff/NekoFinance/issues/24)) ([9464050](https://github.com/johnlaff/NekoFinance/commit/9464050bde95d51e067f4e3486c222a52f3455f2))
* write-back/sync — invalidate do fast-path + TOCTOU do preview (+ teste audit Economia) (plano 042) ([#65](https://github.com/johnlaff/NekoFinance/issues/65)) ([ff350a6](https://github.com/johnlaff/NekoFinance/commit/ff350a6382c50aa53d739a3492c3542052b72597))


### Performance Improvements

* bulk-insert do balance series + filtros de data index-friendly (plano 009) ([#36](https://github.com/johnlaff/NekoFinance/issues/36)) ([76aeb87](https://github.com/johnlaff/NekoFinance/commit/76aeb8751a6dd78c4b409c38f900ea6ebbdf5e4a))
* dashboard reuses the shared forecast cache (no cold re-fetch per visit) ([#25](https://github.com/johnlaff/NekoFinance/issues/25)) ([30a69d7](https://github.com/johnlaff/NekoFinance/commit/30a69d7a4b832c68b2106f9c715a5e5ecb20d5d8))

## [Unreleased]

### Added

- `mia-bench bakeoff` measures the pinned model matrix and decides which one becomes
  the conversation's default — the choice moved from intuition to measurement. A live
  canary verifies every pin against the provider's zero-retention catalog before any
  paid round; a cost probe runs one repetition per model and refuses upfront when the
  whole design would not fit the cap; a one-repetition sieve runs every candidate plus
  the reference ceiling, and a three-repetition final runs the survivors. Nothing is
  decided on partial measurement: the sieve must cover every cleared pin and the final
  every selected finalist. Blind-judgment answers go to a separate sheet that names no
  model, and `mia-bench julgar` closes the loop offline, writing the final decision into
  the report. Adopting the pin stays a manual gesture.

- The Horizonte screen became the cash radar: the only view that looks strictly
  forward (projected, in months, to the end of the data), answering the method's
  question — is there a hole in the road? It opens on a three-voice verdict (clear
  path · a squeeze ahead · nothing booked yet) with the smallest point named and
  the honest twin (where December ends if the un-ballasted months cost the usual).
  The road to December draws the booked line, the un-ballasted zone, the dotted
  "if it costs the usual" trace, and the zero and low point, with a numbers fold.
  A twelve-month signal grid colours each month by its end-of-month balance band
  and carries the three epistemic states (lived · projected-with-ballast · review ·
  no record), each month opening in the Calendar. The projected commitments group
  by month (installment `n/N`, reimbursement as a linked income), and the "E se?"
  entry carries the two financing-gate rulers. Delineated from its neighbours: O
  ano judges the method, Horizonte guards the cash, and Hoje's "can spend" is
  proven by the horizon's lowest point. The ballast rule, typical spend, trust
  frontier and typical trace all come from the forecast engine — no backend change.
- The Teto do diário screen became the record of a decision with proof: it opens
  on the ceiling itself with the detected spending mode stating what the day is
  measured against, then shows the ceremony that produced it (the variable-month
  items, the `total ÷ days` formula rounded up, and the original spreadsheet note
  reproduced verbatim behind a disclosure), the age of that ceremony against the
  method's three-month cadence, and how the day reads the ceiling. Editing is no
  longer the screen's permanent state: it became a three-beat rite on the surface
  (items → divisor → before/after acceptance), with a guard that explains the
  consequence when the new ceiling is lower and still lets it through, a guided
  five-question ceremony for whoever has no ceiling yet, and a calm inline refusal
  when the divisor is missing. The spreadsheet proposal moved from a banner into a
  verdict state of its own, and the ceiling's provenance (the note and the month
  the ceremony was made) now survives the import.
- The O ano screen became the place where the method actually judges: it opens
  on a verdict and the band ruler (fixed 0–40% scale with the 20–30% target
  zone) instead of KPI tiles and a seven-column table, because the savings rate
  is only meaningful as a yearly average. A ballast test now gates that verdict
  — a month ahead only supports it when its booked outflow reaches 60% of
  typical spend; below that the month is flagged for review and the verdict
  falls back to what was actually lived, with the sample size printed on the
  ruler so it never claims to measure a full year from a few months. "Where
  December ends" projects the year in two scenarios (as booked, and if the
  flagged months cost the usual), the twelve months read as one row each
  (income rail, savings fill, 20% tick), the yearly numbers moved into an
  expandable list, and income is compared across years. On desktop the verdict
  and ruler span full width with the supporting cards in a two-column bento.
- The Hoje screen was recomposed around the daily verdict: a greeting hero
  ("Pode gastar hoje …" with the binding guardrail named and a teaching
  layer), the assistant's curation line, a day block that in card mode shows
  open invoices grouped by due date (per-card lines with status context,
  reimbursement tag and an honest footer for idle cards), a month insight in
  Mia's voice derived from the projected balance chain, upcoming movements
  (bills plus the next expected income), and a saldo + reserve pair with
  gauges. Desktop composes in two columns; mobile stacks under a large title
  coordinated with the app bar.
- Credit cards are now a first-class domain: register multiple cards (with
  additional cards per person inheriting the holder's cycle), track persisted
  invoices per card × cycle with derived status, and follow subscriptions and
  installments that pre-launch into future statements.
- New Cartões screen: card list with next due dates, invoice drill-down
  (purchases, series, linked reimbursements, reconciliation line, per-person
  sub-totals), direct statement-total adjustment, and card proposals surfaced
  from the spreadsheet with explicit accept/dismiss.
- Import recognizes card lines in cell notes by alias, materializes future
  invoices even when the cell total is zero, and never creates a card
  silently — unknown aliases become pending proposals.
- The card-mode gate now has two computable legs (savings alive **and**
  reserve ≥ 6 months), each reported honestly with its own state.

### Changed

- The public build variable that carries the Google desktop-client credential is
  now `VITE_GOOGLE_DESKTOP_CLIENT_KEY`. A local `.env` that still names it
  `VITE_GOOGLE_CLIENT_SECRET` silently loses background token refresh — rename the
  key, the value is unchanged. Anything a `VITE_` prefix inlines into the browser
  bundle is published, so the name no longer promises a secrecy it cannot keep.
- "Performance acumulada" is gone from the product. Summing monthly performance
  over a year is not something the method does — once you start setting money
  aside, savings is the number that matters, not the leftover. The yearly
  reading it used to occupy is now "where December ends", projected in two
  scenarios. The "Comparar anos" tab went with it: the comparison the method
  asks for is income across years, so that now lives on the screen permanently
  instead of behind a tab.
- The "Estimativa" mark no longer stacks a dotted underline on top of its pill
  border — the chip alone reads as tappable, and doubling the signal made the
  line noisy wherever the mark appears.
- Tags now carry four independent exclusion flags (Performance, Custo de vida,
  Economia, Diário médio) instead of a single all-or-nothing toggle — each flag
  controls whether a tagged movement counts in that ruler. The balance chain
  remains untouched (Saldo always reflects real cash movement). The Tags screen
  is now a ruler-exception panel: it displays the cost-of-living verdict with
  current exclusions, third-party movement aggregations, and per-tag effects on
  each ruler.
- The Configurações screen was recomposed under the identity direction: it
  opens with a trust verdict ("Tudo neste dispositivo" plus a live state line
  that reports disconnection, pending changes and import conflicts with the
  same weight as good news), organizes everything into Conexão, Privacidade,
  Bolsos, Aparência and Rotina sections, and folds the dense spreadsheet
  panel and local import behind a "Gerenciar" door. A dark-theme switch now
  lives in Aparência next to the accent selector, and the design system
  gained a proper Switch control whose off state stays visible in the light
  theme.
- Registering from Hoje now always goes through the compose flow (dock FAB,
  sidebar CTA or the N shortcut) with explicit approval — the day block is a
  reading surface and no longer embeds a quick-register form.
- Write-back now writes one note line per card in the due-date cell instead
  of collapsing all credit into a single card's lump; the multi-card warning
  is gone because the limitation is gone.
- The forecast derives credit events from invoices (per card, by due date)
  for open and future cycles; realized history still follows the spreadsheet.

### Fixed

- Purchases made after a card's closing day are now assigned to the cycle
  that closes the following month; previously they could land on an invoice
  whose due date preceded the purchase itself.
- Import recognizes a card reimbursement by identity, not only by the
  `#reembolso:` note marker: an income naming a card in the lexicon on that
  invoice's due date is linked to it. The marker keeps precedence because it
  carries who reimburses, inference only acts when the reimbursement accounts
  for the whole income, and a link the owner removed is never recreated.
- The open-invoice block reads the net commitment, labelling the part that
  comes back, and ranks cards by it. Auditable receipts stay gross: the net
  reading exists only where it is marked.
- A zero-valued invoice no longer counts as an open one — it leaves the day's
  list, the card counter and the total, and no longer hides a real invoice of
  the same card. The row stays recorded, and the card's history still shows
  the cycle.
- The cash-limit sentence names the day the calculation actually used — the
  lowest projected balance over the whole horizon — instead of the end of the
  current month, and says which month when that day lies ahead.
