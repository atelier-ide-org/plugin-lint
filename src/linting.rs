//! Linting in the background: each document waits a moment after its last change, then the tools
//! that read it run side by side, and their problems go to the IDE together. A tool that reads
//! the file on disk runs when the text of the editor is the one of the file (once saved); until
//! then, its last problems stay.

use crate::{
    catalog::{self, Input, Kind, Status, TOOLS, Tool},
    parse,
    run::{self, Output},
    text::{Text, plain},
};
use atelier_plugin_api::{
    Host,
    protocol::{Diagnostic, Document},
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

/// How long a document waits after its last change.
const QUIET: Duration = Duration::from_millis(350);
/// How long a tool may run.
const TIMEOUT: Duration = Duration::from_secs(30);

/// The problems a tool found in a document, or why it failed, and how long it took.
type Run = (Result<Vec<Diagnostic>, String>, Duration);

/// How the last run of a tool went, for the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Found(usize),
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct Report {
    pub file: PathBuf,
    pub outcome: Outcome,
    pub took: Duration,
}

#[derive(Default)]
struct State {
    pending: HashMap<PathBuf, (Document, Instant)>,
    running: HashSet<PathBuf>,
    /// The problems the tools that read the file found in it last, by file and tool.
    on_disk: HashMap<PathBuf, HashMap<&'static str, Vec<Diagnostic>>>,
    /// The version of each document whose problems went to the IDE last.
    published: HashMap<PathBuf, u64>,
    reports: HashMap<&'static str, Report>,
    /// The failures already told, so that typing does not repeat them.
    told: HashSet<(&'static str, String)>,
    /// The documents closed: an answer that comes for them is dropped.
    closed: HashSet<PathBuf>,
    /// The tools turned off in the window.
    off: Vec<String>,
    started: bool,
}

pub struct Linter {
    state: Mutex<State>,
    wake: Condvar,
}

impl Default for Linter {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            wake: Condvar::new(),
        }
    }
}

/// The same text, whatever its line ends (the editor may not keep the file's).
fn same_text(a: &str, b: &str) -> bool {
    a == b || a.replace("\r\n", "\n") == b.replace("\r\n", "\n")
}

impl Linter {
    pub fn set_off(&self, off: Vec<String>) {
        self.state.lock().unwrap().off = off;
    }

