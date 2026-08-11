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
- **Conflito nunca funde**: quando os dois aparelhos avançaram a partir da mesma base, o dono
  escolhe o vencedor manualmente, vendo a lista de gestos do lado perdedor (lida do `sync_log`).
- O transporte fica atrás do `appDataFolder` do Drive — invisível ao dono, escopo OAuth estreito
  (`drive.appdata`, não acesso aos arquivos do Google Drive do usuário).

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
