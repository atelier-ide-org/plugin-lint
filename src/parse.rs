//! The reports of the tools, read into problems ([`Diagnostic`]). Each reader gets what the
//! program wrote, the text it read (to place the problems) and the file (to keep only its
//! problems when a tool reports on several).

use crate::{
    run::Output,
    text::{Text, Unit, edit, fix, problem, problem_at_bytes},
};
use atelier_plugin_api::protocol::{Diagnostic, Severity};
use serde_json::Value;
use std::path::Path;

pub type Reader = fn(&Output, &Text, &Path, &str) -> Result<Vec<Diagnostic>, String>;

/// The JSON of a report, which some tools put after other lines.
fn json(report: &str) -> Result<Value, String> {
    let trimmed = report.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Ok(value);
    }
    // From the first line that starts a JSON value.
    let mut at = 0;
    for line in report.split_inclusive('\n') {
        let start = line.trim_start();
        if start.starts_with('{') || start.starts_with('[') {
            let rest = &report[at + (line.len() - start.len())..];
            let mut values = serde_json::Deserializer::from_str(rest).into_iter::<Value>();
            if let Some(Ok(value)) = values.next() {
                return Ok(value);
            }
        }
        at += line.len();
    }
    Err(failure(report))
}

/// The first lines of what a tool wrote, as the reason it failed.
pub fn failure(report: &str) -> String {
    let lines: Vec<&str> = report
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .take(6)
        .collect();
    if lines.is_empty() {
        "no report".into()
    } else {
        lines.join("\n")
    }
}

fn int(value: &Value, key: &str) -> Option<usize> {
    value.get(key).and_then(Value::as_u64).map(|n| n as usize)
}

fn string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn array<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn level(name: &str) -> Severity {
    match name.to_ascii_lowercase().as_str() {
        "error" | "fatal" | "failure" => Severity::Error,
        "warning" | "warn" => Severity::Warning,
        "info" | "information" | "advice" | "convention" | "refactor" | "suggestion" | "note"
        | "style" => Severity::Info,
        _ => Severity::Hint,
    }
}

/// Whether `reported` (absolute, relative to the folder the tool ran in, or `stdin`) names
/// `file`.
fn same_file(reported: &str, file: &Path) -> bool {
    let reported = reported.replace('\\', "/").to_lowercase();
    let reported = reported.trim_start_matches("./");
    let file = file.to_string_lossy().replace('\\', "/").to_lowercase();
    reported == "stdin"
        || reported == "-"
        || reported == file
        || file.ends_with(&format!("/{reported}"))
}

/// `name: 1:2: …` lines: the 1-based line and column, and the rest.
fn located_lines<'a>(
    report: &'a str,
    file: &'a Path,
) -> impl Iterator<Item = (usize, usize, &'a str)> {
    report.lines().filter_map(move |line| {
        // A Windows path has a colon after its drive letter.
        let skip = usize::from(line.as_bytes().get(1) == Some(&b':')) * 2;
        let mut parts = line[skip..].splitn(4, ':');
        let name = format!("{}{}", &line[..skip], parts.next()?);
        let row = parts.next()?.trim().parse().ok()?;
        let column = parts.next()?.trim().parse().ok()?;
        let rest = parts.next()?;
        same_file(&name, file).then_some((row, column, rest))
    })
}

/// The fix of ESLint `{range: [start, end], text}`, its range in UTF-16 units.
fn eslint_edit(text: &Text, fix: &Value) -> Option<atelier_plugin_api::protocol::Edit> {
    let range = fix.get("range")?.as_array()?;
    let start = range.first()?.as_u64()? as usize;
    let end = range.get(1)?.as_u64()? as usize;
    Some(edit(
        text.position_utf16(start),
        text.position_utf16(end),
        string(fix, "text").unwrap_or_default(),
    ))
}