    pub fn reports(&self) -> HashMap<&'static str, Report> {
        self.state.lock().unwrap().reports.clone()
    }

    /// A document changed: it is linted once it stays the same for a moment.
    pub fn document(self: &Arc<Self>, document: Document, host: &Host) {
        let mut state = self.state.lock().unwrap();
        if !state.started {
            state.started = true;
            let (linter, host) = (self.clone(), host.clone());
            std::thread::spawn(move || linter.work(&host));
        }
        state.closed.remove(&document.path);
        state
            .pending
            .insert(document.path.clone(), (document, Instant::now()));
        self.wake.notify_all();
    }

    pub fn closed(&self, path: &Path) {
        let mut state = self.state.lock().unwrap();
        state.pending.remove(path);
        state.on_disk.remove(path);
        state.published.remove(path);
        state.closed.insert(path.to_path_buf());
    }

    /// The loop of the background thread: starts the documents that waited long enough.
    fn work(self: &Arc<Self>, host: &Host) {
        let mut state = self.state.lock().unwrap();
        loop {
            let now = Instant::now();
            let due: Vec<PathBuf> = state
                .pending
                .iter()
                .filter(|(path, (_, at))| now >= *at + QUIET && !state.running.contains(*path))
                .map(|(path, _)| path.clone())
                .collect();
            for path in due {
                let Some((document, _)) = state.pending.remove(&path) else {
                    continue;
                };
                state.running.insert(path);
                let (linter, host) = (self.clone(), host.clone());
                std::thread::spawn(move || linter.lint(document, &host));
            }
            let next = state
                .pending
                .values()
                .map(|(_, at)| *at + QUIET)
                .min()
                .map_or(Duration::from_secs(3600), |at| {
                    at.saturating_duration_since(Instant::now())
                        .max(Duration::from_millis(10))
                });
            state = self.wake.wait_timeout(state, next).unwrap().0;
        }
    }

    /// Runs the tools of a document, and gives their problems to the IDE.
    fn lint(self: &Arc<Self>, document: Document, host: &Host) {
        let root = host.project().root;
        let off = self.state.lock().unwrap().off.clone();
        let path = document.path.clone();
        let ready: Vec<(&'static Tool, Status)> = TOOLS
            .iter()
            .filter(|t| t.kind == Kind::Lint && t.reads(&path) && !off.iter().any(|o| o == t.id))
            .map(|t| (t, catalog::status(t, &path, root.as_deref())))
            .filter(|(_, s)| matches!(s, Status::Ready { .. }))
            .collect();
        let disk = std::fs::read_to_string(&path).ok();
        let saved = disk
            .as_deref()
            .is_some_and(|d| same_text(d, &document.text));
        let results: Vec<(&'static Tool, Option<Run>)> = std::thread::scope(|scope| {
            let jobs: Vec<_> = ready
                .iter()
                .map(|(tool, status)| {
                    let text = match tool.input {
                        Input::Stdin => Some(document.text.as_str()),
                        Input::File if saved => disk.as_deref(),
                        Input::File => None,
                    };
                    let job = text.map(|text| {
                        let path = &path;
                        scope.spawn(move || {
                            let started = Instant::now();
                            (lint_with(tool, status, path, text), started.elapsed())
                        })
                    });
                    (*tool, job)
                })
                .collect();
            jobs.into_iter()
                .map(|(tool, job)| (tool, job.and_then(|j| j.join().ok())))
                .collect()
        });
        let mut state = self.state.lock().unwrap();
        state.running.remove(&path);
        self.wake.notify_all();
        let mut found = Vec::new();
        let mut failures = Vec::new();
        for (tool, result) in results {
            match result {
                Some((Ok(problems), took)) => {
                    state.reports.insert(
                        tool.id,
                        Report {
                            file: path.clone(),
                            outcome: Outcome::Found(problems.len()),
                            took,
                        },
                    );
                    if tool.input == Input::File {
                        state
                            .on_disk
                            .entry(path.clone())
                            .or_default()
                            .insert(tool.id, problems.clone());
                    }
                    found.extend(problems);
                }
                Some((Err(reason), took)) => {
                    state.reports.insert(
                        tool.id,
                        Report {
                            file: path.clone(),
                            outcome: Outcome::Failed(reason.clone()),
                            took,
                        },
                    );
                    if state.told.insert((tool.id, reason.clone())) {
                        failures.push(format!("{}: {reason}", tool.name));
                    }
                }
                // Not saved: its last problems in the file, while its lines are the same.
                None => {
                    if let Some(kept) = state.on_disk.get(&path).and_then(|t| t.get(tool.id)) {
                        found.extend(kept.iter().cloned());
                    }
                }
            }
        }
        // Closed meanwhile, or an answer for a newer version went first.
        let closed = state.closed.contains(&path);
        let older = state
            .published
            .get(&path)
            .is_some_and(|v| *v > document.version);
        if !older && !closed {
            state.published.insert(path.clone(), document.version);
            drop(state);
            found.sort_by_key(|d| (d.line, d.column));
            host.publish_diagnostics(path, document.version, found);
        } else {
            drop(state);
        }
        for failure in failures {
            host.error(failure);
        }
        host.request_repaint();
    }
}

/// Runs a tool on the text of `path`, and reads its report.
fn lint_with(
    tool: &Tool,
    status: &Status,
    path: &Path,
    text: &str,
) -> Result<Vec<Diagnostic>, String> {
    let Status::Ready { program, cwd, .. } = status else {
        return Ok(Vec::new());
    };
    let input = (tool.input == Input::Stdin).then_some(text);
    let output = run::run(&program.path, &(tool.args)(path), cwd, input, TIMEOUT)?;
    read(tool, &output, text, path)
}

/// The problems of a report; nothing written and a success is no problem.
pub fn read(
    tool: &Tool,
    output: &Output,
    text: &str,
    path: &Path,
) -> Result<Vec<Diagnostic>, String> {
    let output = Output {
        stdout: plain(&output.stdout),
        stderr: plain(&output.stderr),
        code: output.code,
    };
    if output.stdout.trim().is_empty() && output.code == Some(0) {
        return Ok(Vec::new());
    }
    let Some(reader) = tool.read else {
        return Ok(Vec::new());
    };
    let found = reader(&output, &Text::new(text), path, tool.id);
    // A report that could not be read, from a program that failed: why it failed.
    found.map_err(|e| {
        if output.stderr.trim().is_empty() {
            e
        } else {
            parse::failure(&output.stderr)
        }
    })
}

