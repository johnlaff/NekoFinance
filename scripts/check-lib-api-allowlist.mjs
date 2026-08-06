#!/usr/bin/env node
// Anti-rot check for the `lib/api` funnel gate allowlist (docs/adr/0006-lib-api-funnel-gate.md).
// Fails when an entry no longer imports lib/api (dead entry — remove it) or when the list has
// grown past its recorded ceiling (a new direct import slipped in instead of going through a view).

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import {
  LIB_API_ALLOWLIST,
  LIB_API_ALLOWLIST_CEILING,
} from "../eslint.lib-api-allowlist.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// Mirrors the `regex` pattern in eslint.config.js's no-restricted-imports rule.
const LIB_API_IMPORT_RE = /from\s+["'](?:\.\/|(?:\.\.\/)+)(?:lib\/)?api["']/;

let hasError = false;

if (LIB_API_ALLOWLIST.length > LIB_API_ALLOWLIST_CEILING) {
  console.error(
    `lib/api allowlist cresceu: ${LIB_API_ALLOWLIST.length} entradas, teto era ${LIB_API_ALLOWLIST_CEILING}. ` +
      "Um import novo de lib/api deve passar pela *View.ts da tela, não entrar na allowlist.",
  );
  hasError = true;
}

const deadEntries = [];
for (const relativePath of LIB_API_ALLOWLIST) {
  let content;
  try {
    content = readFileSync(join(root, relativePath), "utf8");
  } catch {
    deadEntries.push(`${relativePath} (arquivo não existe mais)`);
    continue;
  }
  if (!LIB_API_IMPORT_RE.test(content)) {
    deadEntries.push(relativePath);
  }
}

if (deadEntries.length > 0) {
  console.error(
    "Entradas mortas na allowlist de lib/api (não importam mais lib/api — remova-as):\n" +
      deadEntries.map((entry) => `  - ${entry}`).join("\n"),
  );
  hasError = true;
}

if (hasError) {
  process.exit(1);
}

console.log(
  `lib/api allowlist ok (${LIB_API_ALLOWLIST.length}/${LIB_API_ALLOWLIST_CEILING})`,
);
