# Lint (`lint`)

A native plugin of [Atelier IDE](https://atelier-ide-org.github.io/docs/): the linters of a project (ESLint, Oxlint, Biome, Stylelint, markdownlint, Ruff, Pylint, flake8, mypy, ShellCheck, Hadolint, RuboCop, golangci-lint and more) check the documents as they are typed, and Prettier or Oxfmt format them. What it does for the user: [`doc/en-US.md`](doc/en-US.md).

It needs the plugin interface 0.5: the documents, the problems and the formatting (`Info::lints`, `Info::formats`).

```sh
cargo test
atelier-cli plugin compile           # target/release/lint.atelier-plugin, for this system
atelier-cli plugin compile --debug   # target/debug/lint.atelier-plugin
```

Install the file in Atelier: **Plugins**, **Install a plugin from a file** (the + button).

- `plugin.json`: its identifier, name, description, library and the version of the plugin interface (`abi`).
- `src/lib.rs`: the plugin (`atelier_plugin_api::Plugin`) and its window, drawn with its own egui.
- `src/catalog.rs`: the tools, the files they read, when they run and how they are called.
- `src/parse.rs`: the readers of their reports; `src/text.rs`: their places, as the IDE counts them.
- `src/linting.rs`: linting in the background, and formatting; `src/run.rs`: finding and running the programs.
- `locales/`, `doc/`: its texts and its documentation page.

The test `real_tools_find_problems_and_format` (`src/linting.rs`) also runs the real tools when `LINT_TEST_JS` names a project with ESLint, Oxlint, Biome, Stylelint, markdownlint, Prettier and Oxfmt in `node_modules`, and `LINT_TEST_PY` one with Ruff, Pylint, flake8, mypy and yamllint in `.venv`.

Publishing: raising the version of `Cargo.toml` on `main` publishes it as the release `plugins/lint/vX.Y.Z` of atelier-ide-org/releases (`.github/workflows/release.yml`), and Atelier lists it in **Plugins › Marketplace**.
