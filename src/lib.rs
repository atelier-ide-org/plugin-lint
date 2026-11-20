//! Lint, a native plugin of Atelier: the linters and formatters of a project, in the editor.
//!
//! ESLint, Oxlint, Biome, Stylelint, markdownlint, Ruff, Pylint, flake8, mypy, ShellCheck,
//! Hadolint and the others ([`catalog::TOOLS`]) check the documents as they are typed (or once
//! saved, for those that only read files), and their problems join the ones of the IDE.
//! Prettier and Oxfmt format them (Reformat Code). A tool runs where the project asks for it:
//! its configuration, or its program installed in the project ([`catalog::Activation`]).

#[macro_use]
mod i18n;
mod catalog;
mod linting;
mod parse;
mod run;
mod text;

use atelier_plugin_api::{FormatJob, Host, Plugin, Surface, egui, protocol, ui};
use catalog::{Input, Kind, Status, TOOLS, Tool};
use linting::{Linter, Outcome};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

/// The identifier of the tool window.
const TOOL_WINDOW: &str = "lint.window";
/// How long the window keeps where the tools stand before it looks again.
const LOOK_AGAIN: Duration = Duration::from_secs(3);

/// Where the tools of the active file stand, as the window shows them.
struct View {
    file: Option<PathBuf>,
    rows: Vec<(&'static Tool, Status)>,
    at: Instant,
}

#[derive(Default)]
pub struct LintPlugin {
    linter: Arc<Linter>,
    /// The tools turned off in the window, kept in the settings of the plugin.
    off: Vec<String>,
    loaded: bool,
    view: Option<View>,
}

atelier_plugin_api::export_plugin!(LintPlugin::default);

/// Every name of `kind`, once.
fn files_of(kind: Kind) -> Vec<String> {
    let mut names: Vec<String> = TOOLS
        .iter()
        .filter(|t| t.kind == kind)
        .flat_map(|t| t.files.iter().map(|f| f.to_string()))
        .collect();
    names.sort();
    names.dedup();
    names
}

fn settings_file(host: &Host) -> PathBuf {
    host.data_dir()
        .join("plugins")
        .join("lint")
        .join("settings.json")
}

impl LintPlugin {
    /// Reads the settings, the first time the plugin needs them.
    fn load(&mut self, host: &Host) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let json = std::fs::read_to_string(settings_file(host))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        self.off = json
            .as_ref()
            .and_then(|j| j.get("off"))
            .and_then(|o| o.as_array())
            .map(|o| {
                o.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        self.linter.set_off(self.off.clone());
    }

    fn save(&self, host: &Host) {
        let path = settings_file(host);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let json = serde_json::json!({ "off": self.off });
        if let Err(e) = std::fs::write(&path, format!("{json:#}\n")) {
            host.error(trf!("lint.cannot_save", e = e));
        }
        self.linter.set_off(self.off.clone());
    }

    /// Where the tools of the active file stand, looked at again when it changes.
    fn refresh_view(&mut self, host: &Host) {
        let project = host.project();
        let file = project.active_file;
        let fresh = self
            .view
            .as_ref()
            .is_some_and(|v| v.file == file && v.at.elapsed() < LOOK_AGAIN);
        if fresh {
            return;
        }
        let rows = file
            .as_ref()
            .map(|file| {
                TOOLS
                    .iter()
                    .filter(|t| t.reads(file))
                    .map(|t| (t, catalog::status(t, file, project.root.as_deref())))
                    .collect()
            })
            .unwrap_or_default();
        self.view = Some(View {
            file,
            rows,
            at: Instant::now(),
        });
    }

    fn window(&mut self, ui: &mut egui::Ui, host: &Host) {
        self.load(host);
        self.refresh_view(host);
        let t = ui::theme(ui.ctx());
        let Some(view) = &self.view else {
            return;
        };
        let reports = self.linter.reports();
        let mut toggled = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            let Some(file) = &view.file else {
                ui.label(egui::RichText::new(tr!("lint.no_file")).color(t.text_secondary));
                return;
            };
            ui::section(ui, tr!("lint.active_file"));
            ui.label(file_name(file));
            ui.add_space(8.0);
            if view.rows.is_empty() {
                ui.label(egui::RichText::new(tr!("lint.no_tools")).color(t.text_secondary));
            }
            for kind in [Kind::Lint, Kind::Format] {
                let rows: Vec<_> = view
                    .rows
                    .iter()
                    .filter(|(tool, _)| tool.kind == kind)
                    .collect();
                if rows.is_empty() {
                    continue;
                }
                ui::section(
                    ui,
                    if kind == Kind::Lint {
                        tr!("lint.linters")
                    } else {
                        tr!("lint.formatters")
                    },
                );
                for (tool, status) in rows {
                    let mut on = !self.off.iter().any(|o| o == tool.id);
                    ui.horizontal(|ui| {
                        if ui::checkbox(ui, &mut on, tool.name).changed() {
                            toggled = Some((tool.id, on));
                        }
                        if kind == Kind::Lint {
                            let when = if tool.input == Input::Stdin {
                                tr!("lint.live")
                            } else {
                                tr!("lint.on_save")
                            };
                            ui.label(egui::RichText::new(when).small().color(t.text_disabled));
                        }
                    });
                    let (line, color) = match status {
                        _ if !on => (tr!("lint.turned_off").to_string(), t.text_disabled),
                        Status::Ready {
                            config, program, ..
                        } => {
                            let mut line = match config {
                                Some(config) => {
                                    trf!("lint.ready_config", config = file_name(config))
                                }
                                None if program.local => tr!("lint.ready_installed").to_string(),
                                None => tr!("lint.ready").to_string(),
                            };
                            match reports.get(tool.id) {
                                Some(report) if &report.file == file => match &report.outcome {
                                    Outcome::Found(count) => {
                                        line = format!(
                                            "{line} · {}",
                                            trf!(
                                                "lint.found",
                                                count = count,
                                                ms = report.took.as_millis()
                                            )
                                        );
                                    }
                                    Outcome::Failed(reason) => {
                                        line = trf!("lint.failed", reason = reason);
                                    }
                                },
                                _ => {}
                            }
                            let failed = reports.get(tool.id).is_some_and(|r| {
                                &r.file == file && matches!(r.outcome, Outcome::Failed(_))
                            });
                            (line, if failed { t.error } else { t.text_secondary })
                        }
                        Status::Missing {
                            config: Some(config),
                        } => (
                            trf!(
                                "lint.missing_config",
                                config = file_name(config),
                                program = tool.program
                            ),
                            t.warning,
                        ),
                        Status::Missing { config: None } => {
                            (trf!("lint.missing", program = tool.program), t.warning)
                        }
                        Status::Off => (tr!("lint.off").to_string(), t.text_disabled),
                    };
                    ui.indent(tool.id, |ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(line).small().color(color)).wrap(),
                        );
                    });
                    ui.add_space(4.0);
                }
            }
            ui.add_space(8.0);
            ui.collapsing(tr!("lint.known"), |ui| {
                for kind in [Kind::Lint, Kind::Format] {
                    let names: Vec<&str> = TOOLS
                        .iter()
                        .filter(|t| t.kind == kind)
                        .map(|t| t.name)
                        .collect();
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(names.join(", "))
                                .small()
                                .color(t.text_secondary),
                        )
                        .wrap(),
                    );
                    ui.add_space(4.0);
                }
            });
        });
        if let Some((id, on)) = toggled {
            self.off.retain(|o| o != id);
            if !on {
                self.off.push(id.to_string());
            }
            self.save(host);
            self.view = None;
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl Plugin for LintPlugin {
    fn info(&mut self, language: &str) -> protocol::Info {
        i18n::set_language(language);
        protocol::Info {
            tool_windows: vec![protocol::ToolWindow {
                id: TOOL_WINDOW.into(),
                title: tr!("lint.title").into(),
                icon: "problems".into(),
                dock: protocol::Dock::Right,
            }],
            commands: vec![protocol::Command {
                id: "lint.open".into(),
                label: tr!("lint.open").into(),
            }],
            lints: files_of(Kind::Lint),
            formats: files_of(Kind::Format),
            ..Default::default()
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, surface: Surface<'_>, host: &Host) {
        i18n::set_language(&ui::language(ui.ctx()));
        if surface == Surface::ToolWindow(TOOL_WINDOW) {
            self.window(ui, host);
            // Reports come from the background.
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
    }

    fn run_command(&mut self, id: &str, host: &Host) {
        if id == "lint.open" {
            host.show_tool_window(TOOL_WINDOW);
        }
    }

    fn file_closed(&mut self, path: &Path) {
        self.linter.closed(path);
    }

    fn document(&mut self, document: &protocol::Document, host: &Host) {
        self.load(host);
        self.linter.document(document.clone(), host);
    }

    fn format(&mut self, request: &protocol::FormatRequest, host: &Host) -> Option<FormatJob> {
        self.load(host);
        let root = host.project().root;
        let (tool, status) = catalog::formatter(&request.path, root.as_deref(), &self.off)?;
        let (path, text) = (request.path.clone(), request.text.clone());
        Some(Box::new(move || {
            match linting::format(tool, &status, &path, &text) {
                Ok(text) => protocol::Formatted::Text(text),
                Err(e) => protocol::Formatted::Error(e),
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_lints_and_formats_the_files_of_its_tools() {
        let info = LintPlugin::default().info("en_US");
        assert_eq!(info.tool_windows[0].id, TOOL_WINDOW);
        assert_eq!(info.commands[0].id, "lint.open");
        let covers = |names: &[String], path: &str| protocol::Info::covers(names, Path::new(path));
        for file in [
            "a.ts",
            "a.vue",
            "a.py",
            "a.css",
            "a.md",
            "Dockerfile",
            "a.sh",
            "a.go",
        ] {
            assert!(covers(&info.lints, file), "{file}");
        }
        for file in ["a.ts", "a.json", "a.md", "a.yaml", "Cargo.toml"] {
            assert!(covers(&info.formats, file), "{file}");
        }
        assert!(!covers(&info.formats, "a.py"));
    }

    #[test]
    fn every_tool_has_its_texts_and_a_reader_when_it_lints() {
        let mut ids: Vec<&str> = TOOLS.iter().map(|t| t.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), TOOLS.len(), "two tools share an identifier");
        for tool in TOOLS {
            assert_eq!(tool.kind == Kind::Lint, tool.read.is_some(), "{}", tool.id);
            assert!(!tool.files.is_empty(), "{}", tool.id);
        }
        for language in ["en_US", "fr_FR"] {
            i18n::set_language(language);
            for key in [
                "lint.title",
                "lint.open",
                "lint.active_file",
                "lint.no_file",
                "lint.no_tools",
                "lint.linters",
                "lint.formatters",
                "lint.live",
                "lint.on_save",
                "lint.turned_off",
                "lint.ready",
                "lint.ready_config",
                "lint.ready_installed",
                "lint.found",
                "lint.failed",
                "lint.missing",
                "lint.missing_config",
                "lint.off",
                "lint.known",
                "lint.cannot_save",
            ] {
                assert_ne!(i18n::text(key), key, "{language}: {key}");
            }
        }
    }
}
