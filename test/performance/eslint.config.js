import js from "@eslint/js";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: ["node_modules/", "generated/", ".artifacts/"],
  },
  js.configs.recommended,
  ...tseslint.configs.recommendedTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      // The generated client is asynchronous throughout: an un-awaited call
      // silently drops the request from the flow, and a check that treats the
      // promise as a value always passes.
      "@typescript-eslint/no-floating-promises": "error",
      "@typescript-eslint/no-misused-promises": "error",
    },
  },
  {
    files: ["eslint.config.js", "types/**/*.d.ts"],
    extends: [tseslint.configs.disableTypeChecked],
  },
);
