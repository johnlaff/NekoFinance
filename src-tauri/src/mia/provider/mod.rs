//! O adapter do provedor: o que sai no fio e o que entra de volta.
//!
//! As garantias de privacidade da conversa não são configuração de conta — são asserção sobre o
//! JSON produzido aqui, porque configuração muda sem aviso e sem erro. A montagem da requisição e
//! a leitura do stream são funções puras: nada neste módulo abre conexão, e nenhum teste dele
//! precisa de rede, chave ou saldo.
//!
//! A credencial nunca atravessa este módulo. Quem a carrega é a borda de rede, que a lê do cofre
//! do sistema e a some no cabeçalho — não há caminho daqui até um log, um evento ou o banco.

pub(crate) mod drift;
pub(crate) mod pins;
pub(crate) mod request;
pub(crate) mod stream;

#[cfg(test)]
mod tests;