/// Whether a line comment of JavaScript may go above a line of the file (not in a Vue or
/// Svelte template).
fn js_comments(file: &Path) -> bool {
    let extension = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"].contains(&extension.as_str())
}

/// ESLint, `--format json`: columns in UTF-16 units, as JavaScript counts. Its fix, its
/// suggestions, and a comment that turns the rule off for the line.
pub fn eslint(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(out.report())?;
    let mut found = Vec::new();
    for entry in report.as_array().into_iter().flatten() {
        for m in array(entry, "messages") {
            let message = string(m, "message").unwrap_or_default();
            // A file its configuration leaves out is not a problem of the file.
            if m.get("ruleId").is_some_and(Value::is_null)
                && (message.starts_with("File ignored")
                    || message.contains("matching configuration"))
            {
                continue;
            }
            let end = int(m, "endLine").zip(int(m, "endColumn"));
            let rule = string(m, "ruleId");
            let mut found_here = problem(
                text,
                int(m, "line").unwrap_or(1),
                int(m, "column").unwrap_or(1),
                end,
                Unit::Utf16,
                if int(m, "severity") == Some(2) {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                message,
                source,
                rule.clone(),
            );
            let name = rule.clone().unwrap_or_default();
            if let Some(e) = m.get("fix").and_then(|f| eslint_edit(text, f)) {
                found_here.fixes.push(fix(
                    trf!("lint.fix_rule", tool = source, rule = name),
                    vec![e],
                    true,
                ));
            }
            for suggestion in array(m, "suggestions") {
                if let Some(e) = suggestion.get("fix").and_then(|f| eslint_edit(text, f)) {
                    let title = string(suggestion, "desc").unwrap_or_else(|| name.clone());
                    found_here
                        .fixes
                        .push(fix(format!("{source}: {title}"), vec![e], false));
                }
            }
            if let Some(rule) = rule.filter(|_| js_comments(file)) {
                let line = found_here.line;
                let indent = &text.line(line as usize)[..text.line_start(line as usize) as usize];
                found_here.fixes.push(fix(
                    trf!("lint.disable_line", tool = source, rule = rule),
                    vec![edit(
                        (line, 0),
                        (line, 0),
                        format!("{indent}// eslint-disable-next-line {rule}\n"),
                    )],
                    false,
                ));
            }
            found.push(found_here);
        }
    }
    Ok(found)
}

/// Oxlint, `--format json`: spans in bytes.
pub fn oxlint(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(out.report())?;
    let list = report
        .get("diagnostics")
        .and_then(Value::as_array)
        .or(report.as_array())
        .ok_or_else(|| failure(out.report()))?;
    Ok(list
        .iter()
        .filter(|d| string(d, "filename").is_none_or(|f| same_file(&f, file)))
        .map(|d| {
            let span = d
                .get("labels")
                .and_then(Value::as_array)
                .and_then(|l| l.first())
                .and_then(|l| l.get("span"));
            let offset = span.and_then(|s| int(s, "offset")).unwrap_or(0);
            let length = span.and_then(|s| int(s, "length")).unwrap_or(0);
            let mut message = string(d, "message").unwrap_or_default();
            if let Some(help) = string(d, "help").filter(|h| !h.is_empty()) {
                message = format!("{message}\n{help}");
            }
            problem_at_bytes(
                text,
                offset..offset + length,
                level(&string(d, "severity").unwrap_or_default()),
                message,
                source,
                string(d, "code"),
            )
        })
        .collect())
}

/// Biome, `--reporter=json`.
pub fn biome(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(&out.stdout).or_else(|_| json(out.report()))?;
    Ok(array(&report, "diagnostics")
        .filter(|d| {
            d.pointer("/location/path")
                .and_then(Value::as_str)
                .is_none_or(|p| same_file(p, file))
        })
        .map(|d| {
            let at = |key: &str| {
                d.pointer(&format!("/location/{key}"))
                    .map(|p| (int(p, "line").unwrap_or(1), int(p, "column").unwrap_or(1)))
            };
            let (line, column) = at("start").unwrap_or((1, 1));
            problem(
                text,
                line,
                column,
                at("end"),
                Unit::Chars,
                level(&string(d, "severity").unwrap_or_default()),
                string(d, "message").unwrap_or_default(),
                source,
                string(d, "category"),
            )
        })
        .collect())
}

/// Stylelint, `--formatter json` (on its errors): its texts end with the rule.
pub fn stylelint(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(out.report())?;
    let mut found = Vec::new();
    for file in report.as_array().into_iter().flatten() {
        if let Some(warning) = array(file, "invalidOptionWarnings").next() {
            return Err(string(warning, "text").unwrap_or_default());
        }
        for w in array(file, "warnings") {
            let rule = string(w, "rule");
            let mut message = string(w, "text").unwrap_or_default();
            if let Some(rule) = &rule {
                message = message
                    .trim_end()
                    .trim_end_matches(&format!("({rule})"))
                    .trim_end()
                    .to_string();
            }
            found.push(problem(
                text,
                int(w, "line").unwrap_or(1),
                int(w, "column").unwrap_or(1),
                int(w, "endLine").zip(int(w, "endColumn")),
                Unit::Chars,
                level(&string(w, "severity").unwrap_or_default()),
                message,
                source,
                rule,
            ));
        }
    }
    Ok(found)
}

/// markdownlint, `--json` (on its errors): style, so warnings.
pub fn markdownlint(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    if out.report().trim().is_empty() {
        return Ok(Vec::new());
    }
    let report = json(out.report())?;
    Ok(report
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| {
            let line = int(e, "lineNumber").unwrap_or(1);
            let range = e
                .get("errorRange")
                .and_then(Value::as_array)
                .and_then(|r| Some((r.first()?.as_u64()? as usize, r.get(1)?.as_u64()? as usize)));
            let (column, end) = match range {
                Some((column, length)) => (column, Some((line, column + length))),
                None => (1, Some((line, usize::MAX))),
            };
            let mut message = string(e, "ruleDescription").unwrap_or_default();
            if let Some(detail) = string(e, "errorDetail").filter(|d| !d.is_empty()) {
                message = format!("{message} [{detail}]");
            }
            let code = e
                .get("ruleNames")
                .and_then(Value::as_array)
                .and_then(|n| n.first())
                .and_then(Value::as_str)
                .map(str::to_string);
            let mut found = problem(
                text,
                line,
                column,
                end,
                Unit::Chars,
                Severity::Warning,
                message,
                source,
                code.clone(),
            );
            if let Some(info) = e.get("fixInfo").filter(|f| f.is_object()) {
                let at_line = int(info, "lineNumber").unwrap_or(line);
                let column = int(info, "editColumn").unwrap_or(1);
                let deleted = info.get("deleteCount").and_then(Value::as_i64).unwrap_or(0);
                let inserted = string(info, "insertText").unwrap_or_default();
                let change = if deleted < 0 {
                    // The whole line goes.
                    let line0 = at_line.saturating_sub(1) as u32;
                    edit((line0, 0), (line0 + 1, 0), inserted)
                } else {
                    edit(
                        text.at(at_line, column, Unit::Chars),
                        text.at(at_line, column + deleted as usize, Unit::Chars),
                        inserted,
                    )
                };
                let rule = code.unwrap_or_default();
                found.fixes.push(fix(
                    trf!("lint.fix_rule", tool = source, rule = rule),
                    vec![change],
                    true,
                ));
            }
            found
        })
        .collect())
}

