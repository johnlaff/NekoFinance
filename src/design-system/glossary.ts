/**
 * Glossário canônico PT-BR dos termos do método (teste dos 12 anos) — dado, não componente:
 * mora fora do arquivo do `InfoPopover` para o Fast Refresh preservar estado, e porque a
 * assistente e as telas leem o MESMO vocabulário (um termo, uma redação).
 */
export interface GlossaryEntry {
  title?: string;
  body: string;
}

export const GLOSSARY: Record<string, GlossaryEntry> = {
  pode_gastar: {
    title: "Pode gastar hoje",
    body: "O quanto dá para gastar hoje sem furar o mês. É o menor de dois limites: o que o caixa aguenta e o que respeita sua meta de poupança.",
  },
  piso_caixa: {
    title: "Limite do caixa",
    body: "O máximo por dia que mantém nenhum dia do mês no vermelho, olhando o saldo projetado.",
  },
  folga_poupanca: {
    title: "Limite da poupança",
    body: "O máximo por dia que ainda deixa você guardar a meta do ano (20% a 30% da renda).",
  },
  reserva: {
    title: "Reserva",
    body: "Quantos meses de custo de vida você cobre com o que tem guardado. A meta mínima é 6 meses; a partir de 12 é a 'paz' financeira.",
  },
  caixa: {
    title: "Caixa",
    body: "É dinheiro de passagem, não a sua riqueza. O que está na conta hoje, antes das contas do mês.",
  },
  previsibilidade: {
    title: "Previsibilidade",
    body: "O quanto do gasto típico de cada mês futuro já está lançado. Futuro vazio engana a previsão.",
  },
  colchao: {
    title: "Colchão",
    body: "O que sobra e você guarda para cobrir meses negativos sem sacar investimento. Adaptação válida do método.",
  },
  performance: {
    title: "Performance",
    body: "A foto do mês: Entradas menos tudo que sai — custo de vida (fixas, diário e cartão), Economia, Patrimônio e, no mês em andamento, a previsão do diário que ainda vai ser gasto. É o mesmo cálculo da sua planilha.",
  },
  economizado: {
    title: "Economizado",
    body: "Quanto da renda você guardou como Economia. A meta do método é de 20% a 30% no ano.",
  },
  custo_de_vida: {
    title: "Custo de vida",
    body: "Saídas fixas, diário e cartão somados. O que custa manter sua vida no mês.",
  },
  diario_medio: {
    title: "Diário médio",
    body: "A média do gasto variável por dia até hoje. Ajuda a saber se o ritmo do mês está saudável.",
  },
  cartao: {
    title: "Cartão",
    body: "Compras no cartão viram fatura no vencimento. Gastar hoje no crédito afunda os meses à frente.",
  },
  buraco_do_futuro: {
    title: "O buraco do futuro",
    body: "É o menor ponto da estrada quando ele cruza o zero. Não é sentença: dá para atravessar — antecipar uma entrada, adiar uma saída que caiba, ou cruzar com a reserva por partes, repondo depois. O método manda achar o buraco maior à frente e planejar a travessia com folga.",
  },
  termometro: {
    title: "O termômetro do saldo",
    body: "As cores do saldo como leitura de saúde, em faixas fixas em reais: acima de R$ 2.000 é folga, até R$ 2.000 é ok, até R$ 1.000 é atenção, abaixo de zero é vermelho. A régua é absoluta — não muda com o tamanho da sua vida.",
  },
  diario: {
    title: "Diário",
    body: "A verba variável do dia a dia — mercado, transporte, lazer. O futuro é projetado pela média e o dia é registrado com o real depois que ele passa. Não é percentual da renda: é o seu gasto medido.",
  },
};
