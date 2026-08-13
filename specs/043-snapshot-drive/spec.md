# Spec 043 — Convergência entre aparelhos por snapshot no Drive

## Problem Statement

O sistema de registro do Neko é o SQLite local rico (ADR-0003): splits, tags, vínculos de reembolso, faturas persistidas, tetos aceitos, propostas resolvidas, cenários. A planilha carrega só a projeção colapsada — o enriquecimento **não sobrevive** a uma viagem de ida e volta por ela. Com um segundo aparelho no horizonte (porte Android, ADR-0014), isso vira uma dor concreta: um split feito no desktop não existe no celular; um teto aceito no celular não existe no desktop. Os dados derivados da planilha convergem sozinhos (os dois aparelhos importam a mesma planilha com o mesmo pipeline determinístico) — mas os gestos humanos ficam presos no aparelho onde nasceram. O `sync_log` append-only anteviu exatamente este cenário e está à espera de quem o use.

## Solution

Convergência por **snapshot íntegro com lease**, no modelo consagrado por gerações de apps locais-first de aparelho único por vez: o banco inteiro viaja como arquivo, e uma regra de posse impede que um aparelho sobrescreva silenciosamente o trabalho do outro.

- **Check-out ao abrir**: o app consulta o manifest remoto no `appDataFolder` do Drive; se a cópia remota avançou além da base local, baixa e restaura antes de qualquer gesto.
- **Check-in ao fechar e após gesto material**: o app exporta um snapshot atômico (o caminho `VACUUM INTO` do backup existente) e o sobe com um manifest de sequência — **recusando a subida se o remoto avançou desde a sua base** (a mesma semântica do force-with-lease do git).
- **Conflito nunca é silencioso**: no raro caso de dois aparelhos terem editado a partir da mesma base, o app apresenta a escolha do vencedor **com a lista dos gestos que o lado perdedor tinha** (lida do `sync_log`, hoje só import/write-back da planilha — split, tag, reembolso, fatura, teto e cenário ainda não ficam registrados ali), para o dono refazer o que vale a pena sabendo o recorte real do que a lista cobre.
- **Offline pleno**: cada aparelho tem o banco inteiro; sem rede, tudo funciona — o check-in espera a conexão voltar.

O perímetro de confiança não muda: o dado já vive no Google pela planilha; o snapshot vive no `appDataFolder`, invisível e com escopo OAuth estreito.

## User Stories

1. Como dono, quero que um split feito no desktop apareça no celular na próxima abertura, para que o enriquecimento seja um só entre os aparelhos.
2. Como dono, quero que um teto aceito no celular exista no desktop à noite, para que a cerimônia do teto valha onde quer que aconteça.
3. Como dono, quero lançar e etiquetar no celular sem sinal, para que o método funcione no metrô e no avião.
4. Como dono, quero que o app suba minhas mudanças sozinho ao fechar ou após um gesto relevante, para nunca depender de um botão "sincronizar" que eu esqueceria.
5. Como dono, quero que o app baixe a versão mais nova sozinho ao abrir, para nunca editar em cima de base velha sem saber.
6. Como dono, quero ser impedido de subir quando outro aparelho avançou antes de mim, para que nenhum trabalho meu seja sobrescrito em silêncio.
7. Como dono, num conflito, quero ver a lista dos gestos que o lado perdedor tinha antes de escolher quem vence, para decidir informado e refazer o que valer a pena.
8. Como dono, quero que a planilha continue sendo importável em qualquer aparelho durante tudo isso, para que a fonte editada à mão nunca dependa do sync.
9. Como dono, quero conceder o escopo novo do Drive uma única vez num re-consentimento claro, para saber exatamente o que o app pode tocar (a pasta oculta dele, não meus arquivos).
10. Como dono, quero que a restauração de um snapshot seja tudo-ou-nada com salvaguarda local prévia, para que uma queda de rede no meio nunca me deixe com banco corrompido.
11. Como dono, quero que um snapshot de versão de schema mais nova que o meu app recuse restauração com aviso claro, para que um aparelho desatualizado nunca rebaixe dados migrados.
12. Como dono, quero ver na tela de Conexão quando foi o último check-in/check-out e por qual aparelho, para confiar no estado do sync num relance.
13. Como dono, quero que o estado "há mudanças locais ainda não subidas" seja visível e honesto, para saber que preciso de rede antes de trocar de aparelho.
14. Como mantenedor, quero o árbitro do lease como função pura exaustivamente testada, para que a regra de posse nunca dependa de teste manual.
15. Como mantenedor, quero o transporte do Drive atrás da borda HTTP existente, para testar o fluxo inteiro sem rede.

## Implementation Decisions

