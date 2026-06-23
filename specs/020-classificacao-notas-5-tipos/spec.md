# Spec 020 — Classificação de notas por seção: 5 tipos + patrimônio

> Fonte: decisão explícita do dono em 2026-06-22, sintetizada nos planos do pacote K.
> Esta spec é intencionalmente neutra: não contém nomes, saldos, transcrições, dados de planilha
> ou material privado.

## Decisão do dono

O dono autorizou reabrir o modelo financeiro que estava travado em `specs/011-engine-five-types`
e nos planos 051/052. Essa decisão supersede o enquadramento anterior em que economia continuava
sendo tratada como Saída comum para custo de vida.

O objetivo não é mudar o saldo de caixa nem a Performance por si só. O dinheiro continua saindo da
conta. A mudança é tornar os buckets do método explícitos para que custo de vida, Economia%,
Cartão e Patrimônio sejam calculados e exibidos sem misturar conceitos.

## Modelo canônico

O app deve alinhar o método a estes tipos de movimento/bucket:

- `Entrada`: dinheiro que entra.
- `Saida`: saída fixa ou genérica.
- `Diario`: gasto variável diário.
- `Cartao`: gastos de cartão, dentro do custo de vida, mas visíveis como bucket próprio.
- `Economia`: dinheiro separado para economia acessível; sai do saldo, mas fica fora do custo de
  vida e alimenta `Economia% = economia / entradas`.
- `Patrimonio`: investimento de longo prazo ou ilíquido; fica fora do custo de vida e fora da
  Economia% acessível.

`Ajuste` é uma classificação operacional para linhas de reconciliação/diferença. Ele não é um dos
cinco buckets principais de leitura financeira.

## Fórmulas

`Custo de vida = Saida + Diario + Cartao + previsao_de_diario`.

Economia e Patrimônio ficam fora de custo de vida. Economia alimenta a taxa de economia acessível.
Patrimônio é acompanhado separadamente por ser ilíquido ou de horizonte longo.

Invariante de regressão para o plano 060: saldo e Performance não devem mudar apenas porque os
buckets ficaram mais explícitos. A reclassificação muda decomposição, custo de vida, Economia% e
superfícies de UI, não deve fabricar ou apagar dinheiro.

## Classificação de itens de nota

A classificação inicial de `line_item` é determinística, pura e baseada somente no cabeçalho de
seção imediatamente anterior ao item. A descrição do item não participa da classificação.

| Seção normalizada       | ItemKind     |
| ----------------------- | ------------ |
| `contas`                | `Saida`      |
| `outros`                | `Saida`      |
| `diario`                | `Diario`     |
| `cartao`, `cartoes`     | `Cartao`     |
| `fatura`, `faturas`     | `Cartao`     |
| `investimento`          | `Patrimonio` |
| `economia`              | `Economia`   |
| `ajuste`, `ajustes`     | `Ajuste`     |
| ausente ou desconhecida | `Saida`      |

Normalização: aparar espaços, remover `:` final, dobrar acentos ASCII relevantes e comparar em
minúsculas.

## Sem fallback por banco

Não há fallback por banco, bandeira, emissor ou palavras na descrição. Esse caminho foi descartado
pelo dono por ser propenso a falsos positivos: palavras de banco podem aparecer em itens que não
são cartão, e nomes comerciais podem colidir com descrições normais.

Se a seção está ausente ou não é reconhecida, o item é `Saida`. Para adicionar um novo padrão, a
fonte correta é estender o vocabulário de seções, não adivinhar pela descrição.

## Divergência item-total

Quando a soma dos itens não bater com o total da célula, o comportamento do produto é avisar e
confiar no total da célula. A classificação nunca deve fabricar valor para reconciliar diferença.

## Downstream

- Plano 060: aplicar os buckets ao engine e às métricas.
- Plano 061: mostrar badges/superfícies de Cartão, Economia e Patrimônio.
- Plano 062: preencher a aba Economia por write-back aprovado a partir da economia derivada.

## DoD

- `ItemKind` e `classify_line_item` existem como núcleo puro, sem I/O.
- Testes cobrem seções com variação de acento/caixa, Economia, Patrimônio, Ajuste, desconhecido e
  ausência de fallback por descrição/banco.
- `npm run privacy:scan` verde.