/// Ruff, `--output-format json`: columns in characters.
pub fn ruff(out: &Output, text: &Text, _: &Path, source: &str) -> Result<Vec<Diagnostic>, String> {
    let report = json(out.report())?;
    Ok(report
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| {
            let at = |key: &str| {
                d.get(key)
                    .map(|p| (int(p, "row").unwrap_or(1), int(p, "column").unwrap_or(1)))
            };
            let (line, column) = at("location").unwrap_or((1, 1));
            let code = string(d, "code");
            // Without a code, a syntax error.
            let severity = if code.is_none() {
                Severity::Error
            } else {
                Severity::Warning
            };
            let mut found = problem(
                text,
                line,
                column,
                at("end_location"),
                Unit::Chars,
                severity,
                string(d, "message").unwrap_or_default(),
                source,
                code.clone(),
            );
            if let Some(f) = d.get("fix").filter(|f| f.is_object()) {
                let applicability = string(f, "applicability").unwrap_or_default();
                let edits: Vec<_> = array(f, "edits")
                    .filter_map(|e| {
                        let place = |key: &str| {
                            let p = e.get(key)?;
                            Some(text.at(int(p, "row")?, int(p, "column")?, Unit::Chars))
                        };
                        Some(edit(
                            place("location")?,
                            place("end_location")?,
                            string(e, "content").unwrap_or_default(),
                        ))
                    })
                    .collect();
                if applicability != "display-only" && !edits.is_empty() {
                    let title =
                        string(f, "message").unwrap_or_else(|| code.clone().unwrap_or_default());
                    found.fixes.push(fix(
                        format!("{source}: {title}"),
                        edits,
                        applicability == "safe",
                    ));
                }
            }
            // `# noqa: CODE` at the end of the line, unless it has one.
            if let Some(code) = &code {
                let line0 = found.line as usize;
                if !text.line(line0).contains("# noqa") {
                    let end = text.line_end(line0);
                    found.fixes.push(fix(
                        trf!("lint.disable_line", tool = source, rule = code),
                        vec![edit(
                            (line0 as u32, end),
                            (line0 as u32, end),
                            format!("  # noqa: {code}"),
                        )],
                        false,
                    ));
                }
            }
            found
        })
        .collect())
}

