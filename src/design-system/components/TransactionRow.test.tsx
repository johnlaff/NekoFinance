import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import { TransactionRow } from "./TransactionRow";

describe("TransactionRow", () => {
  it("renderiza data, descrição e procedência", () => {
    render(
      <TransactionRow
        date="13/06"
        desc="Mercado"
        amount={-12000}
        provenance="importado"
      />,
    );
    expect(screen.getByText("13/06")).toBeInTheDocument();
    expect(screen.getByText("Mercado")).toBeInTheDocument();
    expect(screen.getByText("Da planilha")).toBeInTheDocument();
  });

  it("mostra a nota entre aspas e o badge de repasse", () => {
    render(
      <TransactionRow
        date="01/06"
        desc="Despesa fixa demo"
        amount={-200000}
        note="ref. junho"
        passthrough
      />,
    );
    expect(screen.getByText("“ref. junho”")).toBeInTheDocument();
    expect(screen.getByText("repasse")).toBeInTheDocument();
  });

  it("sem lump não tem botão de expandir", () => {
    render(<TransactionRow date="01/06" desc="Receita demo" amount={500000} />);
    expect(
      screen.queryByRole("button", { name: /Abrir itens|Fechar itens/ }),
    ).not.toBeInTheDocument();
  });

  it("expande o lump da fatura mostrando os itens da nota (preservação)", async () => {
    render(
      <TransactionRow
        date="01/06"
        desc="Fatura cartão"
        amount={-150000}
        lump={[
          { what: "Mercado", amount: -90000 },
          { what: "Farmácia", amount: -60000 },
        ]}
      />,
    );
    // Fechado: itens não visíveis.
    expect(screen.queryByText("Mercado")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Abrir itens" }));
    expect(screen.getByText("Mercado")).toBeInTheDocument();
    expect(screen.getByText("Farmácia")).toBeInTheDocument();
    // A nota explicativa da preservação das notas.
    expect(screen.getByText(/Cada item é preservado/)).toBeInTheDocument();
  });

  it("permite ativar a linha clicável pelo teclado", async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    render(
      <TransactionRow date="13/06" desc="Mercado" amount={-12000} onClick={onClick} />,
    );

    await user.tab();
    await user.keyboard("{Enter}");

    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
