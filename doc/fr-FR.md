---
name: Lint
category: tools
summary: Les linters et formateurs de votre projet dans l'éditeur : ESLint, Oxlint, Biome, Stylelint, Ruff, ShellCheck et d'autres à la frappe, Prettier et Oxfmt pour formater.
tags: [lint, linter, eslint, oxlint, biome, stylelint, markdownlint, ruff, pylint, flake8, mypy, shellcheck, hadolint, prettier, oxfmt, formateur]
---

# Lint

Le plugin **Lint** fait tourner les linters de votre projet sur les fichiers que vous modifiez, et montre leurs problèmes dans l'éditeur (soulignés, dans la gouttière et dans **Problèmes**) avec ceux de l'IDE. **Reformater le code** (Ctrl+Alt+L) formate avec Prettier ou Oxfmt quand le projet les utilise.

## Corrections

Sur une ligne qui a un problème, une ampoule apparaît dans la gouttière : un clic dessus, ou **Alt+Entrée**, liste les corrections des problèmes de la ligne :

- la correction de l'outil lui-même (ESLint, Ruff, markdownlint, ShellCheck) et ses suggestions (ESLint) ;
- la désactivation de la règle pour la ligne : `// eslint-disable-next-line règle` au-dessus, `# noqa: CODE` à la fin (Ruff) ;
- **Corriger tous les problèmes eslint du fichier** (et de même pour les autres outils), quand l'outil a plus d'une correction à lui.

Les corrections sont proposées tant que le texte est celui que l'outil a vérifié : après une modification, elles reviennent avec la vérification suivante, un instant plus tard.

## Quand un outil tourne

Un outil ne tourne sur un fichier que là où le projet le demande, pour que des règles que personne n'a choisies restent muettes :

| Outil | Fichiers | Tourne quand le projet a |
| --- | --- | --- |
| ESLint | `.js` `.jsx` `.ts` `.tsx` `.mjs` `.cjs` `.vue` `.svelte` `.astro` | une configuration ESLint (`eslint.config.*`, `.eslintrc*`, `eslintConfig` dans `package.json`) |
| Oxlint | les mêmes | `.oxlintrc.json`, ou `oxlint` dans `node_modules` |
| Biome | JavaScript, TypeScript, JSON, CSS, GraphQL | `biome.json`, ou `@biomejs/biome` dans `node_modules` |
| Deno | JavaScript, TypeScript | `deno.json` |
| Stylelint | `.css` `.scss` `.sass` `.less` | une configuration Stylelint |
| markdownlint | `.md` | `.markdownlint.*` (markdownlint-cli) |
| yamllint | `.yaml` `.yml` | `.yamllint*`, ou yamllint dans l'environnement virtuel |
| actionlint | `.github/workflows/*.yml` | actionlint installé |
| Ruff, Pylint, flake8, mypy | `.py` | leur configuration (`ruff.toml`, `[tool.ruff]`, `.pylintrc`, `.flake8`, `mypy.ini`…), ou l'outil dans `.venv`, `venv` ou `env` |
| SQLFluff | `.sql` | `.sqlfluff` |
| ShellCheck | `.sh` `.bash` `.ksh` `.bats` | ShellCheck installé |
| Hadolint | `Dockerfile`, `Containerfile` | Hadolint installé |
| RuboCop | Ruby | `.rubocop.yml` |
| PHP_CodeSniffer, PHPStan | `.php` | `phpcs.xml`, `phpstan.neon` (programmes dans `vendor/bin`) |
| golangci-lint | `.go` | `.golangci.*` |
| Luacheck, ktlint, SwiftLint, HLint, TFLint | Lua, Kotlin, Swift, Haskell, Terraform | l'outil installé |
| Analyseur Dart | `.dart` | `analysis_options.yaml` ou `pubspec.yaml` |
| clang-tidy | C, C++, Objective-C | `.clang-tidy` |

Les programmes sont cherchés d'abord dans le projet (`node_modules/.bin`, l'environnement virtuel, `vendor/bin`), puis dans le `PATH`.

La plupart des outils lisent le texte de l'éditeur et le vérifient à la frappe. Oxlint, Biome, Deno, mypy, PHPStan, golangci-lint, Dart, clang-tidy et TFLint ne lisent que des fichiers : ils vérifient le fichier une fois enregistré, et leurs problèmes restent jusqu'à l'enregistrement suivant.

## Formatage

**Reformater le code** utilise le formateur du projet, pour les fichiers qu'il connaît :

- **Oxfmt** : `.oxfmtrc.json`, ou `oxfmt` dans `node_modules`. JavaScript, TypeScript, JSON, CSS, HTML, Vue, Markdown, YAML, GraphQL, TOML.
- **Prettier** : une configuration Prettier (`.prettierrc*`, `prettier.config.*`, `prettier` dans `package.json`), ou `prettier` dans `node_modules`.

Quand les deux sont là, celui que le projet configure l'emporte ; Oxfmt d'abord quand les deux sont configurés. Les autres fichiers gardent le formateur de l'IDE.

## La fenêtre Lint

**Lint** (dans la barre de droite, ou **Lint : afficher les linters et formateurs** dans la palette) liste les outils du fichier actif : s'il tourne, avec quelle configuration, combien de problèmes il a trouvés et en combien de temps, ou pourquoi il a échoué. Un outil configuré dont le programme manque le dit. Une case désactive un outil pour tous les projets.