/// Pylint, `--output-format=json`: 0-based columns.
pub fn pylint(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    if out.report().trim().is_empty() {
        return Ok(Vec::new());
    }
    let report = json(out.report())?;
    Ok(report
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| {
            let line = int(m, "line").unwrap_or(1);
            let column = int(m, "column").unwrap_or(0) + 1;
            let end = int(m, "endLine")
                .zip(int(m, "endColumn"))
                .map(|(l, c)| (l, c + 1));
            problem(
                text,
                line,
                column,
                end,
                Unit::Chars,
                level(&string(m, "type").unwrap_or_default()),
                string(m, "message").unwrap_or_default(),
                source,
                string(m, "symbol"),
            )
        })
        .collect())
}

/// flake8, with `--format=%(row)d:%(col)d:%(code)s:%(text)s`.
pub fn flake8(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    Ok(out
        .stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, ':');
            let row = parts.next()?.parse().ok()?;
            let column = parts.next()?.parse().ok()?;
            let code = parts.next()?.to_string();
            let message = parts.next()?.trim().to_string();
            // Syntax errors and names that do not exist.
            let error = ["E9", "F63", "F7", "F82"]
                .iter()
                .any(|p| code.starts_with(p));
            let severity = if error {
                Severity::Error
            } else {
                Severity::Warning
            };
            Some(problem(
                text,
                row,
                column,
                None,
                Unit::Chars,
                severity,
                message,
                source,
                Some(code),
            ))
        })
        .collect())
}

