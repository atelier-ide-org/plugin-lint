//! The readers, on reports the tools wrote (ESLint 9, Oxlint 1.94, Biome 2.6, Stylelint 16,
//! markdownlint-cli 0.4x, Ruff, Pylint 3, flake8 7, mypy 1, yamllint 1), or as their
//! documentation shows them.

use super::*;
use std::path::PathBuf;

fn out(stdout: &str) -> Output {
    Output {
        stdout: stdout.into(),
        stderr: String::new(),
        code: Some(1),
    }
}

fn err(stderr: &str) -> Output {
    Output {
        stdout: String::new(),
        stderr: stderr.into(),
        code: Some(2),
    }
}

/// Each problem as `line:column-end_line:end_column severity code message`, 0-based UTF-16.
fn lines(found: Vec<Diagnostic>) -> Vec<String> {
    found
        .iter()
        .map(|d| {
            format!(
                "{}:{}-{}:{} {:?} {} {}",
                d.line,
                d.column,
                d.end_line,
                d.end_column,
                d.severity,
                d.code.as_deref().unwrap_or("-"),
                d.message
            )
        })
        .collect()
}

fn file(name: &str) -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        "C:\\p\\src"
    } else {
        "/p/src"
    })
    .join(name)
}

#[test]
fn eslint_counts_utf16_and_leaves_out_ignored_files() {
    let source = "const é = 1;\nlet unused = 2\nfoo();\n";
    let report = r#"[{"filePath":"C:\\p\\src\\a.js","messages":[
        {"ruleId":"no-unused-vars","severity":2,"message":"'é' is assigned a value but never used.","line":1,"column":7,"endLine":1,"endColumn":8},
        {"ruleId":"no-undef","severity":1,"message":"'foo' is not defined.","line":3,"column":1,"endLine":3,"endColumn":4},
        {"ruleId":null,"severity":1,"message":"File ignored because no matching configuration was supplied."}
    ]}]"#;
    let found = eslint(&out(report), &Text::new(source), &file("a.js"), "eslint").unwrap();
    assert_eq!(
        lines(found),
        [
            "0:6-0:7 Error no-unused-vars 'é' is assigned a value but never used.",
            "2:0-2:3 Warning no-undef 'foo' is not defined.",
        ]
    );
}

#[test]
fn oxlint_places_byte_spans() {
    let source = "const é = 1;\nlet unused = 2\n";
    let report = r#"{ "diagnostics": [{"message": "Variable 'é' is declared but never used.","code": "eslint(no-unused-vars)","severity": "warning","help": "Consider removing this declaration.","filename": "src/a.js","labels": [{"label": "here","span": {"offset": 6,"length": 2,"line": 1,"column": 7}}]},
        {"message": "x","code": "eslint(no-unused-vars)","severity": "error","filename": "src/other.js","labels": [{"span": {"offset": 0,"length": 1}}]}],
        "number_of_files": 2 }"#;
    let found = oxlint(&out(report), &Text::new(source), &file("a.js"), "oxlint").unwrap();
    assert_eq!(
        lines(found),
        [
            "0:6-0:7 Warning eslint(no-unused-vars) Variable 'é' is declared but never used.\nConsider removing this declaration."
        ]
    );
}

#[test]
fn biome_reads_its_json_among_other_lines() {
    let source = "let a = 1;\ndebugger;\n";
    let report = "The `json` and `json-pretty` reporters are experimental and may change in patch releases.\n{\"summary\":{\"errors\":1},\"diagnostics\":[{\"severity\":\"error\",\"message\":\"This is an unexpected use of the debugger statement.\",\"category\":\"lint/suspicious/noDebugger\",\"location\":{\"path\":\"src/a.js\",\"start\":{\"line\":2,\"column\":1},\"end\":{\"line\":2,\"column\":10}},\"advices\":[]}],\"command\":\"lint\"}\n";
    let found = biome(&out(report), &Text::new(source), &file("a.js"), "biome").unwrap();
    assert_eq!(
        lines(found),
        [
            "1:0-1:9 Error lint/suspicious/noDebugger This is an unexpected use of the debugger statement."
        ]
    );
}

#[test]
fn stylelint_reports_on_its_errors_without_the_rule_in_the_text() {
    let report = r#"[{"source":"C:\\p\\src\\a.css","warnings":[{"line":1,"column":5,"endLine":1,"endColumn":9,"rule":"property-no-unknown","severity":"error","text":"Unknown property \"colr\" (property-no-unknown)"}]}]"#;
    let found = stylelint(
        &err(report),
        &Text::new("a { colr: red; }\n"),
        &file("a.css"),
        "stylelint",
    )
    .unwrap();
    assert_eq!(
        lines(found),
        ["0:4-0:8 Error property-no-unknown Unknown property \"colr\""]
    );
    let bad = r#"[{"warnings":[],"invalidOptionWarnings":[{"text":"Invalid option value \"x\" for rule \"y\""}]}]"#;
    assert!(stylelint(&err(bad), &Text::new(""), &file("a.css"), "stylelint").is_err());
}

