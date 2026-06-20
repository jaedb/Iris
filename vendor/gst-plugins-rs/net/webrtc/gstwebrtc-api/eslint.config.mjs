import jsdoc from "eslint-plugin-jsdoc";
import globals from "globals";
import path from "node:path";
import { fileURLToPath } from "node:url";
import js from "@eslint/js";
import { FlatCompat } from "@eslint/eslintrc";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const compat = new FlatCompat({
    baseDirectory: __dirname,
    recommendedConfig: js.configs.recommended,
    allConfig: js.configs.all
});

export default [
  ...compat.extends("eslint:recommended"),
  {
    plugins: {
        jsdoc,
    },

    languageOptions: {
        globals: {
            ...globals.browser,
        },

        ecmaVersion: 2018,
        sourceType: "module",
    },

    rules: {
        "getter-return": "error",
        "no-await-in-loop": "error",
        "no-console": "off",
        "no-extra-parens": "off",
        "no-template-curly-in-string": "error",
        "consistent-return": "error",
        curly: "error",
        eqeqeq: "error",
        "no-eval": "error",
        "no-extra-bind": "error",
        "no-invalid-this": "error",
        "no-labels": "error",
        "no-lone-blocks": "error",
        "no-loop-func": "error",
        "no-multi-spaces": "error",
        "no-return-assign": "error",
        "no-return-await": "error",
        "no-self-compare": "error",
        "no-throw-literal": "error",
        "no-unused-expressions": "error",
        "no-useless-call": "error",
        "no-useless-concat": "error",
        "no-useless-return": "error",
        "no-void": "error",
        "no-shadow": "error",
        "block-spacing": "error",

        "brace-style": ["error", "1tbs", {
            allowSingleLine: true,
        }],

        camelcase: "error",
        "comma-dangle": "error",
        "eol-last": "error",
        indent: ["error", 2],
        "linebreak-style": "error",
        "new-parens": "error",
        "no-lonely-if": "error",
        "no-multiple-empty-lines": "error",
        "no-trailing-spaces": "error",
        quotes: "error",
        semi: "error",
        "jsdoc/no-undefined-types": "error",
    },
  },
];
