import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";
import { LIB_API_ALLOWLIST } from "./eslint.lib-api-allowlist.mjs";

export default tseslint.config(
  {
    ignores: [
      "coverage/**",
      "dist",
      "**/dist/**",
      "**/playwright-report/**",
      "**/test-results/**",
      "node_modules/**",
      ".ds-sync/**",
      // Só ignora os artefatos gerados/vendorizados do DS; os componentes .tsx à mão são linted.
      "src/design-system/**/*.jsx",
      "src/design-system/**/*.d.ts",
      "src/design-system/ui_kits/**",
      "src/design-system/_ds_bundle.js",
      "src-tauri/**",
      ".agents/**",
      "scripts/**",
      "*.config.js",
      "*.md",
    ],
  },
  js.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    extends: [
      ...tseslint.configs.recommendedTypeChecked,
      ...tseslint.configs.stylisticTypeChecked,
    ],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: {
      "react-hooks": reactHooks,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "no-console": ["warn", { allow: ["warn", "error"] }],
      "@typescript-eslint/consistent-type-imports": [
        "error",
        { fixStyle: "inline-type-imports" },
      ],
      "@typescript-eslint/no-floating-promises": "error",
      "@typescript-eslint/no-misused-promises": "error",
    },
  },
  {
    files: ["src/**/*.test.{ts,tsx}", "src/test/**/*.ts"],
    rules: {
      "@typescript-eslint/no-unsafe-call": "off",
      "@typescript-eslint/no-unsafe-member-access": "off",
    },
  },
  {
    // Funil do `lib/api`: fora das zonas nomeadas, a tela lê o backend só pela sua
    // `*View.ts` (docs/adr/0006-lib-api-funnel-gate.md). Sem `allowTypeImports` — DTO cru
    // não vaza nem como tipo.
    files: ["src/**/*.{ts,tsx}"],
    ignores: [
      "src/screens/*View.ts",
      "src/screens/*View.test.ts",
      "src/screens/miaRuntime.ts",
      "src/screens/miaRuntime.test.ts",
      "src/screens/miaSession.ts",
      "src/screens/miaSession.test.ts",
      "src/hooks/**",
      "src/test/commands.ts",
      ...LIB_API_ALLOWLIST,
    ],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              // Cobre toda profundidade relativa que resolve para src/lib/api.ts: "./api" e
              // "../api" (de dentro de src/lib/ e suas subpastas) e "./lib/api"/"../lib/api"
              // (de qualquer outro ponto de src/**) — sem depender de quantos "../" o caminho
              // carrega.
              regex: "^(\\./|(?:\\.\\./)+)(lib/)?api$",
              message:
                "Não importe lib/api diretamente — leia pela *View.ts da tela (ela é a porta do shim). " +
                "Exceções: *View.ts/*View.test.ts, runtime da Mia (miaRuntime/miaSession + testes), " +
                "src/hooks/**, e src/test/commands.ts (infra de mock do IPC).",
            },
          ],
        },
      ],
    },
  },
);
