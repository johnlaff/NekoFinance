-- Spec 015 (WRONG #4): rebaixa a ÁRVORE GRANULAR de categorias — o "orçamento por categoria"
-- que o método rejeita. A classificação fina passa a ser TAGS (spec 014). Permanecem só as duas
-- MACRO-NATUREZAS (fixo/variável, que o método usa e que já vêm do tipo de movimento) + "Sem
-- categoria". As categorias granulares estavam dormentes (sem UI/comando de orçamento), então a
-- remoção é segura. `category.nature` é preservado.

DELETE FROM category WHERE id IN (
    'cat_fixed_moradia',
    'cat_fixed_transporte',
    'cat_fixed_saude',
    'cat_var_alimentacao',
    'cat_var_lazer',
    'cat_var_vestuario',
    'cat_cartoes',
    'cat_cartoes_adicional'
);
