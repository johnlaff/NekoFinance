# Spec 042 — Spike Android: o gate sensorial da estratégia mobile

## Problem Statement

O dono usa o app na mesa e o método na rua — e na rua o Neko não existe. As decisões que o método pede no momento da compra (quanto sobra hoje, qual fatura vence, o que a Mia diria) chegam horas depois, na frente do desktop. A estratégia mobile está selada no ADR-0014: Tauri Android primeiro, React Native + UniFFI como fallback declarado. Mas a estratégia tem uma pergunta aberta que nenhuma conversa responde: **o Android System WebView entrega a experiência que o design system exige, num aparelho real?** Sem um veredito verificável, o risco é duplo — desistir do porte por uma semana ruim de atrito, ou insistir nele por custo afundado quando a fundação não sustenta.

## Solution

Um spike: adicionar o target Android ao projeto Tauri existente, produzir um APK de desenvolvimento e executar no aparelho físico de referência (Android recente, tela 120Hz, instalado via ADB) um gate de cinco itens passa/reprova. Cada item admite no máximo **duas tentativas de correção** dentro do spike; reprovação persistente em qualquer item reprova o gate e ativa a rota fallback do ADR-0014. O entregável não é código de produção — é o **veredito documentado item a item, com evidência**, anexado ao diretório desta spec.

Os cinco itens do gate:

1. **Scroll denso sem jank** — rolagem longa em Lançamentos e Calendário com dados reais importados, sem queda de frames perceptível.
2. **Animações do design system fluidas** — transições e reveals do Midnight Purr como no desktop; `prefers-reduced-motion` do sistema respeitado.
3. **Teclado virtual íntegro** — abrir o teclado em telas de entrada sem cobrir o input focado nem espremer o layout para fora dos limites.
4. **Cold start ≤ 3s** — do toque no ícone à tela Hoje interativa.
5. **Import completo no aparelho** — uma planilha real completa importada até o fim pelo caminho de import local, provando parser + sqlx + toolchain nativa em execução real, não só em compilação.

## User Stories

1. Como dono do produto, quero um APK do Neko instalado no meu aparelho via ADB, para julgar a experiência com o polegar em vez de por especulação.
2. Como dono do produto, quero rolar Lançamentos e Calendário carregados com meus dados reais, para sentir o comportamento do WebView no pior caso de densidade.
3. Como dono do produto, quero ver as animações do design system rodando no aparelho, para saber se o acabamento que define o produto sobrevive à plataforma.
4. Como usuário com `reduced-motion` ligado no sistema, quero o app calmo e sem animações, para que a preferência de acessibilidade valha também no celular.
5. Como dono do produto, quero digitar num formulário com o teclado virtual aberto, para verificar que nenhum campo fica coberto e nenhuma tela quebra.
6. Como dono do produto, quero medir o tempo do toque no ícone até Hoje interativa, para saber se o app local-first honra a promessa de abrir rápido.
7. Como dono do produto, quero importar minha planilha completa no aparelho, para provar que o núcleo Rust (parser, banco, migrações) funciona de verdade no Android.
8. Como dono do produto, quero cada item do gate registrado com evidência (gravação de tela ou captura) e veredito, para que a decisão estratégica seja auditável depois.
9. Como mantenedor, quero as combinações de toolchain que funcionaram (NDK, alvos, flags) documentadas, para que o porte de produção não redescubra o caminho.
10. Como mantenedor, quero saber quais capacidades desktop precisaram ser desligadas para compilar, para que a spec do porte já nasça com a lista de adapters necessária.

## Implementation Decisions

- O spike roda sobre a base atual (SQLite local como está); nenhuma decisão de sync é exercida aqui.
- `tauri android init` gera o projeto Android dentro do repositório; o diretório gerado só entra em versionamento se o gate passar.
- Capacidades desktop-específicas (cofre de segredos, notificador standalone, agendador do SO, updater) são desligadas por `cfg(target_os)` no shell — gambiarras são aceitáveis num spike, mas ficam listadas no relatório para virarem adapters na spec do porte.
- OAuth e notificações ficam fora do APK do spike; o item 5 usa o caminho de import de arquivo local existente.
- Assinatura de desenvolvimento (debug keystore); nenhuma decisão de distribuição é tomada aqui.
- Problemas conhecidos de toolchain (símbolos de ponto flutuante 128-bit no linker do NDK com sqlx/SQLite) se resolvem por pinagem de versão de NDK/cargo-ndk ou flag de linker; a combinação vencedora é registrada no relatório.
- O relatório do gate vive junto desta spec, item a item: veredito, evidência, tentativas de correção usadas.
- Reprovação do gate não abre trabalho novo neste spike: encerra com o relatório e a recomendação de ativar o fallback do ADR-0014.

## Testing Decisions

- O gate **é** o teste: cinco critérios externos, verificáveis, com teto de tentativas declarado — nenhum julgamento de gosto.
- TDD dispensado, conforme a constituição permite para protótipos visuais: o spike não entrega lógica de domínio nova.
- A suíte existente (`npm run check`) precisa continuar verde no desktop com o target Android presente — o spike não pode quebrar a plataforma que já funciona.

## Out of Scope

- OAuth móvel, notificações, lembretes, updater, sync entre aparelhos.
- Qualquer polimento de UI específico de Android além do necessário para julgar o gate.
- Play Store, assinatura de produção, distribuição.
- A rota React Native (só se ativa por reprovação, e é outra spec).

## Further Notes

A ordem selada da frente mobile é: este spike → convergência por snapshot no Drive (spec 043) → porte Android v1 (spec 044). O spike vai primeiro porque é a aposta mais barata com maior poder de veto: tudo que ele aprende vale nas duas rotas, e nada do que vem depois faz sentido sem o veredito dele.
