# Spec 044 — Porte Android v1: o Neko no bolso, com paridade

## Problem Statement

Com o gate do spike aprovado (spec 042) e a convergência por snapshot entregue (spec 043), falta o produto: o Neko instalado no aparelho Android do dono, com as mesmas capacidades do desktop. Hoje o método vive na mesa; a compra, a dúvida e a conversa com a Mia acontecem na rua. Cada capacidade que o celular não tiver é uma volta forçada ao desktop — e o porte só cumpre a promessa se leitura, escrita, planilha e copiloto vierem juntos.

## Solution

O target Android do mesmo projeto Tauri vira produto: as onze telas e a Mia, lendo e escrevendo no espelho local do aparelho, convergindo com o desktop pelo snapshot no Drive, importando e devolvendo à planilha com o mesmo pipeline gated — com OAuth por deep link, segredos no cofre do sistema, ergonomia de polegar e instalação/atualização por ADB dentro da malha privada do dono.

## User Stories

1. Como dono, quero abrir o Neko no celular e ver Hoje com meus dados reais, para decidir o gasto do dia no momento em que ele acontece.
2. Como dono, quero navegar as onze telas com ergonomia de polegar, para que o celular seja um lugar de primeira classe, não uma versão apertada do desktop.
3. Como dono, quero lançar, dividir, etiquetar e aceitar propostas do celular, para que nenhum gesto do método precise esperar a mesa.
4. Como dono, quero conversar com a Mia no celular, para ter o copiloto no momento da compra, não horas depois.
5. Como dono, quero conectar minha conta Google no celular por um fluxo nativo (navegador seguro + retorno automático ao app), para importar e escrever na planilha de onde eu estiver.
6. Como dono, quero importar a planilha no celular com o mesmo resultado determinístico do desktop, para que os dois espelhos nunca divirjam por caminho de código.
7. Como dono, quero aprovar um write-back no celular com o mesmo diff estruturado e dupla confirmação do desktop, para que a segurança da planilha não afrouxe no bolso.
8. Como dono, quero meus segredos (token Google, chave da Mia) no cofre do sistema Android, para que um backup do aparelho ou um app curioso nunca os leia.
9. Como dono, quero que o snapshot do Drive rode no celular com os mesmos gatilhos do desktop, para trocar de aparelho sem pensar em sincronizar.
10. Como dono, quero instalar e atualizar o app por ADB pela minha malha privada, para ter release sem loja e sem expor nada.
11. Como usuário com `reduced-motion`, tema do sistema ou fonte ampliada, quero o app respeitando as preferências do aparelho, para que acessibilidade não seja recurso de desktop.
12. Como dono, quero as áreas seguras da tela (recorte de câmera, barra de gestos) respeitadas, para nenhum dado ou botão nascer atrás do hardware.
13. Como dono, quero que teclado aberto nunca cubra o campo focado, para digitar valores sem luta.
14. Como dono, quero estados honestos quando algo é indisponível no celular (lembretes, updater), para nunca encontrar tela quebrada fingindo funcionar.
15. Como mantenedor, quero o cofre de segredos atrás de um trait com implementação por plataforma, para que domínio e shell nunca saibam qual sistema os guarda.
16. Como mantenedor, quero o redirect do OAuth como estratégia por plataforma (loopback no desktop, deep link no Android) sobre o mesmo PKCE, para um fluxo só com duas portas de retorno.
17. Como mantenedor, quero um script de build Android análogo ao de Windows, para que gerar o APK assinado seja um comando.
18. Como mantenedor, quero o veredito visual das telas em viewport móvel na suíte existente, para que regressão de layout móvel apareça no CI, não no aparelho.

## Implementation Decisions

- **Pré-fatia: trait de cofre de segredos** (o adapter da cláusula 2 do ADR-0014). As chamadas diretas ao keyring nos dois cofres existentes passam pelo trait; desktop implementa com o keyring atual, Android com o Keystore do sistema. `cfg(target_os)` escolhe no shell, nunca no domínio.
- **OAuth por deep link com esquema próprio do app** (o identificador de bundle já registrado), não App Links com domínio verificado — distribuição é sideload, sem site para hospedar verificação. O fluxo abre no Custom Tab, retorna pelo esquema, e o PKCE/token store não mudam. Limitação conhecida do plugin (segundo deep link no mesmo ciclo de vida) tem mitigação documentada na UI de Conexão: reautenticação orienta reiniciar o app enquanto o upstream não corrige.
- O escopo OAuth é o mesmo do desktop (planilha + `drive.appdata` da spec 043) — nenhum escopo novo nasce aqui.
- Lembretes e updater ficam explicitamente fora: o agendador do SO e o plugin de update não existem no Android; as superfícies correspondentes mostram estado honesto de indisponibilidade por plataforma.
- Release: APK assinado com keystore própria versionada fora do repo; instalação e atualização por ADB na malha privada; o processo entra na documentação de release existente.
- O projeto Android gerado entra em versionamento; a combinação de toolchain aprovada no spike (NDK, cargo-ndk, flags) é pinada em script e documentada.
- Ajustes de UI são refinamento da base mobile-first existente (viewport, safe areas, densidade de toque conforme os padrões de UI do repo) — nenhuma tela é redesenhada.
- CSP e serving de assets seguem o padrão do Tauri para Android (asset loader), mantendo a política restritiva atual.

## Testing Decisions

- **TDD nos contratos novos**: o trait do cofre ganha teste de contrato executado contra a implementação desktop (e contra dublê em memória para os fluxos); a estratégia de redirect do OAuth ganha testes de unidade das duas variantes sobre o mesmo PKCE.
- Import, write-back, forecast e Mia não ganham testes novos por plataforma: o mesmo core roda nos dois alvos, e a suíte existente é o contrato — o porte não pode bifurcar comportamento.
- Playwright visual smoke ganha as capturas móveis que faltarem das telas tocadas, no padrão de viewport móvel já existente na suíte; copy se trava com asserção de texto, nunca só screenshot.
- Verificação em aparelho real via ADB fecha cada fatia visual — o gate do spike já provou que só o aparelho revela certas classes de defeito.
- React Doctor segue como gate de PR; violações novas não entram.

## Out of Scope

- Play Store, assinatura de loja, distribuição além do ADB.
- Lembretes/notificações Android (fatia futura própria, com redesenho WorkManager).
- Updater automático no Android.
- iOS.
- Qualquer mudança de arquitetura de dados além do que as specs 042/043 já entregaram.

## Further Notes

Fecha a trinca da frente mobile (042 → 043 → 044). Se o gate da 042 reprovar, esta spec não executa como está — a rota fallback do ADR-0014 gera spec própria, reaproveitando desta o trait do cofre, o desenho do OAuth móvel e tudo da 043.
