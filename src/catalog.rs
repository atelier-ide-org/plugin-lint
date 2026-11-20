//! The linters and formatters the plugin knows: the files each one reads, where its program
//! and its configuration are, when it runs for a project, and how to call it.

use crate::{
    parse::{self, Reader},
    run::{self, Found, Place},
};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Lint,
    Format,
}

/// What the program reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// The text of the editor, on its input: it runs at each change.
    Stdin,
    /// The file on disk: it runs when the document is saved.
    File,
}

/// When a tool runs for a file, once its program is found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    /// With a configuration of the project: style rules nobody chose would be noise.
    Config,
    /// With a configuration, or installed in the project (`node_modules`, `.venv`…).
    Installed,
    /// Whenever its program is found: its rules need no configuration.
    Found,
}

/// Where a tool runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cwd {
    /// The folder of its configuration, else the project, else the folder of the file.
    Config,
    /// The folder of the file (tools that read the module of a folder).
    FileDir,
}

pub struct Tool {
    /// For the settings, and the source of its problems.
    pub id: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    /// Lowercase extensions, or whole file names.
    pub files: &'static [&'static str],
    /// A further condition on the path (the workflows of GitHub for actionlint).
    pub only: Option<fn(&Path) -> bool>,
    pub program: &'static str,
    pub place: Place,
    /// Files that hold its configuration.
    pub configs: &'static [&'static str],
    /// Keys of `package.json` that hold it.
    pub package_keys: &'static [&'static str],
    /// Sections of `pyproject.toml` (`[tool.<name>]`), `setup.cfg` and `tox.ini` (`[<name>]`).
    pub sections: &'static [&'static str],
    pub activation: Activation,
    pub input: Input,
    pub cwd: Cwd,
    /// Its arguments for a file.
    pub args: fn(&Path) -> Vec<String>,
    /// How its report is read (linters).
    pub read: Option<Reader>,
}

const JS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];
const JS_FRAMEWORKS: &[&str] = &[
    "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "vue", "svelte", "astro",
];
const PRETTIER_FILES: &[&str] = &[
    "js",
    "jsx",
    "mjs",
    "cjs",
    "ts",
    "tsx",
    "mts",
    "cts",
    "vue",
    "html",
    "htm",
    "css",
    "scss",
    "less",
    "json",
    "json5",
    "jsonc",
    "md",
    "markdown",
    "mdx",
    "yaml",
    "yml",
    "graphql",
    "gql",
    "handlebars",
    "hbs",
];
const OXFMT_FILES: &[&str] = &[
    "js",
    "jsx",
    "mjs",
    "cjs",
    "ts",
    "tsx",
    "mts",
    "cts",
    "vue",
    "html",
    "htm",
    "css",
    "scss",
    "less",
    "json",
    "json5",
    "jsonc",
    "md",
    "markdown",
    "mdx",
    "yaml",
    "yml",
    "graphql",
    "gql",
    "handlebars",
    "hbs",
    "toml",
];

fn s(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|a| a.to_string()).collect()
}

fn with_file(list: &[&str], file: &Path) -> Vec<String> {
    let mut all = args(list);
    all.push(s(file));
    all
}

fn in_workflows(path: &Path) -> bool {
    let path = path.to_string_lossy().replace('\\', "/");
    path.contains("/.github/workflows/")
}

