# ADR-0015: Convergência entre aparelhos por snapshot no Drive, com lease

O SQLite local (ADR-0003) é o registro rico: splits, tags, vínculos de reembolso, faturas
persistidas, tetos aceitos, cenários. A planilha só carrega a projeção colapsada — nada disso
sobrevive a uma viagem de ida e volta por ela. Com um segundo aparelho no horizonte (porte
Android, ADR-0014), o enriquecimento feito num aparelho precisa aparecer no outro.

Três rotas foram avaliadas: merge por linha (CRDT ou three-way merge sobre o schema relacional),
um motor de sync de terceiros, e snapshot íntegro do banco com uma regra de posse (lease).

## Decision

**Snapshot íntegro com lease** — o modelo de aparelho-único-por-vez consagrado por gerações de
apps locais-first: o banco inteiro viaja como arquivo (`VACUUM INTO`, o mesmo caminho do backup
existente); um manifest de sequência monotônica por aparelho decide quem pode publicar.

- **Árbitro puro no core**: uma função (`snapshot::lease::decide`) recebe a sequência local, a
  sequência base (última sincronizada) e o manifest remoto, e devolve um veredito fechado — em
  dia, puxar, subir, conflito. Shell e UI só obedecem; nenhuma regra de posse mora fora dela.
- **Force-with-lease**: subir é recusado sempre que o remoto avançou desde a base local — a
  mesma semântica do `git push --force-with-lease`. Ninguém sobrescreve o outro em silêncio.
- **Remoto ausente com base > 0, ou regredido abaixo da base, republica**: nada lá é mais novo
  para disputar — o veredito é `Push`, mesmo sem mudança local nova. É o único jeito de
  restaurar o que a lixeira do Drive ("Excluir dados ocultos do app") ou uma reversão manual
  apagou. Instalação nova sem base local nem manifest remoto (`0, 0, None`) não cai aqui — é
  `UpToDate`, o único jeito de nada ter para subir nem puxar.
- **Conflito nunca funde**: quando os dois aparelhos avançaram a partir da mesma base, o dono
  escolhe o vencedor manualmente, vendo a lista de gestos de CADA lado (lida do `sync_log` de cada
  aparelho) — a escolha é simétrica, então o que se perderia nos dois sentidos fica visível antes
  de escolher, nunca só o lado que a tela presume perdedor. O `sync_log` hoje só registra
  import/write-back da planilha; a lista reflete exatamente o que está gravado ali, sem
  instrumentar gestos de domínio (splits, tags, teto, faturas) — expandir essa cobertura é decisão
  própria, fora do escopo deste mecanismo. O corte "desde a base em comum" é por SEQUÊNCIA
  (`sync_log.seq`, um contador monotônico gravado por trigger em cada linha — nunca o rowid
  implícito do SQLite, que `VACUUM INTO` pode renumerar), não por timestamp: os dois aparelhos
  eram bytes idênticos no momento do último sync, então `MAX(seq)` daquele instante
  (`snapshot_state.base_sync_log_seq`) tem o MESMO significado nos dois lados sem depender de qual
  relógio está certo — um relógio remoto atrasado nunca esconde um gesto de fato posterior à base.
- O transporte fica atrás do `appDataFolder` do Drive — invisível ao dono, escopo OAuth estreito
  (`drive.appdata`, não acesso aos arquivos do Google Drive do usuário).
- **Manifest com o próprio `device_id` só é "eu mesmo" dentro de uma janela estreita**: o
  check-out trata um manifest remoto com o `device_id` deste aparelho como o check-in que morreu
  entre o upload confirmado e a gravação do estado local — e só avança a base sem baixar nem
  trocar arquivo — quando `remote.sequence` bate com `snapshot_state.pending_publish_sequence`, a
  sequência PRETENDIDA gravada antes do upload (nunca a aritmética `base_local + 1`, que só cobria
  o check-in normal — a resolução de conflito mantendo este aparelho publica além disso).
  Qualquer outra sequência com o mesmo id passa pelo veredito normal do árbitro (restauração
  visível). A identidade pode colidir por um caminho fora do lease (cópia manual da pasta do app,
  backup local restaurado à mão sem passar pelo strip do export) — nesse caso as duas instalações
  são, de fato, aparelhos distintos que só compartilham o rótulo, e adotar qualquer sequência à
  frente sem restaurar apagaria essa distinção em silêncio.
- **Convergência é total, não por assunto**: o snapshot é o banco inteiro, então nada nele
  diverge entre aparelhos — inclusive o histórico de conversas da Mia, que mora na mesma tabela
  SQLite que o resto do registro. As únicas exceções são as duas fronteiras estruturais do
  princípio: credenciais (keyring/OAuth), que são identidade do aparelho, e estado derivado
  localmente reconstruível (índice LanceDB), que se reconstrói a partir do que veio.

## Considered alternatives

- **Merge por linha / CRDTs**: rejeitado — os SDKs Rust do espaço (automerge, yrs) ainda pedem
  reestruturar o schema relacional em torno do tipo CRDT escolhido, e nenhum se encaixa no driver
  SQLite atual sem reescrita. O ganho (escrita simultânea real) não existe no caso de uso: o dono
  é uma pessoa em um aparelho de cada vez.
- **Motor de sync de terceiros** (ex. PowerSync, ElectricSQL): rejeitado — impõe um backend
  próprio ou um formato de storage que não é SQLite puro, o que quebraria o `VACUUM INTO` e o
  isolamento local-first que o app já tem.
- **Multi-escritor real via Postgres**: não descartado, adiado — é a evolução natural quando
  escrita simultânea de verdade for necessária, mas troca "local-first, zero servidor" por um
  backend hospedado. Decisão própria, fora do escopo desta spec.

## Why record it here

O lease com aparelho-único-por-vez parece uma limitação até se comparar com o custo de qualquer
alternativa: merge por linha exige reescrever o schema; um motor de terceiros exige um backend.
Um futuro leitor vendo dois aparelhos "brigarem" pela mesma base pode ler isso como um bug a
corrigir com merge automático — não é: é a fronteira que este ADR desenhou de propósito, e a
saída para escrita simultânea de verdade é o Postgres próprio, uma decisão futura e maior, não um
ajuste no lease.