/// mypy, with `--show-column-numbers --show-error-end`: `file:line:col:endline:endcol: error:
/// message  [code]`, the end included.
pub fn mypy(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let mut found = Vec::new();
    for (line, column, rest) in located_lines(&out.stdout, file) {
        let mut parts = rest.splitn(3, ':');
        let end_line: Option<usize> = parts.next().and_then(|p| p.trim().parse().ok());
        let end_column: Option<usize> = parts.next().and_then(|p| p.trim().parse().ok());
        let (end, rest) = match (end_line, end_column, parts.next()) {
            (Some(l), Some(c), Some(rest)) => (Some((l, c + 1)), rest),
            _ => (None, rest),
        };
        let Some((kind, message)) = rest.trim().split_once(':') else {
            continue;
        };
        let (message, code) = match message.trim().rsplit_once("  [") {
            Some((m, c)) => (m.to_string(), Some(c.trim_end_matches(']').to_string())),
            None => (message.trim().to_string(), None),
        };
        found.push(problem(
            text,
            line,
            column,
            end,
            Unit::Chars,
            level(kind.trim()),
            message,
            source,
            code,
        ));
    }
    Ok(found)
}

/// yamllint, `-f parsable`: `stdin:1:5: [error] message (rule)`.
pub fn yamllint(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    Ok(located_lines(&out.stdout, file)
        .filter_map(|(line, column, rest)| {
            let rest = rest.trim();
            let (kind, message) = rest.strip_prefix('[')?.split_once(']')?;
            let message = message.trim();
            let (message, code) = match message.rsplit_once(" (") {
                Some((m, c)) if c.ends_with(')') => {
                    (m.to_string(), Some(c.trim_end_matches(')').to_string()))
                }
                _ => (message.to_string(), None),
            };
            Some(problem(
                text,
                line,
                column,
                None,
                Unit::Chars,
                level(kind),
                message,
                source,
                code,
            ))
        })
        .collect())
}

/// ShellCheck, `-f json1`.
pub fn shellcheck(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(out.report())?;
    Ok(array(&report, "comments")
        .map(|c| {
            let end = int(c, "endLine").zip(int(c, "endColumn"));
            let code = int(c, "code").map(|n| format!("SC{n}"));
            let mut found = problem(
                text,
                int(c, "line").unwrap_or(1),
                int(c, "column").unwrap_or(1),
                end,
                Unit::Chars,
                level(&string(c, "level").unwrap_or_default()),
                string(c, "message").unwrap_or_default(),
                source,
                code.clone(),
            );
            let edits: Vec<_> = c
                .pointer("/fix/replacements")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|r| {
                    Some(edit(
                        text.at(int(r, "line")?, int(r, "column")?, Unit::Chars),
                        text.at(int(r, "endLine")?, int(r, "endColumn")?, Unit::Chars),
                        string(r, "replacement").unwrap_or_default(),
                    ))
                })
                .collect();
            if !edits.is_empty() {
                let rule = code.unwrap_or_default();
                found.fixes.push(fix(
                    trf!("lint.fix_rule", tool = source, rule = rule),
                    edits,
                    true,
                ));
            }
            found
        })
        .collect())
}

/// Hadolint, `-f json`: lines only, most of the time.
pub fn hadolint(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    if out.report().trim().is_empty() {
        return Ok(Vec::new());
    }
    let report = json(out.report())?;
    Ok(report
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| {
            let line = int(d, "line").unwrap_or(1);
            problem(
                text,
                line,
                1,
                Some((line, usize::MAX)),
                Unit::Chars,
                level(&string(d, "level").unwrap_or_default()),
                string(d, "message").unwrap_or_default(),
                source,
                string(d, "code"),
            )
        })
        .collect())
}

/// RuboCop, `--format json`: the last column is included.
pub fn rubocop(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(out.report())?;
    let mut found = Vec::new();
    for file in array(&report, "files") {
        for o in array(file, "offenses") {
            let location = o.get("location").cloned().unwrap_or_default();
            let line = int(&location, "start_line")
                .or(int(&location, "line"))
                .unwrap_or(1);
            let column = int(&location, "start_column")
                .or(int(&location, "column"))
                .unwrap_or(1);
            let end = int(&location, "last_line")
                .zip(int(&location, "last_column"))
                .map(|(l, c)| (l, c + 1));
            let cop = string(o, "cop_name");
            let mut message = string(o, "message").unwrap_or_default();
            if let Some(cop) = &cop {
                message = message.trim_start_matches(&format!("{cop}: ")).to_string();
            }
            found.push(problem(
                text,
                line,
                column,
                end,
                Unit::Chars,
                level(&string(o, "severity").unwrap_or_default()),
                message,
                source,
                cop,
            ));
        }
    }
    Ok(found)
}