#[test]
fn markdownlint_underlines_its_range_or_the_line() {
    let source = "#Title\nSome  text\n* a\n";
    let report = r#"[
      {"fileName":"stdin","lineNumber":1,"ruleNames":["MD018","no-missing-space-atx"],"ruleDescription":"No space after hash on atx style heading","errorDetail":null,"errorRange":[1,2],"severity":"error"},
      {"fileName":"stdin","lineNumber":3,"ruleNames":["MD032","blanks-around-lists"],"ruleDescription":"Lists should be surrounded by blank lines","errorDetail":null,"errorRange":null,"severity":"error"}
    ]"#;
    let found = markdownlint(
        &err(report),
        &Text::new(source),
        &file("a.md"),
        "markdownlint",
    )
    .unwrap();
    assert_eq!(
        lines(found),
        [
            "0:0-0:2 Warning MD018 No space after hash on atx style heading",
            "2:0-2:3 Warning MD032 Lists should be surrounded by blank lines",
        ]
    );
    assert!(
        markdownlint(&err(""), &Text::new(source), &file("a.md"), "markdownlint")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn ruff_counts_characters() {
    let source = "x = \"é\"; import os\n";
    let report = r#"[{"code":"F401","end_location":{"column":19,"row":1},"location":{"column":17,"row":1},"message":"`os` imported but unused"},
        {"code":null,"end_location":{"column":2,"row":1},"location":{"column":1,"row":1},"message":"SyntaxError: x"}]"#;
    let found = ruff(&out(report), &Text::new(source), &file("a.py"), "ruff").unwrap();
    assert_eq!(
        lines(found),
        [
            "0:16-0:18 Warning F401 `os` imported but unused",
            "0:0-0:1 Error - SyntaxError: x",
        ]
    );
}

#[test]
fn pylint_flake8_and_mypy() {
    let source = "import os\nx = \"é\"; y: int = \"s\"\n";
    let text = Text::new(source);
    let report = r#"[{"type":"convention","line":1,"column":0,"endLine":null,"endColumn":null,"symbol":"missing-module-docstring","message":"Missing module docstring"},
        {"type":"warning","line":1,"column":0,"endLine":1,"endColumn":9,"symbol":"unused-import","message":"Unused import os"}]"#;
    assert_eq!(
        lines(pylint(&out(report), &text, &file("a.py"), "pylint").unwrap()),
        [
            "0:0-0:6 Info missing-module-docstring Missing module docstring",
            "0:0-0:9 Warning unused-import Unused import os",
        ]
    );
    let report =
        "1:1:F401:'os' imported but unused\n2:8:E702:multiple statements on one line (semicolon)\n";
    assert_eq!(
        lines(flake8(&out(report), &text, &file("a.py"), "flake8").unwrap()),
        [
            "0:0-0:6 Warning F401 'os' imported but unused",
            "1:7-1:8 Warning E702 multiple statements on one line (semicolon)",
        ]
    );
    let name = file("a.py").display().to_string();
    let report = format!(
        "{name}:2:18:2:20: error: Incompatible types in assignment  [assignment]\nother.py:1:1:1:2: error: elsewhere  [x]\n"
    );
    assert_eq!(
        lines(mypy(&out(&report), &text, &file("a.py"), "mypy").unwrap()),
        ["1:17-1:20 Error assignment Incompatible types in assignment"]
    );
}

#[test]
fn yamllint_reads_parsable_lines() {
    let report = "stdin:1:5: [error] too many spaces after colon (colons)\nstdin:1:1: [warning] missing document start \"---\" (document-start)\n";
    assert_eq!(
        lines(
            yamllint(
                &out(report),
                &Text::new("a:   1\n"),
                &file("a.yaml"),
                "yamllint"
            )
            .unwrap()
        ),
        [
            "0:4-0:5 Error colons too many spaces after colon",
            "0:0-0:1 Warning document-start missing document start \"---\"",
        ]
    );
}

#[test]
fn shell_docker_and_workflow_linters() {
    let text = Text::new("echo $1\nFROM node\n");
    let report = r#"{"comments":[{"file":"-","line":1,"endLine":1,"column":6,"endColumn":8,"level":"info","code":2086,"message":"Double quote to prevent globbing and word splitting."}]}"#;
    assert_eq!(
        lines(shellcheck(&out(report), &text, &file("a.sh"), "shellcheck").unwrap()),
        ["0:5-0:7 Info SC2086 Double quote to prevent globbing and word splitting."]
    );
    let report = r#"[{"code":"DL3007","column":1,"file":"-","level":"warning","line":2,"message":"Using latest is prone to errors"}]"#;
    assert_eq!(
        lines(hadolint(&out(report), &text, &file("Dockerfile"), "hadolint").unwrap()),
        ["1:0-1:9 Warning DL3007 Using latest is prone to errors"]
    );
    let report = r#"[{"message":"property \"runs-on\" is missing","filepath":"<stdin>","line":1,"column":3,"kind":"syntax-check","end_column":4}]"#;
    assert_eq!(
        lines(actionlint(&out(report), &text, &file("ci.yml"), "actionlint").unwrap()),
        ["0:2-0:4 Error syntax-check property \"runs-on\" is missing"]
    );
}

