//! A casca do binário `mia-bench`. Toda a bancada vive no lib, onde a suíte a exercita; aqui só
//! se entrega o controle.

fn main() -> std::process::ExitCode {
    neko_finance_lib::mia_bench::main()
}