/// PHP_CodeSniffer, `--report=json`.
pub fn phpcs(out: &Output, text: &Text, _: &Path, source: &str) -> Result<Vec<Diagnostic>, String> {
    let report = json(out.report())?;
    let mut found = Vec::new();
    for file in report
        .get("files")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|f| f.values())
    {
        for m in array(file, "messages") {
            found.push(problem(
                text,
                int(m, "line").unwrap_or(1),
                int(m, "column").unwrap_or(1),
                None,
                Unit::Chars,
                level(&string(m, "type").unwrap_or_default()),
                string(m, "message").unwrap_or_default(),
                source,
                string(m, "source"),
            ));
        }
    }
    Ok(found)
}

/// PHPStan, `--error-format=json`: lines only.
pub fn phpstan(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(out.report())?;
    let mut found = Vec::new();
    let files = report.get("files").and_then(Value::as_object);
    for (name, entry) in files.into_iter().flatten() {
        if !same_file(name, file) {
            continue;
        }
        for m in array(entry, "messages") {
            let line = int(m, "line").unwrap_or(1);
            found.push(problem(
                text,
                line,
                1,
                Some((line, usize::MAX)),
                Unit::Chars,
                Severity::Error,
                string(m, "message").unwrap_or_default(),
                source,
                string(m, "identifier"),
            ));
        }
    }
    Ok(found)
}

/// golangci-lint, in JSON: the problems of the package, kept for the file.
pub fn golangci(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(&out.stdout)?;
    Ok(array(&report, "Issues")
        .filter(|i| {
            i.pointer("/Pos/Filename")
                .and_then(Value::as_str)
                .is_some_and(|f| same_file(f, file))
        })
        .map(|i| {
            let pos = i.get("Pos").cloned().unwrap_or_default();
            let severity = string(i, "Severity")
                .filter(|s| !s.is_empty())
                .map_or(Severity::Warning, |s| level(&s));
            problem(
                text,
                int(&pos, "Line").unwrap_or(1),
                int(&pos, "Column").unwrap_or(1),
                None,
                Unit::Bytes,
                severity,
                string(i, "Text").unwrap_or_default(),
                source,
                string(i, "FromLinter"),
            )
        })
        .collect())
}

/// Luacheck, `--formatter plain --codes --ranges`: `file:1:7-9: (W211) message`.
pub fn luacheck(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    Ok(out
        .stdout
        .lines()
        .filter_map(|line| {
            let skip = usize::from(line.as_bytes().get(1) == Some(&b':')) * 2;
            let mut parts = line[skip..].splitn(4, ':');
            let name = format!("{}{}", &line[..skip], parts.next()?);
            if !same_file(&name, file) {
                return None;
            }
            let row: usize = parts.next()?.trim().parse().ok()?;
            let (column, end) = parts.next()?.split_once('-')?;
            let (column, end): (usize, usize) =
                (column.trim().parse().ok()?, end.trim().parse().ok()?);
            let rest = parts.next()?.trim();
            let (code, message) = rest.strip_prefix('(')?.split_once(')')?;
            let severity = if code.starts_with('E') {
                Severity::Error
            } else {
                Severity::Warning
            };
            Some(problem(
                text,
                row,
                column,
                Some((row, end + 1)),
                Unit::Bytes,
                severity,
                message.trim(),
                source,
                Some(code.to_string()),
            ))
        })
        .collect())
}