/// Every tool, the formatters first in the order they are preferred.
pub static TOOLS: &[Tool] = &[
    // ---- Formatters ----
    Tool {
        id: "oxfmt",
        name: "Oxfmt",
        kind: Kind::Format,
        files: OXFMT_FILES,
        only: None,
        program: "oxfmt",
        place: Place::Node,
        configs: &[".oxfmtrc.json", ".oxfmtrc.jsonc", "oxfmt.config.ts"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Installed,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| vec![format!("--stdin-filepath={}", s(file))],
        read: None,
    },
    Tool {
        id: "prettier",
        name: "Prettier",
        kind: Kind::Format,
        files: PRETTIER_FILES,
        only: None,
        program: "prettier",
        place: Place::Node,
        configs: &[
            ".prettierrc",
            ".prettierrc.json",
            ".prettierrc.json5",
            ".prettierrc.yaml",
            ".prettierrc.yml",
            ".prettierrc.toml",
            ".prettierrc.js",
            ".prettierrc.cjs",
            ".prettierrc.mjs",
            ".prettierrc.ts",
            ".prettierrc.cts",
            ".prettierrc.mts",
            "prettier.config.js",
            "prettier.config.cjs",
            "prettier.config.mjs",
            "prettier.config.ts",
            "prettier.config.cts",
            "prettier.config.mts",
        ],
        package_keys: &["prettier"],
        sections: &[],
        activation: Activation::Installed,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| vec!["--no-color".into(), "--stdin-filepath".into(), s(file)],
        read: None,
    },
    // ---- JavaScript, TypeScript and their frameworks ----
    Tool {
        id: "eslint",
        name: "ESLint",
        kind: Kind::Lint,
        files: JS_FRAMEWORKS,
        only: None,
        program: "eslint",
        place: Place::Node,
        configs: &[
            "eslint.config.js",
            "eslint.config.mjs",
            "eslint.config.cjs",
            "eslint.config.ts",
            "eslint.config.mts",
            "eslint.config.cts",
            ".eslintrc",
            ".eslintrc.js",
            ".eslintrc.cjs",
            ".eslintrc.json",
            ".eslintrc.yaml",
            ".eslintrc.yml",
        ],
        package_keys: &["eslintConfig"],
        sections: &[],
        activation: Activation::Config,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            vec![
                "--stdin".into(),
                "--stdin-filename".into(),
                s(file),
                "--format".into(),
                "json".into(),
            ]
        },
        read: Some(parse::eslint),
    },
    Tool {
        id: "oxlint",
        name: "Oxlint",
        kind: Kind::Lint,
        files: JS_FRAMEWORKS,
        only: None,
        program: "oxlint",
        place: Place::Node,
        configs: &[".oxlintrc.json", ".oxlintrc.jsonc", "oxlint.config.ts"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Installed,
        input: Input::File,
        cwd: Cwd::Config,
        args: |file| with_file(&["--format", "json"], file),
        read: Some(parse::oxlint),
    },
    Tool {
        id: "biome",
        name: "Biome",
        kind: Kind::Lint,
        files: &[
            "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "json", "jsonc", "css",
            "graphql", "gql",
        ],
        only: None,
        program: "biome",
        place: Place::Node,
        configs: &["biome.json", "biome.jsonc"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Installed,
        input: Input::File,
        cwd: Cwd::Config,
        args: |file| with_file(&["lint", "--reporter=json"], file),
        read: Some(parse::biome),
    },
    Tool {
        id: "deno",
        name: "Deno",
        kind: Kind::Lint,
        files: JS,
        only: None,
        program: "deno",
        place: Place::System,
        configs: &["deno.json", "deno.jsonc"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Config,
        input: Input::File,
        cwd: Cwd::Config,
        args: |file| with_file(&["lint", "--json"], file),
        read: Some(parse::deno),
    },
    // ---- Styles and documents ----
    Tool {
        id: "stylelint",
        name: "Stylelint",
        kind: Kind::Lint,
        files: &["css", "scss", "sass", "less"],
        only: None,
        program: "stylelint",
        place: Place::Node,
        configs: &[
            ".stylelintrc",
            ".stylelintrc.json",
            ".stylelintrc.yaml",
            ".stylelintrc.yml",
            ".stylelintrc.js",
            ".stylelintrc.cjs",
            ".stylelintrc.mjs",
            "stylelint.config.js",
            "stylelint.config.cjs",
            "stylelint.config.mjs",
            "stylelint.config.ts",
        ],
        package_keys: &["stylelint"],
        sections: &[],
        activation: Activation::Config,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            vec![
                "--stdin".into(),
                "--stdin-filename".into(),
                s(file),
                "--formatter".into(),
                "json".into(),
            ]
        },
        read: Some(parse::stylelint),
    },
    Tool {
        id: "markdownlint",
        name: "markdownlint",
        kind: Kind::Lint,
        files: &["md", "markdown"],
        only: None,
        program: "markdownlint",
        place: Place::Node,
        configs: &[
            ".markdownlint.json",
            ".markdownlint.jsonc",
            ".markdownlint.yaml",
            ".markdownlint.yml",
            ".markdownlintrc",
        ],
        package_keys: &[],
        sections: &[],
        activation: Activation::Config,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |_| args(&["--stdin", "--json"]),
        read: Some(parse::markdownlint),
    },
    Tool {
        id: "yamllint",
        name: "yamllint",
        kind: Kind::Lint,
        files: &["yaml", "yml"],
        only: None,
        program: "yamllint",
        place: Place::Python,
        configs: &[".yamllint", ".yamllint.yaml", ".yamllint.yml"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Installed,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |_| args(&["-f", "parsable", "-"]),
        read: Some(parse::yamllint),
    },
    Tool {
        id: "actionlint",
        name: "actionlint",
        kind: Kind::Lint,
        files: &["yaml", "yml"],
        only: Some(in_workflows),
        program: "actionlint",
        place: Place::System,
        configs: &[],
        package_keys: &[],
        sections: &[],
        activation: Activation::Found,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            vec![
                "-format".into(),
                "{{json .}}".into(),
                "-stdin-filename".into(),
                s(file),
                "-".into(),
            ]
        },
        read: Some(parse::actionlint),
    },
    // ---- Python ----
    Tool {
        id: "ruff",
        name: "Ruff",
        kind: Kind::Lint,
        files: &["py", "pyi"],
        only: None,
        program: "ruff",
        place: Place::Python,
        configs: &["ruff.toml", ".ruff.toml"],
        package_keys: &[],
        sections: &["ruff"],
        activation: Activation::Installed,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            vec![
                "check".into(),
                "--no-fix".into(),
                "--output-format".into(),
                "json".into(),
                "--stdin-filename".into(),
                s(file),
                "-".into(),
            ]
        },
        read: Some(parse::ruff),
    },
    Tool {
        id: "pylint",
        name: "Pylint",
        kind: Kind::Lint,
        files: &["py"],
        only: None,
        program: "pylint",
        place: Place::Python,
        configs: &[".pylintrc", "pylintrc"],
        package_keys: &[],
        sections: &["pylint"],
        activation: Activation::Installed,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            vec![
                "--output-format=json".into(),
                "--from-stdin".into(),
                s(file),
            ]
        },
        read: Some(parse::pylint),
    },
    Tool {
        id: "flake8",
        name: "flake8",
        kind: Kind::Lint,
        files: &["py"],
        only: None,
        program: "flake8",
        place: Place::Python,
        configs: &[".flake8"],
        package_keys: &[],
        sections: &["flake8"],
        activation: Activation::Installed,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            vec![
                "--format=%(row)d:%(col)d:%(code)s:%(text)s".into(),
                "--stdin-display-name".into(),
                s(file),
                "-".into(),
            ]
        },
        read: Some(parse::flake8),
    },
    Tool {
        id: "mypy",
        name: "mypy",
        kind: Kind::Lint,
        files: &["py", "pyi"],
        only: None,
        program: "mypy",
        place: Place::Python,
        configs: &["mypy.ini", ".mypy.ini"],
        package_keys: &[],
        sections: &["mypy"],
        activation: Activation::Installed,
        input: Input::File,
        cwd: Cwd::Config,
        args: |file| {
            with_file(
                &[
                    "--show-column-numbers",
                    "--show-error-end",
                    "--no-error-summary",
                    "--no-color-output",
                    "--hide-error-context",
                    "--no-pretty",
                ],
                file,
            )
        },
        read: Some(parse::mypy),
    },
    Tool {
        id: "sqlfluff",
        name: "SQLFluff",
        kind: Kind::Lint,
        files: &["sql"],
        only: None,
        program: "sqlfluff",
        place: Place::Python,
        configs: &[".sqlfluff"],
        package_keys: &[],
        sections: &["sqlfluff"],
        activation: Activation::Config,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            vec![
                "lint".into(),
                "--format".into(),
                "json".into(),
                "--stdin-filename".into(),
                s(file),
                "-".into(),
            ]
        },
        read: Some(parse::sqlfluff),
    },
    // ---- Shell and containers ----
    Tool {
        id: "shellcheck",
        name: "ShellCheck",
        kind: Kind::Lint,
        files: &[
            "sh",
            "bash",
            "ksh",
            "bats",
            ".bashrc",
            ".bash_profile",
            ".profile",
        ],
        only: None,
        program: "shellcheck",
        place: Place::System,
        configs: &[".shellcheckrc"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Found,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            let mut list = args(&["-f", "json1"]);
            let name = file.to_string_lossy().to_ascii_lowercase();
            if name.ends_with(".bash") || name.ends_with(".bats") || name.contains(".bash") {
                list.push("--shell=bash".into());
            } else if name.ends_with(".ksh") {
                list.push("--shell=ksh".into());
            }
            list.push("-".into());
            list
        },
        read: Some(parse::shellcheck),
    },
    Tool {
        id: "hadolint",
        name: "Hadolint",
        kind: Kind::Lint,
        files: &["dockerfile", "containerfile"],
        only: None,
        program: "hadolint",
        place: Place::System,
        configs: &[".hadolint.yaml", ".hadolint.yml"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Found,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |_| args(&["-f", "json", "-"]),
        read: Some(parse::hadolint),
    },
    // ---- Other languages ----
    Tool {
        id: "rubocop",
        name: "RuboCop",
        kind: Kind::Lint,
        files: &["rb", "rake", "gemspec", "ru", "gemfile", "rakefile"],
        only: None,
        program: "rubocop",
        place: Place::System,
        configs: &[".rubocop.yml"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Config,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            vec![
                "--format".into(),
                "json".into(),
                "--force-exclusion".into(),
                "--stdin".into(),
                s(file),
            ]
        },
        read: Some(parse::rubocop),
    },
    Tool {
        id: "phpcs",
        name: "PHP_CodeSniffer",
        kind: Kind::Lint,
        files: &["php"],
        only: None,
        program: "phpcs",
        place: Place::Php,
        configs: &[
            "phpcs.xml",
            ".phpcs.xml",
            "phpcs.xml.dist",
            ".phpcs.xml.dist",
        ],
        package_keys: &[],
        sections: &[],
        activation: Activation::Config,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            vec![
                "--report=json".into(),
                "-q".into(),
                format!("--stdin-path={}", s(file)),
                "-".into(),
            ]
        },
        read: Some(parse::phpcs),
    },
    Tool {
        id: "phpstan",
        name: "PHPStan",
        kind: Kind::Lint,
        files: &["php"],
        only: None,
        program: "phpstan",
        place: Place::Php,
        configs: &["phpstan.neon", "phpstan.neon.dist", "phpstan.dist.neon"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Config,
        input: Input::File,
        cwd: Cwd::Config,
        args: |file| {
            with_file(
                &[
                    "analyse",
                    "--error-format=json",
                    "--no-progress",
                    "--no-ansi",
                ],
                file,
            )
        },
        read: Some(parse::phpstan),
    },
    Tool {
        id: "golangci-lint",
        name: "golangci-lint",
        kind: Kind::Lint,
        files: &["go"],
        only: None,
        program: "golangci-lint",
        place: Place::System,
        configs: &[
            ".golangci.yml",
            ".golangci.yaml",
            ".golangci.toml",
            ".golangci.json",
        ],
        package_keys: &[],
        sections: &[],
        activation: Activation::Config,
        input: Input::File,
        cwd: Cwd::Config,
        args: |file| {
            let dir = file.parent().unwrap_or(file);
            with_file(
                &[
                    "run",
                    "--output.json.path=stdout",
                    "--show-stats=false",
                    "--issues-exit-code=0",
                ],
                dir,
            )
        },
        read: Some(parse::golangci),
    },
    Tool {
        id: "luacheck",
        name: "Luacheck",
        kind: Kind::Lint,
        files: &["lua", "rockspec"],
        only: None,
        program: "luacheck",
        place: Place::System,
        configs: &[".luacheckrc"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Found,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |file| {
            vec![
                "--formatter".into(),
                "plain".into(),
                "--codes".into(),
                "--ranges".into(),
                "--filename".into(),
                s(file),
                "-".into(),
            ]
        },
        read: Some(parse::luacheck),
    },
    Tool {
        id: "ktlint",
        name: "ktlint",
        kind: Kind::Lint,
        files: &["kt", "kts"],
        only: None,
        program: "ktlint",
        place: Place::System,
        configs: &[".editorconfig"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Found,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |_| args(&["--stdin", "--reporter=json"]),
        read: Some(parse::ktlint),
    },
    Tool {
        id: "swiftlint",
        name: "SwiftLint",
        kind: Kind::Lint,
        files: &["swift"],
        only: None,
        program: "swiftlint",
        place: Place::System,
        configs: &[".swiftlint.yml", ".swiftlint.yaml"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Found,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |_| args(&["lint", "--use-stdin", "--reporter", "json", "--quiet"]),
        read: Some(parse::swiftlint),
    },
    Tool {
        id: "hlint",
        name: "HLint",
        kind: Kind::Lint,
        files: &["hs", "lhs"],
        only: None,
        program: "hlint",
        place: Place::System,
        configs: &[".hlint.yaml"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Found,
        input: Input::Stdin,
        cwd: Cwd::Config,
        args: |_| args(&["--json", "-"]),
        read: Some(parse::hlint),
    },
    Tool {
        id: "dart",
        name: "Dart analyzer",
        kind: Kind::Lint,
        files: &["dart"],
        only: None,
        program: "dart",
        place: Place::System,
        configs: &["analysis_options.yaml", "pubspec.yaml"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Config,
        input: Input::File,
        cwd: Cwd::Config,
        args: |file| with_file(&["analyze", "--format=machine"], file),
        read: Some(parse::dart),
    },
    Tool {
        id: "clang-tidy",
        name: "clang-tidy",
        kind: Kind::Lint,
        files: &[
            "c", "cc", "cpp", "cxx", "c++", "h", "hh", "hpp", "hxx", "m", "mm",
        ],
        only: None,
        program: "clang-tidy",
        place: Place::System,
        configs: &[".clang-tidy"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Config,
        input: Input::File,
        cwd: Cwd::Config,
        args: |file| vec![s(file), "--quiet".into()],
        read: Some(parse::compiler),
    },
    Tool {
        id: "tflint",
        name: "TFLint",
        kind: Kind::Lint,
        files: &["tf"],
        only: None,
        program: "tflint",
        place: Place::System,
        configs: &[".tflint.hcl"],
        package_keys: &[],
        sections: &[],
        activation: Activation::Found,
        input: Input::File,
        cwd: Cwd::FileDir,
        args: |_| args(&["--format", "json"]),
        read: Some(parse::tflint),
    },
];

#[cfg(test)]
pub fn tool(id: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.id == id)
}

/// Whether `names` (extensions or file names) covers `path`.
pub fn covers(names: &[&str], path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    let extension = name.rsplit_once('.').map(|(_, e)| e);
    names
        .iter()
        .any(|n| *n == name || Some(*n) == extension || name.ends_with(&format!(".{n}")))
}

impl Tool {
    pub fn reads(&self, path: &Path) -> bool {
        covers(self.files, path) && self.only.is_none_or(|only| only(path))
    }
}

/// Where a tool stands for a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// It runs: its program, the folder it runs in, and its configuration.
    Ready {
        program: Found,
        cwd: PathBuf,
        config: Option<PathBuf>,
    },
    /// The project has its configuration, but its program is nowhere.
    Missing { config: Option<PathBuf> },
    /// Nothing in the project asks for it.
    Off,
}

/// The configuration of `tool` for a file of `dir`: the nearest, from `dir` up to the project
/// (or a few folders up, out of it).
pub fn config(tool: &Tool, dir: &Path, root: Option<&Path>) -> Option<PathBuf> {
    let within = |d: &Path| root.is_none_or(|r| d.starts_with(r) || !dir.starts_with(r));
    for d in dir.ancestors().take(12).take_while(|d| within(d)) {
        if let Some(found) = tool.configs.iter().map(|c| d.join(c)).find(|p| p.is_file()) {
            return Some(found);
        }
        if !tool.package_keys.is_empty() {
            let package = d.join("package.json");
            if let Some(json) = std::fs::read_to_string(&package)
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                && tool.package_keys.iter().any(|k| json.get(k).is_some())
            {
                return Some(package);
            }
        }
        for section in tool.sections {
            for (file, header) in [
                ("pyproject.toml", format!("[tool.{section}")),
                ("setup.cfg", format!("[{section}]")),
                ("tox.ini", format!("[{section}]")),
            ] {
                let path = d.join(file);
                if std::fs::read_to_string(&path).is_ok_and(|t| t.contains(&header)) {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Where `tool` stands for the file `path`, in the project `root`.
pub fn status(tool: &Tool, path: &Path, root: Option<&Path>) -> Status {
    let dir = path.parent().unwrap_or(Path::new("."));
    let config = config(tool, dir, root);
    let program = run::find(tool.program, tool.place, dir, root);
    let wanted = match tool.activation {
        Activation::Config => config.is_some(),
        Activation::Installed => config.is_some() || program.as_ref().is_some_and(|p| p.local),
        Activation::Found => program.is_some(),
    };
    if !wanted {
        return Status::Off;
    }
    let Some(program) = program else {
        return Status::Missing { config };
    };
    let cwd = match tool.cwd {
        Cwd::FileDir => dir.to_path_buf(),
        Cwd::Config => config
            .as_ref()
            .and_then(|c| c.parent())
            .map(Path::to_path_buf)
            .or_else(|| root.filter(|r| path.starts_with(r)).map(Path::to_path_buf))
            .unwrap_or_else(|| dir.to_path_buf()),
    };
    Status::Ready {
        program,
        cwd,
        config,
    }
}

/// The formatter of a file: among those that are ready and not turned off, one the project
/// configures, else the first ([`TOOLS`] order).
pub fn formatter(
    path: &Path,
    root: Option<&Path>,
    off: &[String],
) -> Option<(&'static Tool, Status)> {
    let ready: Vec<(&'static Tool, Status)> = TOOLS
        .iter()
        .filter(|t| t.kind == Kind::Format && t.reads(path) && !off.iter().any(|o| o == t.id))
        .map(|t| (t, status(t, path, root)))
        .filter(|(_, s)| matches!(s, Status::Ready { .. }))
        .collect();
    let configured = ready.iter().position(|(_, s)| {
        matches!(
            s,
            Status::Ready {
                config: Some(_),
                ..
            }
        )
    });
    let index = configured.unwrap_or(0);
    ready.into_iter().nth(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lint-catalog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (file, text) in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        dir
    }

    fn shim(name: &str) -> String {
        if cfg!(windows) {
            format!("node_modules/.bin/{name}.cmd")
        } else {
            format!("node_modules/.bin/{name}")
        }
    }

    #[test]
    fn files_are_covered_by_extension_or_name() {
        assert!(covers(&["ts"], Path::new("src/App.TS")));
        assert!(covers(&["dockerfile"], Path::new("Dockerfile")));
        assert!(covers(&["dockerfile"], Path::new("api.dockerfile")));
        assert!(covers(&[".bashrc"], Path::new("/home/me/.bashrc")));
        assert!(!covers(&["ts"], Path::new("a.js")));
        let actionlint = tool("actionlint").unwrap();
        assert!(actionlint.reads(Path::new("/p/.github/workflows/ci.yml")));
        assert!(!actionlint.reads(Path::new("/p/config.yml")));
    }

    #[test]
    fn a_style_linter_runs_only_with_a_configuration() {
        let root = project(
            "eslint",
            &[(&shim("eslint"), ""), ("package.json", "{\"name\":\"x\"}")],
        );
        let eslint = tool("eslint").unwrap();
        let file = root.join("src/a.ts");
        // Installed, but no configuration: ESLint 9 would refuse to run.
        assert_eq!(status(eslint, &file, Some(&root)), Status::Off);
        std::fs::write(root.join("eslint.config.js"), "export default [];").unwrap();
        let Status::Ready {
            program,
            cwd,
            config,
        } = status(eslint, &file, Some(&root))
        else {
            panic!("ESLint is configured");
        };
        assert!(program.local);
        assert_eq!(cwd, root);
        assert_eq!(config, Some(root.join("eslint.config.js")));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_configuration_in_package_json_or_pyproject_counts() {
        let root = project(
            "keys",
            &[
                ("package.json", "{\"prettier\": {\"semi\": false}}"),
                (
                    "pyproject.toml",
                    "[project]\nname = \"x\"\n\n[tool.ruff.lint]\nselect = [\"E\"]\n",
                ),
            ],
        );
        let prettier = tool("prettier").unwrap();
        assert_eq!(
            config(prettier, &root.join("src"), Some(&root)),
            Some(root.join("package.json"))
        );
        let ruff = tool("ruff").unwrap();
        assert_eq!(
            config(ruff, &root, Some(&root)),
            Some(root.join("pyproject.toml"))
        );
        assert_eq!(config(tool("pylint").unwrap(), &root, Some(&root)), None);
        // Configured, but nowhere to be found: the window tells it.
        if run::find("prettier", Place::Node, &root, Some(&root)).is_none() {
            assert!(matches!(
                status(prettier, &root.join("a.ts"), Some(&root)),
                Status::Missing { .. }
            ));
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_formatter_the_project_configures_is_chosen() {
        let root = project(
            "formatter",
            &[
                (&shim("prettier"), ""),
                (&shim("oxfmt"), ""),
                (".prettierrc", "{}"),
            ],
        );
        let file = root.join("a.ts");
        let (chosen, _) = formatter(&file, Some(&root), &[]).unwrap();
        assert_eq!(chosen.id, "prettier");
        std::fs::write(root.join(".oxfmtrc.json"), "{}").unwrap();
        // Both configured: the first of the list.
        assert_eq!(formatter(&file, Some(&root), &[]).unwrap().0.id, "oxfmt");
        // Turned off in the window.
        let off = vec!["oxfmt".to_string()];
        assert_eq!(
            formatter(&file, Some(&root), &off).unwrap().0.id,
            "prettier"
        );
        // TOML is Oxfmt's only.
        assert_eq!(
            formatter(&root.join("Cargo.toml"), Some(&root), &[])
                .unwrap()
                .0
                .id,
            "oxfmt"
        );
        assert!(formatter(&root.join("a.rs"), Some(&root), &[]).is_none());
        std::fs::remove_dir_all(&root).ok();
    }
}