/// Formats `text` with `tool`: the new text, or why it could not be formatted.
pub fn format(tool: &Tool, status: &Status, path: &Path, text: &str) -> Result<String, String> {
    let Status::Ready { program, cwd, .. } = status else {
        return Err(format!("{} is not installed", tool.name));
    };
    let output = run::run(&program.path, &(tool.args)(path), cwd, Some(text), TIMEOUT)?;
    // Nothing back for a text that was not empty: it failed without saying it.
    let answered = !output.stdout.is_empty() || text.is_empty();
    if output.code == Some(0) && answered {
        Ok(output.stdout)
    } else {
        Err(format!(
            "{}: {}",
            tool.name,
            parse::failure(&plain(output.report()))
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_tool(id: &str, root: &Path, name: &str, text: &str) -> Result<Vec<Diagnostic>, String> {
        let tool = TOOLS.iter().find(|t| t.id == id).unwrap();
        let path = root.join(name);
        std::fs::write(&path, text).unwrap();
        let status = catalog::status(tool, &path, Some(root));
        assert!(matches!(status, Status::Ready { .. }), "{id}: {status:?}");
        lint_with(tool, &status, &path, text)
    }

    /// The real tools, in projects that have them: `LINT_TEST_JS` (ESLint, Oxlint, Biome,
    /// Stylelint, markdownlint, Prettier and Oxfmt in `node_modules`, with the configurations of
    /// ESLint, Stylelint and markdownlint) and `LINT_TEST_PY` (Ruff, Pylint, flake8, mypy and
    /// yamllint in `.venv`).
    #[test]
    fn real_tools_find_problems_and_format() {
        if let Some(root) = std::env::var_os("LINT_TEST_JS").map(PathBuf::from) {
            let js = "const é = 1;\nlet unused = 2\ndebugger;\n";
            for id in ["eslint", "oxlint", "biome"] {
                let found = run_tool(id, &root, "sample.js", js).unwrap();
                assert!(!found.is_empty(), "{id} found nothing");
                assert!(found.iter().all(|d| d.source == id));
            }
            let found = run_tool("eslint", &root, "sample.js", js).unwrap();
            let unused = found
                .iter()
                .find(|d| d.code.as_deref() == Some("no-unused-vars"))
                .unwrap();
            assert_eq!((unused.line, unused.column, unused.end_column), (0, 6, 7));
            let found = run_tool("stylelint", &root, "sample.css", "a { colr: red; }\n").unwrap();
            assert_eq!(found[0].code.as_deref(), Some("property-no-unknown"));
            let found = run_tool("markdownlint", &root, "sample.md", "#Title\n").unwrap();
            assert!(found.iter().any(|d| d.code.as_deref() == Some("MD018")));
            for id in ["prettier", "oxfmt"] {
                let tool = TOOLS.iter().find(|t| t.id == id).unwrap();
                let path = root.join("sample.ts");
                let status = catalog::status(tool, &path, Some(&root));
                assert_eq!(
                    format(tool, &status, &path, "const  a={b:1}").unwrap(),
                    "const a = { b: 1 };\n"
                );
                let error = format(tool, &status, &path, "const = ;").unwrap_err();
                assert!(error.starts_with(tool.name), "{error}");
            }
        }
        if let Some(root) = std::env::var_os("LINT_TEST_PY").map(PathBuf::from) {
            let py = "import os\nx = \"é\"; y: int = \"s\"\n";
            for id in ["ruff", "pylint", "flake8", "mypy"] {
                let found = run_tool(id, &root, "sample.py", py).unwrap();
                assert!(!found.is_empty(), "{id} found nothing");
            }
            let found = run_tool("yamllint", &root, "sample.yaml", "a:   1\n").unwrap();
            assert!(found.iter().any(|d| d.code.as_deref() == Some("colons")));
        }
    }
}
