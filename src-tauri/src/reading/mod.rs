//! A leitura financeira do dia: uma carga, uma composição, muitos recortes.
//!
//! O módulo separa o inventário do que a projeção precisa ([`inputs`]) da carga que o preenche
//! ([`load`]). A carga é a única fronteira de SQL da rota de forecast; o que vier depois consome
//! [`ForecastInputs`] por referência e, por não receber pool, não tem como consultar o banco.

// A leitura nasce COMPLETA antes de qualquer consumidor migrar: o inventário e a carga existem e
// estão provados, e as telas seguem nos caminhos antigos até a composição chegar. O `allow` cai
// junto com o primeiro recorte que consumir a leitura.
#![allow(dead_code)]

pub(crate) mod inputs;
pub(crate) mod load;
