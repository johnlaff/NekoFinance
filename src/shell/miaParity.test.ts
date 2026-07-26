/// <reference types="node" />

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { SCREEN_KEYS } from "./AppShell";

// Sem este teste, paridade é promessa: tela nova entraria sem a conversa saber respondê-la.
const MANIFEST_PATH = join(process.cwd(), "docs", "mia-tool-parity.md");

function rows() {
  return readFileSync(MANIFEST_PATH, "utf8")
    .split(/\r?\n/)
    .filter((line) => line.startsWith("|"))
    .map((line) =>
      line
        .split("|")
        .slice(1, -1)
        .map((column) => column.trim()),
    )
    .filter(
      (columns) =>
        columns[0] !== "Tela" && !columns.every((column) => /^:?-{3,}:?$/.test(column)),
    );
}

function key(column: string | undefined) {
  return column?.replace(/^`/, "").replace(/`$/, "") ?? "";
}

describe("Mia parity manifest", () => {
  it("has four filled columns in every row", () => {
    rows().forEach((columns, index) => {
      expect(
        columns,
        `linha ${index + 1} do manifesto precisa ter quatro colunas`,
      ).toHaveLength(4);
      columns.forEach((column, columnIndex) => {
        expect(
          column,
          `coluna ${columnIndex + 1} da linha ${index + 1} do manifesto precisa estar preenchida`,
        ).not.toBe("");
      });
    });
  });

  it("covers every application screen", () => {
    const manifestScreens = new Set(rows().map(([screen]) => key(screen)));

    SCREEN_KEYS.forEach((screen) => {
      expect(
        manifestScreens.has(screen),
        `A tela "${screen}" ficou sem ferramenta; tela nova precisa de uma linha no manifesto.`,
      ).toBe(true);
    });
  });

  it("does not retain dead screens", () => {
    rows().forEach(([screen]) => {
      expect(
        SCREEN_KEYS,
        `O manifesto cita a tela "${key(screen)}", que não existe no app.`,
      ).toContain(key(screen));
    });
  });
});
