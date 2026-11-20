//! Finding the programs of the tools, and running them: without a console window, with the text
//! on their input, and stopped when they take too long.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Where a program of a tool is looked for, before the `PATH`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// `node_modules/.bin`, from the folder of the file up.
    Node,
    /// The virtual environment of the project (`.venv`, `venv`, `env`, or `VIRTUAL_ENV`).
    Python,
    /// `vendor/bin`, where Composer puts them.
    Php,
    /// The `PATH` only.
    System,
}

/// A program found, and whether it belongs to the project (installed in it) rather than to the
/// system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub path: PathBuf,
    pub local: bool,
}

/// The names a program has on disk: `eslint.cmd`, `ruff.exe`… on Windows.
fn file_names(name: &str) -> Vec<String> {
    if cfg!(windows) {
        ["exe", "cmd", "bat"]
            .iter()
            .map(|e| format!("{name}.{e}"))
            .chain([name.to_string()])
            .collect()
    } else {
        vec![name.to_string()]
    }
}

fn in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    file_names(name)
        .into_iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
}

/// The program `name` for a file of `dir`: the project's, else the system's.
pub fn find(name: &str, place: Place, dir: &Path, root: Option<&Path>) -> Option<Found> {
    let local = match place {
        Place::Node => dir
            .ancestors()
            .find_map(|d| in_dir(&d.join("node_modules").join(".bin"), name)),
        Place::Php => dir
            .ancestors()
            .find_map(|d| in_dir(&d.join("vendor").join("bin"), name)),
        Place::Python => {
            let bin = if cfg!(windows) { "Scripts" } else { "bin" };
            let from_env = std::env::var_os("VIRTUAL_ENV").map(PathBuf::from);
            let project = dir
                .ancestors()
                .take_while(|d| root.is_none_or(|r| d.starts_with(r)))
                .flat_map(|d| [".venv", "venv", "env"].map(|v| d.join(v)))
                .collect::<Vec<_>>();
            from_env
                .into_iter()
                .chain(project)
                .find_map(|venv| in_dir(&venv.join(bin), name))
        }
        Place::System => None,
    };
    if let Some(path) = local {
        return Some(Found { path, local: true });
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .find_map(|d| in_dir(&d, name))
        .map(|path| Found { path, local: false })
}

/// What a program wrote, and how it ended.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    /// None when it was stopped, or killed by a signal.
    pub code: Option<i32>,
}

impl Output {
    /// What it wrote, on its output, else on its errors (stylelint and markdownlint write their
    /// report there).
    pub fn report(&self) -> &str {
        if self.stdout.trim().is_empty() {
            &self.stderr
        } else {
            &self.stdout
        }
    }
}

/// Runs `program` with `args` in `dir`, `input` on its standard input, for at most `timeout`.
pub fn run(
    program: &Path,
    args: &[String],
    dir: &Path,
    input: Option<&str>,
    timeout: Duration,
) -> Result<Output, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(dir)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Reports without colors.
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("{}: {e}", program.display()))?;
    // Written and read on threads: a program may fill its output before it has read its input.
    let writer = input.map(|text| {
        let mut stdin = child.stdin.take();
        let text = text.to_string();
        std::thread::spawn(move || {
            if let Some(stdin) = stdin.as_mut() {
                let _ = stdin.write_all(text.as_bytes());
            }
        })
    });
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            String::from_utf8_lossy(&bytes).into_owned()
        })
    };
    let stdout = read(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = read(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let started = Instant::now();
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{} took more than {} s",
                    program.display(),
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(e.to_string()),
        }
    };
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    Ok(Output {
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
        code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_of_the_project_come_before_the_system() {
        let dir = std::env::temp_dir().join(format!("lint-find-{}", std::process::id()));
        let bin = dir.join("node_modules").join(".bin");
        std::fs::create_dir_all(bin.join("x")).unwrap();
        let name = if cfg!(windows) { "tool.cmd" } else { "tool" };
        std::fs::write(bin.join(name), "").unwrap();
        let found = find("tool", Place::Node, &dir.join("src/deep"), Some(&dir)).unwrap();
        assert_eq!(found.path, bin.join(name));
        assert!(found.local);
        assert_eq!(find("tool", Place::Php, &dir, Some(&dir)), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_program_reads_its_input_and_its_output_comes_back() {
        let (program, args): (&str, Vec<String>) = if cfg!(windows) {
            ("cmd", vec!["/C".into(), "more".into()])
        } else {
            ("cat", vec![])
        };
        let Some(found) = find(program, Place::System, Path::new("."), None) else {
            return;
        };
        let output = run(
            &found.path,
            &args,
            Path::new("."),
            Some("hello"),
            Duration::from_secs(10),
        )
        .unwrap();
        assert!(output.stdout.contains("hello"));
        assert_eq!(output.code, Some(0));
    }
}
