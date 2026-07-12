---
target: src/screens/scenarios.tsx — LoanSection
total_score: 25
p0_count: 0
p1_count: 2
timestamp: 2026-07-12T15-03-56Z
slug: src-screens-scenarios-tsx
---

Método: avaliações independentes (design review + detector técnico)

## Design Health Score

| #         | Heurística                  |      Nota | Ponto principal                                                            |
| --------- | --------------------------- | --------: | -------------------------------------------------------------------------- |
| 1         | Visibilidade do status      |         2 | A prévia não distinguia erro de resultado calculado.                       |
| 2         | Sistema × mundo real        |         4 | Valor, prazo, juros, parcela, total e custo seguem o modelo financeiro.    |
| 3         | Controle e liberdade        |         2 | A criação é unitária; remover o grupo exige apagar linhas individualmente. |
| 4         | Consistência e padrões      |         3 | Vocabulário e componentes seguem o Midnight Ledger.                        |
| 5         | Prevenção de erros          |         2 | Prazo e taxa são validados; data e erro da prévia tinham lacunas.          |
| 6         | Reconhecimento, não memória |         3 | A prévia mantém os três resultados relevantes visíveis.                    |
| 7         | Flexibilidade e eficiência  |         2 | Não há remoção em lote nem submit por Enter.                               |
| 8         | Estética e minimalismo      |         3 | Estrutura direta, sem ornamento ou informação supérflua.                   |
| 9         | Recuperação de erros        |         2 | Erros preservam dados, mas prévia e data não orientavam a recuperação.     |
| 10        | Ajuda e documentação        |         2 | Não há ajuda contextual para taxa mensal e custo do crédito.               |
| **Total** |                             | **25/40** | **Aceitável**                                                              |

## Anti-Patterns Verdict

**Avaliação de design:** aprovado. A seção usa affordances familiares, componentes existentes e copy direta. A criação atômica reforça precisão e confiança sem introduzir decoração ou padrões estranhos ao produto.

**Varredura determinística:** `detect.mjs` retornou exit code 0 e `[]`: zero achados, regras ou falsos positivos em `src/screens/scenarios.tsx`.

**Evidência visual:** automação de browser com mutação não estava disponível. A avaliação usou source, diff e testes; nenhum overlay foi criado.

## Overall Impression

A troca de múltiplas escritas por um único comando transacional remove o principal risco emocional e técnico do fluxo: um empréstimo não fica parcialmente persistido. A maior oportunidade é garantir que a prévia financeira só apareça e só habilite a confirmação quando houver cálculo válido.

## What's Working

- A UI executa uma única intenção de domínio; o backend grava principal e parcelas dentro da mesma transação.
- Prazo e taxa têm validação local, `aria-invalid` e mensagens próximas ao campo.
- Parcela, total pago e custo do crédito formam uma prévia pequena e suficiente para decidir.

## Priority Issues

### [P1] Falha da prévia podia parecer matemática válida

**Por que importa:** uma rejeição de `price_installment_cmd` fazia a seção mostrar parcela e total zerados, custo negativo e CTA habilitado. Em uma interface financeira, desconhecido não pode ser apresentado como zero.

**Correção:** renderizar loading, erro com retry e resumo somente após sucesso; bloquear confirmação enquanto a prévia estiver indisponível.

**Comando sugerido:** `$impeccable harden`.

### [P1] Empréstimo nasce como unidade, mas não pode ser removido como unidade

**Por que importa:** um grupo com até 480 parcelas exige apagar cada linha individualmente.

**Correção:** modelar “Remover empréstimo” por `groupId`, com confirmação e exclusão atômica.

**Comando sugerido:** `$impeccable shape`.

### [P2] Sucesso não recebe confirmação explícita

**Por que importa:** a lista atualizada fica acima da seção e pode estar fora da viewport; limpar apenas o principal é um sinal indireto.

**Correção:** anunciar a criação em região live e direcionar o usuário ao grupo ou ao impacto no Horizonte.

**Comando sugerido:** `$impeccable polish`.

### [P2] Data vazia só falhava no backend

**Por que importa:** o CTA permanecia disponível e a resposta tendia ao erro genérico, longe da origem.

**Correção:** incluir a data na validade local, marcar o input e mostrar mensagem associada.

**Comando sugerido:** `$impeccable harden`.

## Persona Red Flags

### Alex — usuário avançado

A criação exige uma única ação, mas a reversão não tem operação em lote nem submit por Enter. Para prazos longos, a assimetria entre criar e desfazer é severa.

### Sam — teclado e leitor de tela

Labels e erros de prazo/taxa são associados corretamente. A prévia, porém, não anunciava erro e podia ler `R$ 0,00` como resultado real; a data vazia não possuía `aria-invalid`.

### Usuário solo em ambiente noturno

A atomicidade transmite calma e controle. Números falsos na prévia e ausência de confirmação pós-sucesso quebram a promessa de precisão discreta.

## Minor Observations

- O CTA usa `size="sm"`; o design system documenta 36 px como mínimo denso de desktop.
- A grade de três colunas dentro do side-sheet merece confirmação visual em largura mínima e zoom de 200%.
- A manutenção da lista de linhas individuais continua útil para dados legados e não contradiz a criação atômica.

## Questions to Consider

- Se o empréstimo nasce como uma unidade, por que não pode morrer como uma unidade?
- Uma prévia financeira desconhecida deveria permitir confirmação?
- Após criar, é mais útil revelar as parcelas ou levar o foco ao impacto no Horizonte?