- **A costura central é um árbitro puro no core**: recebe a sequência local, a sequência base (última sincronizada) e o manifest remoto; devolve um veredito fechado — `em dia`, `puxar`, `subir`, `conflito`. Toda a inteligência do lease mora nessa função; shell e UI apenas obedecem.
- O manifest remoto carrega: identificador do aparelho, sequência monotônica, carimbo de criação, versão do app e versão do schema. Sequência decide posse; as versões decidem compatibilidade de restauração (schema remoto mais novo que o local ⇒ restauração recusada com orientação de atualizar o app).
- A exportação reusa o caminho de backup existente (`VACUUM INTO` com escrita em temporário e renomeio) — snapshot é sempre um arquivo SQLite íntegro, nunca cópia de arquivo vivo com WAL.
- A restauração é o espelho: baixa para temporário, valida integridade, guarda salvaguarda do banco atual, troca por renomeio atômico.
- Transporte pelo cliente HTTP e pelo refresh de token existentes; o escopo `drive.appdata` é adicionado aos escopos do OAuth, o que força re-consentimento único — documentado na UI de Conexão como mudança esperada.
- Gatilhos: check-out (restauração de verdade) só ao abrir; ao ganhar foco roda uma sonda mais leve (mesmo debounce de probe do sync da planilha) que apenas AVISA quando o remoto avançou, sem baixar nem trocar o banco ativo — o pool já está `app.manage()`-do e em uso, então a convergência de verdade só acontece no próximo boot. Check-in ao fechar e após gesto material com debounce — a definição de "gesto material" é a mesma que o `sync_log` já registra.
- O import da planilha também avança a sequência local: importar É mudar o banco; o lease trata igual.
- **Check-out com manifest do próprio `device_id`**: o veredito `Pull` do árbitro para um manifest que carrega o `device_id` deste aparelho só é tratado como "check-in próprio que morreu entre o upload confirmado e a gravação do estado local" quando `remote.sequence == base_local + 1` — a janela exata daquela queda. Só nesse caso a base local avança para alcançar o remoto sem baixar nem trocar arquivo. Qualquer outra sequência com o mesmo `device_id` (duas instalações compartilhando identidade por um caminho lateral — cópia manual da pasta do app, backup local restaurado à mão sem passar pelo strip do export) segue o veredito normal do árbitro: restauração de verdade, registrada na linha "Última leitura do Drive" e com a salvaguarda local de sempre. A largura ampla foi avaliada e descartada por adotar silenciosamente o conteúdo de uma instalação alheia sempre que a identidade colide.
- Conflito não tenta merge por linha: a resolução é escolher o vencedor, com a lista de gestos do perdedor (extraída do `sync_log` entre a base comum e a ponta perdedora) exibida antes da escolha.
- Tudo entra pelo funil de views (ADR-0006/0008): a UI de estado do sync vive na superfície de Conexão existente; a tela de conflito é estado do shell.
- Esta spec entrega o mecanismo completo hospedado no desktop (onde já é exercitável de ponta a ponta, inclusive simulando o segundo aparelho); o Android consome na spec do porte sem mudança de contrato.

## Testing Decisions

- **TDD obrigatório** — sync é categoria listada como tal nas regras do repositório.
- O árbitro puro é testado por tabela de casos: todas as combinações de (local, base, remoto) relevantes, incluindo primeira subida, remoto ausente, empate exato, avanço unilateral de cada lado e divergência dupla.
- O transporte é testado com a borda HTTP mockada, como os testes existentes do Google Sheets fazem — nenhum teste toca a rede.
- Exportação atômica já tem teste (o backup); a restauração ganha o espelho dele: falha no meio nunca deixa o destino em estado parcial.
- Regressão com pool de uma conexão para os caminhos de export/restore com transação aberta — a classe de deadlock já vista no repositório não pode voltar por aqui.
- Bom teste aqui é comportamento externo: dado este trio de estados, o veredito é este; dado este veredito, o efeito observável é este. Nenhum teste inspeciona formato interno de manifest além do contrato.

## Out of Scope

- Merge por linha, CRDTs, motores de sync de terceiros — avaliados e descartados por pesquisa datada (imaturidade dos SDKs Rust e incompatibilidade com o driver atual).
- Escrita simultânea de verdade em dois aparelhos: o modelo é aparelho-único-por-vez com conflito explícito. A evolução declarada para multi-escritor real é um Postgres próprio — decisão futura, com pesquisa própria já arquivada.
- O histórico de conversas da Mia viaja no snapshot como consequência do modelo de banco
  inteiro — mora na mesma tabela SQLite que o resto do registro (migration
  `20260731000001_mia_conversation.sql`), sem filtro por assunto. As únicas exceções à
  convergência total são as duas fronteiras do princípio, não recortes de assunto: credenciais
  (keyring/OAuth) são identidade do aparelho e nunca viajam; estado derivado localmente
  reconstruível (o índice LanceDB da Mia) fica fora do snapshot e se reconstrói a partir do que
  veio, em vez de sincronizar.
- Criptografia do snapshot além do transporte TLS e do isolamento do `appDataFolder`.

## Further Notes

Depende do gate da spec 042 apenas na ordem do plano (o mecanismo em si é agnóstico de plataforma e roda inteiro no desktop). O ADR-0015, registrando esta decisão de convergência e suas alternativas rejeitadas, nasce no primeiro PR desta spec.