#[test]
fn ruby_php_go_and_lua_linters() {
    let text = Text::new("puts 'a'\n$x = 1;\nfunc main() {}\nlocal x = 1\n");
    let report = r#"{"files":[{"path":"a.rb","offenses":[{"severity":"convention","message":"Style/StringLiterals: Prefer double quotes.","cop_name":"Style/StringLiterals","location":{"start_line":1,"start_column":6,"last_line":1,"last_column":8}}]}]}"#;
    assert_eq!(
        lines(rubocop(&out(report), &text, &file("a.rb"), "rubocop").unwrap()),
        ["0:5-0:8 Info Style/StringLiterals Prefer double quotes."]
    );
    let report = r#"{"totals":{},"files":{"STDIN":{"messages":[{"message":"Missing doc comment","source":"PEAR.Commenting.FileComment.Missing","severity":5,"type":"ERROR","line":2,"column":1}]}}}"#;
    assert_eq!(
        lines(phpcs(&out(report), &text, &file("a.php"), "phpcs").unwrap()),
        ["1:0-1:2 Error PEAR.Commenting.FileComment.Missing Missing doc comment"]
    );
    let name = file("main.go").display().to_string().replace('\\', "\\\\");
    let report = format!(
        r#"{{"Issues":[{{"FromLinter":"unused","Text":"func main is unused","Severity":"","Pos":{{"Filename":"{name}","Line":3,"Column":6}}}},{{"FromLinter":"x","Text":"y","Pos":{{"Filename":"other.go","Line":1,"Column":1}}}}]}}"#
    );
    assert_eq!(
        lines(golangci(&out(&report), &text, &file("main.go"), "golangci-lint").unwrap()),
        ["2:5-2:9 Warning unused func main is unused"]
    );
    let name = file("a.lua").display().to_string();
    let report = format!("{name}:4:7-7: (W211) unused variable 'x'\n");
    assert_eq!(
        lines(luacheck(&out(&report), &text, &file("a.lua"), "luacheck").unwrap()),
        ["3:6-3:7 Warning W211 unused variable 'x'"]
    );
}

#[test]
fn compilers_report_file_line_column() {
    let name = file("a.c").display().to_string();
    let report = format!(
        "{name}:2:5: warning: unused variable 'x' [clang-diagnostic-unused-variable]\n{name}:1:1: note: here\nother.h:1:1: error: elsewhere\n"
    );
    assert_eq!(
        lines(
            compiler(
                &out(&report),
                &Text::new("int main() {\n    int x;\n"),
                &file("a.c"),
                "clang-tidy"
            )
            .unwrap()
        ),
        [
            "1:4-1:7 Warning clang-diagnostic-unused-variable unused variable 'x'",
            "0:0-0:3 Info - here",
        ]
    );
}

#[test]
fn deno_and_dart() {
    let text = Text::new("let é = 1;\n");
    let report = r#"{"diagnostics":[{"range":{"start":{"line":1,"col":4,"bytePos":4},"end":{"line":1,"col":5,"bytePos":6}},"filename":"file:///p/src/a.ts","message":"`é` is never used","code":"no-unused-vars","hint":"If this is intentional, prefix it with an underscore"}],"errors":[]}"#;
    assert_eq!(
        lines(deno(&out(report), &text, &file("a.ts"), "deno").unwrap()),
        [
            "0:4-0:5 Warning no-unused-vars `é` is never used\nIf this is intentional, prefix it with an underscore"
        ]
    );
    let name = file("a.dart").display().to_string();
    let report =
        format!("INFO|LINT|PREFER_FINAL_LOCALS|{name}|1|5|1|Local variables should be final.\n");
    assert_eq!(
        lines(dart(&err(&report), &text, &file("a.dart"), "dart").unwrap()),
        ["0:4-0:5 Info prefer_final_locals Local variables should be final."]
    );
}

#[test]
fn a_report_that_is_no_json_is_a_failure_of_the_tool() {
    let crashed =
        err("Oops! Something went wrong! :(\n\nESLint: 9.0.0\n\nError: Cannot find module 'x'\n");
    let e = eslint(&crashed, &Text::new(""), &file("a.js"), "eslint").unwrap_err();
    assert!(e.starts_with("Oops! Something went wrong!"), "{e}");
}