/// ktlint, `--reporter=json`.
pub fn ktlint(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    if out.stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    let report = json(&out.stdout)?;
    let mut found = Vec::new();
    for file in report.as_array().into_iter().flatten() {
        for e in array(file, "errors") {
            found.push(problem(
                text,
                int(e, "line").unwrap_or(1),
                int(e, "column").unwrap_or(1),
                None,
                Unit::Chars,
                Severity::Warning,
                string(e, "message").unwrap_or_default(),
                source,
                string(e, "rule"),
            ));
        }
    }
    Ok(found)
}

/// SwiftLint, `--reporter json`.
pub fn swiftlint(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(&out.stdout)?;
    Ok(report
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            let line = int(v, "line").unwrap_or(1);
            let (column, end) = match int(v, "character") {
                Some(c) => (c, None),
                None => (1, Some((line, usize::MAX))),
            };
            problem(
                text,
                line,
                column,
                end,
                Unit::Chars,
                level(&string(v, "severity").unwrap_or_default()),
                string(v, "reason").unwrap_or_default(),
                source,
                string(v, "rule_id"),
            )
        })
        .collect())
}

/// HLint, `--json`: its hint, and what to write instead.
pub fn hlint(out: &Output, text: &Text, _: &Path, source: &str) -> Result<Vec<Diagnostic>, String> {
    let report = json(&out.stdout)?;
    Ok(report
        .as_array()
        .into_iter()
        .flatten()
        .filter(|h| string(h, "severity").as_deref() != Some("Ignore"))
        .map(|h| {
            let mut message = string(h, "hint").unwrap_or_default();
            if let Some(to) = string(h, "to").filter(|t| !t.is_empty()) {
                message = format!("{message}\n→ {to}");
            }
            let end = int(h, "endLine").zip(int(h, "endColumn"));
            problem(
                text,
                int(h, "startLine").unwrap_or(1),
                int(h, "startColumn").unwrap_or(1),
                end,
                Unit::Chars,
                level(&string(h, "severity").unwrap_or_default()),
                message,
                source,
                None,
            )
        })
        .collect())
}

/// SQLFluff, `--format json`.
pub fn sqlfluff(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(&out.stdout)?;
    let mut found = Vec::new();
    for file in report.as_array().into_iter().flatten() {
        for v in array(file, "violations") {
            let line = int(v, "start_line_no").or(int(v, "line_no")).unwrap_or(1);
            let column = int(v, "start_line_pos").or(int(v, "line_pos")).unwrap_or(1);
            let end = int(v, "end_line_no").zip(int(v, "end_line_pos"));
            let severity = if v.get("warning").and_then(Value::as_bool) == Some(true) {
                Severity::Warning
            } else {
                Severity::Error
            };
            found.push(problem(
                text,
                line,
                column,
                end,
                Unit::Chars,
                severity,
                string(v, "description").unwrap_or_default(),
                source,
                string(v, "code"),
            ));
        }
    }
    Ok(found)
}

/// actionlint, `-format '{{json .}}'`.
pub fn actionlint(
    out: &Output,
    text: &Text,
    _: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    if out.stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    let report = json(&out.stdout)?;
    Ok(report
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| {
            let line = int(e, "line").unwrap_or(1);
            let end = int(e, "end_column").map(|c| (line, c + 1));
            problem(
                text,
                line,
                int(e, "column").unwrap_or(1),
                end,
                Unit::Chars,
                Severity::Error,
                string(e, "message").unwrap_or_default(),
                source,
                string(e, "kind"),
            )
        })
        .collect())
}

