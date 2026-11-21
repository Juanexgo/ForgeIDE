use eframe::egui;
use portable_pty::CommandBuilder;
use serde::Deserialize;
use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
pub struct Task {
    pub label: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Deserialize)]
struct TaskFile {
    version: u32,
    tasks: Vec<Task>,
}

#[derive(Default)]
pub struct Tasks {
    tasks: Vec<Task>,
    notice: String,
}

impl Task {
    pub fn command(&self, root: &Path) -> io::Result<(CommandBuilder, PathBuf)> {
        let cwd = self
            .cwd
            .as_ref()
            .map_or_else(|| root.to_owned(), |cwd| root.join(cwd))
            .canonicalize()?;
        let executable = locate(&self.program, root).ok_or_else(|| {
            io::Error::other(format!(
                "{} is missing. Install it or use an absolute program path in .forge/tasks.json.",
                self.program
            ))
        })?;
        let mut command = CommandBuilder::new(executable);
        command.args(&self.args);
        command.env("PATH", environment_path());
        Ok((command, cwd))
    }
}

impl Tasks {
    pub fn show(&mut self, ui: &mut egui::Ui, root: &Path) -> Option<usize> {
        ui.label("Project tasks are loaded from configuration, or detected from supported project manifests. They run only when selected.");
        if ui.button("Load project tasks").clicked() {
            match load(root) {
                Ok(tasks) => {
                    self.tasks = tasks;
                    self.notice.clear();
                }
                Err(error) => self.notice = error.to_string(),
            }
        }
        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
        let mut selected = None;
        for (index, task) in self.tasks.iter().enumerate() {
            let available = locate(&task.program, root).is_some();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(available, egui::Button::new(&task.label))
                    .clicked()
                {
                    selected = Some(index);
                }
                if !available {
                    ui.label(format!("Missing tool — {}", task.program));
                }
            });
        }
        selected
    }

    pub fn launch(
        &self,
        index: usize,
        root: &Path,
        context: &egui::Context,
    ) -> Result<crate::terminal::Terminal, Box<dyn std::error::Error>> {
        let task = self
            .tasks
            .get(index)
            .ok_or_else(|| io::Error::other("Task no longer exists."))?;
        let (command, cwd) = task.command(root)?;
        crate::terminal::Terminal::spawn(&cwd, context, command)
    }
}

pub(crate) fn load(root: &Path) -> io::Result<Vec<Task>> {
    let path = root.join(".forge/tasks.json");
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return detected_tasks(root),
        Err(error) => return Err(error),
    };
    if file.metadata()?.len() > 256 * 1024 {
        return Err(io::Error::other("Task configuration exceeds 256 KiB."));
    }
    let configuration: TaskFile = serde_json::from_reader(file)?;
    if configuration.version != 1 || configuration.tasks.len() > 100 {
        return Err(io::Error::other(
            "Use task schema version 1 with up to 100 tasks.",
        ));
    }
    for task in &configuration.tasks {
        if task
            .kind
            .as_deref()
            .is_some_and(|kind| !matches!(kind, "test" | "build" | "run"))
        {
            return Err(io::Error::other("Task kind must be test, build or run."));
        }
        if task.label.trim().is_empty() || task.program.trim().is_empty() || task.args.len() > 256 {
            return Err(io::Error::other(
                "Each task needs a label, program and up to 256 arguments.",
            ));
        }
    }
    Ok(configuration.tasks)
}

pub fn environment_path() -> std::ffi::OsString {
    let mut directories: Vec<_> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        directories.push(home.join(".cargo/bin"));
        directories.push(home.join(".local/bin"));
    }
    if cfg!(target_os = "macos") {
        directories.extend(
            ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
                .into_iter()
                .map(PathBuf::from),
        );
    }
    if cfg!(windows) {
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            let program_files = PathBuf::from(program_files);
            directories.push(program_files.join("nodejs"));
            directories.push(program_files.join("Git/cmd"));
        }
    }
    std::env::join_paths(directories.into_iter().filter(|path| path.is_absolute()))
        .unwrap_or_else(|_| std::env::var_os("PATH").unwrap_or_default())
}

pub fn locate(program: &str, root: &Path) -> Option<PathBuf> {
    let path = Path::new(program);
    let names = if cfg!(windows) && path.extension().is_none() {
        vec![
            program.to_owned(),
            format!("{program}.exe"),
            format!("{program}.cmd"),
        ]
    } else {
        vec![program.to_owned()]
    };
    if path.components().count() > 1 || path.is_absolute() {
        return executable(&root.join(path));
    }
    std::env::split_paths(&environment_path())
        .filter(|path| path.is_absolute())
        .find_map(|directory| {
            names
                .iter()
                .find_map(|name| executable(&directory.join(name)))
        })
}

fn executable(path: &Path) -> Option<PathBuf> {
    let metadata = path.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    path.canonicalize().ok()?;
    std::path::absolute(path).ok()
}

