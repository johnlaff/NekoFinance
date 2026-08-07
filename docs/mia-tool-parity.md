# Paridade superfície → ferramenta

Toda superfície que a interface mostra tem uma ferramenta da conversa que a alcança. Sem esta
tabela a paridade seria promessa: uma tela nova entraria, e a conversa passaria a não saber
responder sobre o que a pessoa está vendo — sem nada quebrar.

A tabela é exercitada dos dois lados, e é por isso que ela envelhece bem:

- **Da interface** (`src/shell/miaParity.test.ts`): toda tela do app aparece aqui. Tela nova sem
  linha reprova a suíte.
- **Da fachada** (`src-tauri/src/mia/tests.rs`): toda ferramenta citada existe no catálogo, e toda
  ferramenta do catálogo aparece aqui. Ferramenta renomeada, removida ou órfã reprova a suíte.

A coluna **Tela** usa a chave da navegação (o tipo `Screen` em `src/shell/screens.ts`); a coluna
**Ferramenta** usa o nome exato do catálogo (`src-tauri/src/mia/catalog.rs`).

| Tela          | Superfície                  | O que ela publica                                                       | Ferramenta                   |
| ------------- | --------------------------- | ----------------------------------------------------------------------- | ---------------------------- |
| `hoje`        | Retrato do dia              | Teto e gasto do dia, saldo projetado do mês, reserva, modo de gasto     | `get_financial_snapshot`     |
| `hoje`        | Quanto ainda dá para gastar | Safe-to-spend do dia e qual régua está limitando                        | `get_forecast`               |
| `lancamentos` | Livro-razão                 | Célula × nota, filtros de recorte, tags, titulares e totais do filtro   | `search_transactions`        |
| `lancamentos` | Compromissos nomeados       | Obrigações, séries e o mês típico de cada uma                           | `get_commitments`            |
| `mes`         | O mês nas contas do método  | Entradas, os cinco baldes, custo de vida, performance e Economizado%    | `get_month_analysis`         |
| `cartoes`     | Faturas e ciclos            | Fatura em aberto, vencimento, parcelas e reembolsos vinculados          | `get_commitments`            |
| `cartoes`     | Próximo vencimento          | Fatura de cada cartão com valor e status                                | `get_financial_snapshot`     |
| `ano`         | A régua anual               | Economizado% contra a faixa, meses sem lastro e o recorte que sustenta  | `get_year_analysis`          |
| `calendario`  | Grade de saldo              | Movimento e saldo dia a dia, com o menor saldo do recorte               | `get_cashflow_calendar`      |
| `horizonte`   | Radar do caixa              | Saldo projetado à frente, fundo do poço e cobertura dos meses futuros   | `get_forecast`               |
| `horizonte`   | Cenários ("e se")           | A hipótese projetada contra o mundo real, com as diferenças prontas     | `simulate_scenario`          |
| `tags`        | Interruptores de régua      | Em quais réguas cada tag conta, o que ela moveu e o preço das exceções  | `get_tags`                   |
| `mia`         | Didática do método          | O capítulo do método por trás do termo que a pessoa perguntou           | `get_method_guidance`        |
| `mia`         | Cartão de proposta          | O lançamento avulso montado pela conversa, à espera do gesto de aprovar | `propose_transaction`        |
| `teto`        | Cerimônia do teto           | Teto vigente, procedência, itens mensais e as metas do método           | `get_budget_settings`        |
| `config`      | Bolsos                      | Onde o dinheiro está por classe de liquidez, e o patrimônio somado      | `get_accounts_and_net_worth` |
| `config`      | Conexão e estado dos dados  | Cobertura, última importação, pendências e o que falta preencher        | `get_data_status`            |