/// Deno, `lint --json`: ranges in bytes when it gives them.
pub fn deno(out: &Output, text: &Text, _: &Path, source: &str) -> Result<Vec<Diagnostic>, String> {
    let report = json(&out.stdout)?;
    let mut found = Vec::new();
    for d in array(&report, "diagnostics") {
        let byte = |key: &str| {
            d.pointer(&format!("/range/{key}/bytePos"))
                .and_then(Value::as_u64)
                .map(|n| n as usize)
        };
        let mut message = string(d, "message").unwrap_or_default();
        if let Some(hint) = string(d, "hint").filter(|h| !h.is_empty()) {
            message = format!("{message}\n{hint}");
        }
        let code = string(d, "code");
        found.push(match (byte("start"), byte("end")) {
            (Some(start), Some(end)) => {
                problem_at_bytes(text, start..end, Severity::Warning, message, source, code)
            }
            _ => {
                let at = |key: &str| {
                    d.pointer(&format!("/range/{key}"))
                        .map(|p| (int(p, "line").unwrap_or(1), int(p, "col").unwrap_or(0) + 1))
                };
                let (line, column) = at("start").unwrap_or((1, 1));
                problem(
                    text,
                    line,
                    column,
                    at("end"),
                    Unit::Chars,
                    Severity::Warning,
                    message,
                    source,
                    code,
                )
            }
        });
    }
    for e in array(&report, "errors") {
        let message = string(e, "message").unwrap_or_default();
        found.push(problem(
            text,
            1,
            1,
            None,
            Unit::Chars,
            Severity::Error,
            message,
            source,
            None,
        ));
    }
    Ok(found)
}

/// `dart analyze --format=machine`: `SEVERITY|TYPE|CODE|file|line|column|length|message`.
pub fn dart(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    Ok(out
        .report()
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(8, '|').collect();
            let [severity, _, code, name, row, column, length, message] = parts.as_slice() else {
                return None;
            };
            if !same_file(name, file) {
                return None;
            }
            let (row, column, length): (usize, usize, usize) = (
                row.parse().ok()?,
                column.parse().ok()?,
                length.parse().ok()?,
            );
            Some(problem(
                text,
                row,
                column,
                Some((row, column + length)),
                Unit::Utf16,
                level(severity),
                message.replace("\\|", "|"),
                source,
                Some(code.to_ascii_lowercase()),
            ))
        })
        .collect())
}

/// Compilers and clang-tidy: `file:line:col: warning: message [check]`.
pub fn compiler(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    Ok(located_lines(&out.stdout, file)
        .filter_map(|(line, column, rest)| {
            let (kind, message) = rest.trim().split_once(':')?;
            if !matches!(kind.trim(), "error" | "warning" | "note" | "fatal error") {
                return None;
            }
            let message = message.trim();
            let (message, code) = match message.rsplit_once(" [") {
                Some((m, c)) if c.ends_with(']') => {
                    (m.to_string(), Some(c.trim_end_matches(']').to_string()))
                }
                _ => (message.to_string(), None),
            };
            let severity = if kind.contains("error") {
                Severity::Error
            } else {
                level(kind.trim())
            };
            Some(problem(
                text,
                line,
                column,
                None,
                Unit::Bytes,
                severity,
                message,
                source,
                code,
            ))
        })
        .collect())
}

/// TFLint, `--format json`: the problems of the module, kept for the file.
pub fn tflint(
    out: &Output,
    text: &Text,
    file: &Path,
    source: &str,
) -> Result<Vec<Diagnostic>, String> {
    let report = json(&out.stdout)?;
    Ok(array(&report, "issues")
        .filter(|i| {
            i.pointer("/range/filename")
                .and_then(Value::as_str)
                .is_some_and(|f| same_file(f, file))
        })
        .map(|i| {
            let at = |key: &str| {
                i.pointer(&format!("/range/{key}"))
                    .map(|p| (int(p, "line").unwrap_or(1), int(p, "column").unwrap_or(1)))
            };
            let (line, column) = at("start").unwrap_or((1, 1));
            let severity = i
                .pointer("/rule/severity")
                .and_then(Value::as_str)
                .map_or(Severity::Warning, level);
            let code = i
                .pointer("/rule/name")
                .and_then(Value::as_str)
                .map(str::to_string);
            problem(
                text,
                line,
                column,
                at("end"),
                Unit::Bytes,
                severity,
                string(i, "message").unwrap_or_default(),
                source,
                code,
            )
        })
        .collect())
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