pub fn detected_tasks(root: &Path) -> io::Result<Vec<Task>> {
    let task = |label: &str, program: &str, args: &[&str], kind: &str| Task {
        label: label.into(),
        program: program.into(),
        args: args.iter().map(|value| (*value).into()).collect(),
        cwd: None,
        kind: Some(kind.into()),
    };
    let mut tasks = Vec::new();
    if root.join("Cargo.toml").is_file() {
        tasks.push(task("Cargo: test", "cargo", &["test"], "test"));
        tasks.push(task("Cargo: build", "cargo", &["build"], "build"));
    }
    if root.join("go.mod").is_file() {
        tasks.push(task("Go: test packages", "go", &["test", "./..."], "test"));
        tasks.push(task(
            "Go: build packages",
            "go",
            &["build", "./..."],
            "build",
        ));
    }
    if root.join("pytest.ini").is_file() || root.join("conftest.py").is_file() {
        tasks.push(task("Python: pytest", "python3", &["-m", "pytest"], "test"));
    }
    let manifest = root.join("package.json");
    if manifest.is_file() {
        let value: serde_json::Value =
            serde_json::from_slice(&crate::packages::read_bounded(&manifest, 256 * 1024)?)?;
        for (script, kind) in [("test", "test"), ("build", "build"), ("dev", "run")] {
            if value["scripts"][script].is_string() {
                tasks.push(Task {
                    label: format!("npm: {script}"),
                    program: "npm".into(),
                    args: vec!["run".into(), script.into()],
                    cwd: None,
                    kind: Some(kind.into()),
                });
            }
        }
    }
    Ok(tasks)
}

pub struct Exclusions {
    patterns: regex::RegexSet,
}

impl Exclusions {
    pub fn load(root: &Path) -> io::Result<Self> {
        let file = match std::fs::File::open(root.join(".forge/excludes")) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self {
                    patterns: regex::RegexSet::new([
                        "(?:^|/)(?:[.]git|node_modules|target)(?:/|$)",
                    ])
                    .map_err(io::Error::other)?,
                });
            }
            Err(error) => return Err(error),
        };
        if file.metadata()?.len() > 64 * 1024 {
            return Err(io::Error::other(".forge/excludes exceeds 64 KiB."));
        }
        use std::io::Read;
        let mut text = String::new();
        file.take(64 * 1024).read_to_string(&mut text)?;
        let mut patterns = vec!["(?:^|/)(?:[.]git|node_modules|target)(?:/|$)".to_owned()];
        for pattern in text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            if patterns.len() > 256 || pattern.starts_with('!') {
                return Err(io::Error::other(
                    "Use up to 256 exclusion globs. Negated patterns are unsupported.",
                ));
            }
            let pattern = pattern.trim_start_matches('/').trim_end_matches('/');
            let mut expression = if pattern.contains('/') {
                "^".to_owned()
            } else {
                "(?:^|/)".to_owned()
            };
            let mut characters = pattern.chars().peekable();
            while let Some(character) = characters.next() {
                match character {
                    '*' if characters.peek() == Some(&'*') => {
                        characters.next();
                        if characters.peek() == Some(&'/') {
                            characters.next();
                            expression.push_str("(?:.*/)?");
                        } else {
                            expression.push_str(".*");
                        }
                    }
                    '*' => expression.push_str("[^/]*"),
                    '?' => expression.push_str("[^/]"),
                    _ => expression.push_str(&regex::escape(&character.to_string())),
                }
            }
            expression.push_str("(?:/.*)?$");
            patterns.push(expression);
        }
        Ok(Self {
            patterns: regex::RegexSet::new(patterns).map_err(io::Error::other)?,
        })
    }

    pub fn matches(&self, root: &Path, path: &Path) -> bool {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        self.patterns.is_match(&relative)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_exclude_nested_outputs_and_leave_similar_paths_visible() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let root = folder.path();
        std::fs::create_dir(root.join(".forge"))?;
        std::fs::write(
            root.join(".forge/excludes"),
            "# project outputs\nbuild/\n**/*.generated.ts\nsrc/**/cache/\n",
        )?;
        let exclusions = Exclusions::load(root)?;
        for path in [
            "build/file.rs",
            "src/x.generated.ts",
            "src/a/b/cache/result",
            "node_modules/pkg/main.js",
        ] {
            assert!(exclusions.matches(root, &root.join(path)), "{path}");
        }
        for path in ["builder/file.rs", "src/main.ts", "src/cacheable/result"] {
            assert!(!exclusions.matches(root, &root.join(path)), "{path}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod detected_tests {
    use super::*;
    #[test]
    fn detects_supported_tasks_without_writing_configuration_and_respects_explicit_profiles()
    -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        std::fs::write(root.join("Cargo.toml"), "[package]")?;
        std::fs::write(
            root.join("package.json"),
            r#"{"scripts":{"test":"vitest","dev":"astro dev"}}"#,
        )?;
        let tasks = load(root)?;
        assert!(
            tasks
                .iter()
                .any(|task| task.program == "cargo" && task.kind.as_deref() == Some("test"))
        );
        assert!(
            tasks
                .iter()
                .any(|task| task.program == "npm" && task.args == ["run", "test"])
        );
        assert!(!root.join(".forge").exists());
        std::fs::create_dir(root.join(".forge"))?;
        std::fs::write(
            root.join(".forge/tasks.json"),
            r#"{"version":1,"tasks":[]}"#,
        )?;
        assert!(load(root)?.is_empty());
        std::fs::write(root.join(".forge/tasks.json"), "invalid")?;
        assert!(load(root).is_err());
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn preserves_executable_shim_names_when_resolving_tools() -> io::Result<()> {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("dispatcher");
        let shim = directory.path().join("cargo");
        std::fs::write(&target, "#!/bin/sh\nexit 0\n")?;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700))?;
        symlink(&target, &shim)?;
        assert_eq!(executable(&shim), Some(std::path::absolute(&shim)?));
        Ok(())
    }
}
