import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { ApprovalDiffCard } from "./ApprovalDiffCard";

describe("ApprovalDiffCard", () => {
  const changes = [{ field: "Saída", before: "R$ 100,00", after: "R$ 120,00" }];

  it("mostra origem (aba/range) e os campos antes→depois", () => {
    render(<ApprovalDiffCard sheet="2026" range="C9" changes={changes} />);
    expect(screen.getByText("2026")).toBeInTheDocument();
    expect(screen.getByText("· C9")).toBeInTheDocument();
    expect(screen.getByText("Saída")).toBeInTheDocument();
    expect(screen.getByText("R$ 100,00")).toBeInTheDocument();
    expect(screen.getByText("R$ 120,00")).toBeInTheDocument();
  });

  it.each([
    ["pending", "Precisa de aprovação"],
    ["approved", "Aprovado"],
    ["rejected", "Recusado"],
  ] as const)("status %s → pill %s", (status, label) => {
    render(<ApprovalDiffCard sheet="2026" changes={changes} status={status} />);
    expect(screen.getByText(label)).toBeInTheDocument();
  });

  it("expõe uma região com aria-label de título + status", () => {
    render(
      <ApprovalDiffCard sheet="2026" title="Atualizar fatura" changes={changes} />,
    );
    expect(
      screen.getByRole("region", { name: "Atualizar fatura — Precisa de aprovação" }),
    ).toBeInTheDocument();
  });

  it("renderiza nota e ações quando fornecidas", () => {
    render(
      <ApprovalDiffCard
        sheet="2026"
        changes={changes}
        note={<span>colável só após aprovação</span>}
        actions={<button>Aprovar</button>}
      />,
    );
    expect(screen.getByText("colável só após aprovação")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Aprovar" })).toBeInTheDocument();
  });
});
