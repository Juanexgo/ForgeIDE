use crate::project::Task;
use eframe::egui;
use std::{
    collections::VecDeque,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    time::Instant,
};

pub fn revision(tabs: &[crate::file::OpenFile], root: &Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hasher);
    let mut files: Vec<_> = tabs
        .iter()
        .filter(|file| file.path.starts_with(root))
        .collect();
    files.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    for file in files {
        file.path.hash(&mut hasher);
        file.revision().hash(&mut hasher);
        file.is_dirty().hash(&mut hasher);
        file.external_change.hash(&mut hasher);
        file.disk_stamp().hash(&mut hasher);
    }
    hasher.finish()
}

const OUTPUT_LIMIT: usize = 256 * 1024;

struct Run {
    child: Child,
    output: mpsc::Receiver<Vec<u8>>,
    bytes: Vec<u8>,
    status: Option<ExitStatus>,
    disconnected: bool,
    truncated: bool,
    started: Instant,
    revision: u64,
}

impl Run {
    fn start(task: &Task, root: &Path, revision: u64, context: &egui::Context) -> io::Result<Self> {
        let (command, cwd) = task.command(root)?;
        let argv = command.get_argv();
        let mut child = Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(cwd)
            .env("NO_COLOR", "1")
            .env("PATH", crate::project::environment_path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let (sender, output) = mpsc::sync_channel(64);
        drain(
            child.stdout.take().expect("stdout"),
            sender.clone(),
            context.clone(),
        );
        drain(
            child.stderr.take().expect("stderr"),
            sender,
            context.clone(),
        );
        Ok(Self {
            child,
            output,
            bytes: Vec::new(),
            status: None,
            disconnected: false,
            truncated: false,
            started: Instant::now(),
            revision,
        })
    }

    fn poll(&mut self) -> io::Result<bool> {
        for _ in 0..64 {
            match self.output.try_recv() {
                Ok(bytes) => {
                    let remaining = OUTPUT_LIMIT.saturating_sub(self.bytes.len());
                    self.truncated |= bytes.len() > remaining;
                    self.bytes
                        .extend_from_slice(&bytes[..bytes.len().min(remaining)]);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.disconnected = true;
                    break;
                }
            }
        }
        if self.status.is_none() {
            self.status = self.child.try_wait()?;
        }
        Ok(self.status.is_some() && self.disconnected)
    }

    fn text(&self) -> String {
        let mut text = String::from_utf8_lossy(&self.bytes).into_owned();
        if self.truncated {
            text.push_str(
                "
[Output display limited to 256 KiB; the process output was drained.]
",
            );
        }
        text
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if self.status.is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

fn drain(
    mut stream: impl Read + Send + 'static,
    sender: mpsc::SyncSender<Vec<u8>>,
    context: egui::Context,
) {
    std::thread::spawn(move || {
        let mut bytes = [0u8; 4096];
        loop {
            match stream.read(&mut bytes) {
                Ok(0) => break,
                Ok(length) => {
                    if sender.send(bytes[..length].to_vec()).is_err() {
                        break;
                    }
                    context.request_repaint();
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    let _ = sender.send(
                        format!(
                            "
Output error — {error}
"
                        )
                        .into_bytes(),
                    );
                    break;
                }
            }
        }
        context.request_repaint();
    });
}

enum ResultState {
    Pending,
    Passed,
    Failed,
    Cancelled,
}

struct Case {
    task: Task,
    state: ResultState,
    output: String,
    revision: Option<u64>,
    duration: f32,
}

#[derive(Default)]
pub struct Panel {
    root: Option<PathBuf>,
    cases: Vec<Case>,
    queue: VecDeque<usize>,
    active: Option<(usize, Run)>,
    selected: usize,
    notice: String,
}

impl Panel {
    pub fn load(&mut self, root: &Path) -> io::Result<()> {
        if self.active.is_some() {
            return Err(io::Error::other(
                "Stop the current test run before reloading.",
            ));
        }
        let cases = crate::project::load(root)?
            .into_iter()
            .filter(|task| task.kind.as_deref() == Some("test"))
            .map(|task| Case {
                task,
                state: ResultState::Pending,
                output: String::new(),
                revision: None,
                duration: 0.0,
            })
            .collect();
        self.cases = cases;
        self.root = Some(root.to_owned());
        self.queue.clear();
        self.selected = 0;
        Ok(())
    }

    pub fn summary(&self) -> (usize, usize, bool) {
        (
            self.cases.len(),
            self.cases
                .iter()
                .filter(|case| matches!(case.state, ResultState::Failed))
                .count(),
            self.active.is_some() || !self.queue.is_empty(),
        )
    }

    pub fn has_tests(&self) -> bool {
        !self.cases.is_empty()
    }

    pub fn queue_all(&mut self) {
        if self.active.is_none() {
            self.queue = (0..self.cases.len()).collect();
        }
    }

    pub fn tick(&mut self, context: &egui::Context, revision: u64) {
        if let Some((index, run)) = &mut self.active {
            context.request_repaint_after(std::time::Duration::from_millis(100));
            match run.poll() {
                Ok(true) => {
                    let case = &mut self.cases[*index];
                    case.state = if run.status.is_some_and(|status| status.success()) {
                        ResultState::Passed
                    } else {
                        ResultState::Failed
                    };
                    case.duration = run.started.elapsed().as_secs_f32();
                    case.output = run.text();
                    case.revision = Some(run.revision);
                    self.active = None;
                }
                Ok(false) => {}
                Err(error) => {
                    let case = &mut self.cases[*index];
                    case.output = error.to_string();
                    case.state = ResultState::Failed;
                    self.active = None;
                }
            }
        }
        if self.active.is_none() {
            if let (Some(index), Some(root)) = (self.queue.pop_front(), self.root.as_ref()) {
                self.selected = index;
                match Run::start(&self.cases[index].task, root, revision, context) {
                    Ok(run) => {
                        self.cases[index].state = ResultState::Pending;
                        self.cases[index].revision = Some(revision);
                        self.active = Some((index, run));
                        context.request_repaint_after(std::time::Duration::from_millis(100));
                    }
                    Err(error) => {
                        self.cases[index].state = ResultState::Failed;
                        self.cases[index].output = error.to_string();
                        self.cases[index].revision = Some(revision);
                        context.request_repaint();
                    }
                }
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, root: &Path, revision: u64, unsaved: usize) {
        if self.root.as_deref() != Some(root) && self.active.is_none() {
            if let Err(error) = self.load(root) {
                self.notice = error.to_string();
            }
        }
        let (total, failed, running) = self.summary();
        ui.label(format!(
            "{total} test tasks · {failed} failed{}",
            if running { " · running" } else { "" }
        ));
        ui.label(crate::chrome::small(
            "Test tasks run against files saved on disk.",
        ));
        if unsaved > 0 {
            ui.colored_label(
                crate::chrome::theme_red(),
                format!("{unsaved} open files have unsaved changes."),
            );
        }
        if ui.button("Load test tasks").clicked() {
            match self.load(root) {
                Ok(()) => {
                    self.notice = if self.cases.is_empty() {
                        "No supported test task detected. Add a test task in project configuration."
                            .into()
                    } else {
                        String::new()
                    }
                }
                Err(error) => self.notice = error.to_string(),
            }
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.has_tests() && self.active.is_none(),
                    egui::Button::new("Run all"),
                )
                .clicked()
            {
                self.queue_all();
            }
            if ui
                .add_enabled(
                    self.active.is_some() || !self.queue.is_empty(),
                    egui::Button::new("Stop"),
                )
                .clicked()
            {
                self.queue.clear();
                if let Some((index, run)) = self.active.take() {
                    let case = &mut self.cases[index];
                    case.state = ResultState::Cancelled;
                    case.output = run.text();
                    case.revision = Some(run.revision);
                    case.duration = run.started.elapsed().as_secs_f32();
                }
            }
        });
        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
        let busy = self.active.is_some();
        egui::ScrollArea::vertical()
            .id_salt("test-cases")
            .max_height(200.0)
            .show(ui, |ui| {
                for (index, case) in self.cases.iter().enumerate() {
                    ui.push_id(index, |ui| {
                        let running = self
                            .active
                            .as_ref()
                            .is_some_and(|(active, _)| *active == index);
                        let (label, color) = if running {
                            ("Running", crate::chrome::theme_accent())
                        } else {
                            match case.state {
                                ResultState::Pending => ("Not run", crate::chrome::theme_muted()),
                                ResultState::Passed => ("Passed", crate::chrome::theme_green()),
                                ResultState::Failed => ("Failed", crate::chrome::theme_red()),
                                ResultState::Cancelled => {
                                    ("Cancelled", crate::chrome::theme_muted())
                                }
                            }
                        };
                        if ui
                            .selectable_label(
                                self.selected == index,
                                egui::RichText::new(format!("{} · {label}", case.task.label))
                                    .color(color),
                            )
                            .clicked()
                        {
                            self.selected = index;
                        }
                        if ui
                            .add_enabled(!busy && self.queue.is_empty(), egui::Button::new("Run"))
                            .clicked()
                        {
                            self.queue.push_back(index);
                        }
                    });
                }
            });
        if let Some(case) = self.cases.get(self.selected) {
            if case.revision.is_some_and(|previous| previous != revision) {
                ui.colored_label(
                    crate::chrome::theme_red(),
                    "Open documents changed since this run.",
                );
            }
            ui.label(crate::chrome::small(format!("{:.2} s", case.duration)));
            let output = self
                .active
                .as_ref()
                .filter(|(index, _)| *index == self.selected)
                .map_or_else(|| case.output.clone(), |(_, run)| run.text());
            egui::ScrollArea::both()
                .id_salt("test-output")
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(output).monospace()).selectable(true),
                    );
                });
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn test_tasks_report_failure_and_bound_output_without_blocking() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        std::fs::create_dir(folder.path().join(".forge"))?;
        std::fs::write(
            folder.path().join(".forge/tasks.json"),
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "tasks": [
                    {"label": "large", "kind": "test", "program": "/bin/sh", "args": ["-c", "yes fixture | head -c 300000"]},
                    {"label": "failure", "kind": "test", "program": "/bin/sh", "args": ["-c", "printf failure; exit 3"]}
                ]
            }))?,
        )?;
        let context = egui::Context::default();
        let mut panel = Panel::default();
        panel.load(folder.path())?;
        panel.queue_all();
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        while !panel.queue.is_empty() || panel.active.is_some() {
            panel.tick(&context, 17);
            assert!(Instant::now() < deadline, "test tasks did not finish");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(matches!(panel.cases[0].state, ResultState::Passed));
        assert!(panel.cases[0].output.contains("Output display limited"));
        assert!(panel.cases[0].output.len() < OUTPUT_LIMIT + 200);
        assert!(matches!(panel.cases[1].state, ResultState::Failed));
        assert_eq!(panel.cases[1].output, "failure");
        assert_eq!(panel.cases[1].revision, Some(17));
        Ok(())
    }

    #[test]
    fn test_revision_changes_after_edit_and_save() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().canonicalize()?;
        let path = root.join("sample.txt");
        std::fs::write(&path, "old")?;
        let mut files = vec![crate::file::OpenFile::open(&path)?];
        let original = revision(&files, &root);
        files[0].text = "changed".into();
        files[0].sync_edit().map_err(io::Error::other)?;
        let edited = revision(&files, &root);
        assert_ne!(original, edited);
        files[0].save()?;
        assert_ne!(edited, revision(&files, &root));
        Ok(())
    }
}
