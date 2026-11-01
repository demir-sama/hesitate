import { dirname } from "path";
import { fileURLToPath } from "url";
import { FlatCompat } from "@eslint/eslintrc";
const compat = new FlatCompat({ baseDirectory: dirname(fileURLToPath(import.meta.url)) });
export default [
  ...compat.extends("next/core-web-vitals", "next/typescript"),
  { rules: { "@next/next/no-img-element": "off", "react/no-unescaped-entities": "off", "@typescript-eslint/no-explicit-any": "off" } },
  { ignores: ["node_modules/**", ".next/**", "scripts/**"] },
];
