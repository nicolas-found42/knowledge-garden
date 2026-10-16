import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist/**", "node_modules/**", "src-tauri/**", "docs/**"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.ts", "vite.config.ts"],
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
    },
  },
  {
    files: ["eslint.config.js"],
    languageOptions: { globals: globals.node },
  },
);
