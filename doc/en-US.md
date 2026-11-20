---
name: Lint
category: tools
summary: The linters and formatters of your project in the editor: ESLint, Oxlint, Biome, Stylelint, Ruff, ShellCheck and others as you type, Prettier and Oxfmt to format.
tags: [lint, linter, eslint, oxlint, biome, stylelint, markdownlint, ruff, pylint, flake8, mypy, shellcheck, hadolint, prettier, oxfmt, formatter]
---

# Lint

The **Lint** plugin runs the linters of your project on the files you edit, and shows their problems in the editor (underlined, in the gutter and in **Problems**) next to the ones of the IDE. **Reformat Code** (Ctrl+Alt+L) formats with Prettier or Oxfmt when the project uses them.

## Fixes

On a line with a problem, a bulb shows in the gutter: a click on it, or **Alt+Enter**, lists the fixes of the problems there:

- the fix of the tool itself (ESLint, Ruff, markdownlint, ShellCheck) and its suggestions (ESLint);
- turning the rule off for the line: `// eslint-disable-next-line rule` above it, `# noqa: CODE` at its end (Ruff);
- **Fix all ESLint problems in the file** (and the same for the other tools), when the tool has more than one fix of its own.

Fixes are offered while the text is the one the tool checked: after a change, they come back with the next check, a moment later.

## When a tool runs

A tool runs for a file only where the project asks for it, so that rules nobody chose stay silent:

| Tool | Files | Runs when the project has |
| --- | --- | --- |
| ESLint | `.js` `.jsx` `.ts` `.tsx` `.mjs` `.cjs` `.vue` `.svelte` `.astro` | an ESLint configuration (`eslint.config.*`, `.eslintrc*`, `eslintConfig` in `package.json`) |
| Oxlint | same | `.oxlintrc.json`, or `oxlint` in `node_modules` |
| Biome | JavaScript, TypeScript, JSON, CSS, GraphQL | `biome.json`, or `@biomejs/biome` in `node_modules` |
| Deno | JavaScript, TypeScript | `deno.json` |
| Stylelint | `.css` `.scss` `.sass` `.less` | a Stylelint configuration |
| markdownlint | `.md` | `.markdownlint.*` (markdownlint-cli) |
| yamllint | `.yaml` `.yml` | `.yamllint*`, or yamllint in the virtual environment |
| actionlint | `.github/workflows/*.yml` | actionlint installed |
| Ruff, Pylint, flake8, mypy | `.py` | their configuration (`ruff.toml`, `[tool.ruff]`, `.pylintrc`, `.flake8`, `mypy.ini`…), or the tool in `.venv`, `venv` or `env` |
| SQLFluff | `.sql` | `.sqlfluff` |
| ShellCheck | `.sh` `.bash` `.ksh` `.bats` | ShellCheck installed |
| Hadolint | `Dockerfile`, `Containerfile` | Hadolint installed |
| RuboCop | Ruby | `.rubocop.yml` |
| PHP_CodeSniffer, PHPStan | `.php` | `phpcs.xml`, `phpstan.neon` (programs in `vendor/bin`) |
| golangci-lint | `.go` | `.golangci.*` |
| Luacheck, ktlint, SwiftLint, HLint, TFLint | Lua, Kotlin, Swift, Haskell, Terraform | the tool installed |
| Dart analyzer | `.dart` | `analysis_options.yaml` or `pubspec.yaml` |
| clang-tidy | C, C++, Objective-C | `.clang-tidy` |

Programs are looked for in the project first (`node_modules/.bin`, the virtual environment, `vendor/bin`), then on the `PATH`.

Most tools read the text of the editor and check it as you type. Oxlint, Biome, Deno, mypy, PHPStan, golangci-lint, Dart, clang-tidy and TFLint read files only: they check the file once it is saved, and their problems stay until the next save.

## Formatting

**Reformat Code** uses the formatter of the project, for the files it knows:

- **Oxfmt**: `.oxfmtrc.json`, or `oxfmt` in `node_modules`. JavaScript, TypeScript, JSON, CSS, HTML, Vue, Markdown, YAML, GraphQL, TOML.
- **Prettier**: a Prettier configuration (`.prettierrc*`, `prettier.config.*`, `prettier` in `package.json`), or `prettier` in `node_modules`.

When both are there, the one the project configures wins; Oxfmt first when both are configured. Other files keep the formatter of the IDE.

## The Lint window

**Lint** (in the right stripe, or **Lint: show the linters and formatters** in the palette) lists the tools of the active file: whether each one runs, with which configuration, how many problems it found and how long it took, or why it failed. A configured tool whose program is missing says so. A check box turns a tool off for every project.
