INSERT OR IGNORE INTO category (id, name, parent_id, nature) VALUES
    ('cat_root', 'Sem categoria', NULL, 'variable'),
    ('cat_fixed', 'Fixas', NULL, 'fixed'),
    ('cat_fixed_moradia', 'Moradia', 'cat_fixed', 'fixed'),
    ('cat_fixed_transporte', 'Transporte', 'cat_fixed', 'fixed'),
    ('cat_fixed_saude', 'Saúde', 'cat_fixed', 'fixed'),
    ('cat_variable', 'Variáveis', NULL, 'variable'),
    ('cat_var_alimentacao', 'Alimentação', 'cat_variable', 'variable'),
    ('cat_var_lazer', 'Lazer', 'cat_variable', 'variable'),
    ('cat_var_vestuario', 'Vestuário', 'cat_variable', 'variable'),
    ('cat_cartoes', 'Cartões', NULL, 'variable'),
    ('cat_cartoes_adicional', 'Cartão Adicional', 'cat_cartoes', 'variable');
