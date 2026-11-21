use crate::protocol::Process;
use eframe::egui::{self, ScrollArea, TextEdit};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Panel {
    adapter: Option<PathBuf>,
    program: Option<PathBuf>,
    lines: String,
    target_args: String,
    attach: bool,
    pid: String,
    gutter: BTreeMap<PathBuf, Vec<u32>>,
    expression: String,
    profiles: Vec<DebugProfile>,
    selected_profile: usize,
    session: Option<Session>,
    notice: String,
}

enum Target {
    Launch { program: PathBuf, args: Vec<String> },
    Attach { pid: u32 },
    Profile { request: String, arguments: Value },
}

#[derive(serde::Deserialize)]
struct DebugProfile {
    name: String,
    adapter: String,
    #[serde(default)]
    adapter_args: Vec<String>,
    request: String,
    arguments: Value,
}

#[derive(serde::Deserialize)]
struct LaunchFile {
    version: u32,
    configurations: Vec<DebugProfile>,
}

fn substitute(value: &Value, root: &Path, source: &Path) -> Value {
    match value {
        Value::String(text) => json!(
            text.replace("${workspaceFolder}", &root.to_string_lossy())
                .replace("${file}", &source.to_string_lossy())
        ),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| substitute(value, root, source))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), substitute(value, root, source)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

struct Config {
    target: Target,
    source: PathBuf,
    cwd: PathBuf,
    lines: Vec<u32>,
}

struct Pending {
    command: &'static str,
    sent: Instant,
    epoch: u64,
    arguments: Value,
}

struct Session {
    process: Process,
    config: Config,
    next_seq: u64,
    pending: BTreeMap<u64, Pending>,
    epoch: u64,
    thread: Option<u64>,
    supports_configuration_done: bool,
    disconnecting: bool,
    ended: bool,
    notice: String,
    breakpoints: Vec<String>,
    stack: Vec<Value>,
    scopes: Vec<Value>,
    variables: Vec<String>,
    variable_nodes: Vec<Value>,
    watch_expressions: Vec<String>,
    evaluations: BTreeMap<String, String>,
    frame: Option<u64>,
    output: String,
}

impl Panel {
    pub fn state_label(&self) -> &'static str {
        match &self.session {
            Some(session) if session.thread.is_some() => "Paused",
            Some(_) => "Running",
            None => "Inactive",
        }
    }
    pub fn stopped_location(&self) -> Option<(PathBuf, usize)> {
        let session = self.session.as_ref()?;
        session.thread?;
        let frame = session
            .stack
            .iter()
            .find(|frame| frame["id"].as_u64() == session.frame)?;
        let path = frame["source"]["path"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| session.config.source.clone());
        Some((path, frame["line"].as_u64()?.max(1) as usize))
    }
    pub fn handle_keys(
        &mut self,
        context: &egui::Context,
        root: &Path,
        source: Option<&Path>,
    ) -> bool {
        let stop =
            context.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::F5));
        if stop {
            if let Some(session) = &mut self.session {
                if let Err(error) = session.stop() {
                    self.fail(error);
                }
            }
            return true;
        }
        let command =
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
                Some("continue")
            } else if context
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F10))
            {
                Some("next")
            } else if context
                .input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::F11))
            {
                Some("stepOut")
            } else if context
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F11))
            {
                Some("stepIn")
            } else {
                None
            };
        let Some(command) = command else {
            return false;
        };
        if let Some(session) = &mut self.session {
            if let Some(thread) = session.thread {
                if let Err(error) = session.resume(command, thread) {
                    self.fail(error);
                }
            }
        } else if command == "continue" {
            if let Err(error) = self.start_configured(context, root, source) {
                self.notice = error.to_string();
            }
        }
        true
    }
    fn start_configured(
        &mut self,
        context: &egui::Context,
        root: &Path,
        source: Option<&Path>,
    ) -> io::Result<()> {
        if self.profiles.is_empty() && root.join(".forge/launch.json").is_file() {
            let profiles: LaunchFile = serde_json::from_slice(&crate::packages::read_bounded(
                &root.join(".forge/launch.json"),
                256 * 1024,
            )?)?;
            if profiles.version != 1 || profiles.configurations.len() > 50 {
                return Err(io::Error::other("Unsupported debug configuration."));
            }
            self.profiles = profiles.configurations;
            self.selected_profile = 0;
        }
        let source = source
            .ok_or_else(|| {
                io::Error::other("Open a source file and choose a debug profile or adapter.")
            })?
            .canonicalize()?;
        let lines = self
            .gutter
            .get(&source)
            .cloned()
            .unwrap_or(parse_lines(&self.lines)?);
        if let Some(profile) = self.profiles.get(self.selected_profile) {
            let adapter = crate::project::locate(&profile.adapter, root).ok_or_else(|| {
                io::Error::other(format!("Missing debug adapter — {}", profile.adapter))
            })?;
            if !matches!(profile.request.as_str(), "launch" | "attach")
                || !profile.arguments.is_object()
            {
                return Err(io::Error::other(
                    "Invalid debug profile request or arguments.",
                ));
            }
            let config = Config {
                target: Target::Profile {
                    request: profile.request.clone(),
                    arguments: substitute(&profile.arguments, root, &source),
                },
                source,
                cwd: root.to_owned(),
                lines,
            };
            let arguments = profile
                .adapter_args
                .iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>();
            self.session = Some(Session::start_with_args(
                &adapter, &arguments, config, context,
            )?);
        } else {
            let target = if self.attach {
                let pid = self
                    .pid
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|pid| *pid > 0)
                    .ok_or_else(|| io::Error::other("Enter a positive process ID."))?;
                Target::Attach { pid }
            } else {
                Target::Launch {
                    program: self
                        .program
                        .as_ref()
                        .ok_or_else(|| io::Error::other("Choose the program to debug."))?
                        .canonicalize()?,
                    args: self.target_args.lines().map(str::to_owned).collect(),
                }
            };
            let adapter = self.adapter.as_ref().ok_or_else(|| {
                io::Error::other("Choose a debug adapter or load a project profile.")
            })?;
            self.session = Some(Session::start(
                adapter,
                Config {
                    target,
                    source,
                    cwd: root.to_owned(),
                    lines,
                },
                context,
            )?);
        }
        Ok(())
    }

    pub fn show(&mut self, ui: &mut egui::Ui, root: &Path, source: Option<&Path>) {
        ui.label(crate::chrome::small(format!(
            "{} · F5 continue/start · F9 breakpoint · F10/F11 step",
            self.state_label()
        )));
        if let Some(source) = source {
            if self.session.is_none() {
                if let Some(lines) = self.gutter.get(source) {
                    self.lines = lines
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                }
            }
        }
        if let Some(session) = &mut self.session {
            ui.add(TextEdit::singleline(&mut self.expression).hint_text("Watch expression"));
            if ui
                .add_enabled(
                    session.frame.is_some() && !self.expression.trim().is_empty(),
                    egui::Button::new("Add watch / evaluate"),
                )
                .clicked()
            {
                let expression = self.expression.trim().to_owned();
                if !session.watch_expressions.contains(&expression)
                    && session.watch_expressions.len() < 20
                {
                    session.watch_expressions.push(expression.clone());
                }
                if let Err(error) = session.evaluate(&expression) {
                    session.notice = error.to_string();
                }
            }
            let mut remove_watch = None;
            for (index, expression) in session.watch_expressions.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{expression} = {}",
                        session
                            .evaluations
                            .get(expression)
                            .map_or("…", String::as_str)
                    ));
                    if ui.small_button("×").clicked() {
                        remove_watch = Some(index);
                    }
                });
            }
            if let Some(index) = remove_watch {
                session.watch_expressions.remove(index);
            }
            if ui
                .add_enabled(!session.disconnecting, egui::Button::new("Stop Debugger"))
                .clicked()
            {
                if let Err(error) = session.stop() {
                    self.fail(error);
                }
            }
            let Some(session) = &mut self.session else {
                return;
            };
            ui.label(&session.notice);
            if let Some(thread) = session.thread {
                let mut command = None;
                ui.horizontal_wrapped(|ui| {
                    for (label, step) in [
                        ("Continue", "continue"),
                        ("Step Over", "next"),
                        ("Step In", "stepIn"),
                        ("Step Out", "stepOut"),
                    ] {
                        if ui.button(label).clicked() {
                            command = Some(step);
                        }
                    }
                });
                if let Some(command) = command {
                    if let Err(error) = session.resume(command, thread) {
                        self.fail(error);
                    }
                }
            }
            let Some(session) = &mut self.session else {
                return;
            };
            ScrollArea::vertical()
                .id_salt("debug-details")
                .show(ui, |ui| {
                    let mut selected = None;
                    for breakpoint in &session.breakpoints {
                        ui.label(breakpoint);
                    }
                    if !session.stack.is_empty() {
                        ui.heading("Call stack");
                        for frame in &session.stack {
                            let name = frame["name"].as_str().unwrap_or("Frame");
                            let line = frame["line"].as_u64().unwrap_or(0);
                            if ui.button(format!("{name} · line {line}")).clicked() {
                                session.epoch += 1;
                                session.evaluations.clear();
                                session.frame = frame["id"].as_u64();
                                selected =
                                    session.frame.map(|id| ("scopes", json!({"frameId": id})));
                            }
                        }
                    }
                    if !session.scopes.is_empty() {
                        ui.heading("Variables");
                        for scope in &session.scopes {
                            if ui
                                .button(scope["name"].as_str().unwrap_or("Scope"))
                                .clicked()
                            {
                                selected = scope["variablesReference"]
                                    .as_u64()
                                    .map(|id| ("variables", json!({"variablesReference": id})));
                            }
                        }
                    }
                    for variable in &session.variable_nodes {
                        let label = format!(
                            "{} = {}",
                            variable["name"].as_str().unwrap_or(""),
                            variable["value"].as_str().unwrap_or("")
                        );
                        let reference = variable["variablesReference"].as_u64().unwrap_or(0);
                        if reference > 0 {
                            if ui.button(format!("▸ {label}")).clicked() {
                                selected =
                                    Some(("variables", json!({"variablesReference": reference})));
                            }
                        } else {
                            ui.label(label);
                        }
                    }
                    if let Some((command, args)) = selected {
                        if command == "scopes" {
                            for expression in session.watch_expressions.clone() {
                                if let Err(error) = session.evaluate(&expression) {
                                    session.notice = error.to_string();
                                }
                            }
                        }
                        if let Err(error) = session.send(command, args) {
                            session.notice = error.to_string();
                        }
                    }
                    if !session.output.is_empty() {
                        ui.heading("Debug output");
                        ui.label(&session.output);
                    }
                });
        } else {
            ui.collapsing("Project debug profiles", |ui| {
                if ui.button("Load .forge/launch.json").clicked() {
                    let loaded = (|| -> io::Result<Vec<DebugProfile>> {
                        let file = std::fs::File::open(root.join(".forge/launch.json"))?;
                        if file.metadata()?.len() > 256 * 1024 { return Err(io::Error::other("Launch configuration exceeds 256 KiB.")); }
                        let profiles: LaunchFile = serde_json::from_reader(file)?;
                        if profiles.version != 1 || profiles.configurations.len() > 50 {
                            return Err(io::Error::other("Use launch schema version 1 with up to 50 configurations."));
                        }
                        Ok(profiles.configurations)
                    })();
                    match loaded {
                        Ok(profiles) => { self.profiles = profiles; self.selected_profile = 0; self.notice.clear(); }
                        Err(error) => self.notice = error.to_string(),
                    }
                }
                if let Some(profile) = self.profiles.get(self.selected_profile) {
                    egui::ComboBox::from_id_salt("debug-profile").selected_text(&profile.name).show_ui(ui, |ui| {
                        for (index, profile) in self.profiles.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_profile, index, &profile.name);
                        }
                    });
                }
                if let Some(profile) = self.profiles.get(self.selected_profile) {
                    let adapter = crate::project::locate(&profile.adapter, root);
                    if adapter.is_none() { ui.label(format!("Missing adapter — {}", profile.adapter)); }
                    if ui.add_enabled(adapter.is_some() && source.is_some(), egui::Button::new("Start profile")).clicked() {
                        let result = (|| {
                            let source = source.expect("profile source").canonicalize()?;
                            if !matches!(profile.request.as_str(), "launch" | "attach") || !profile.arguments.is_object() {
                                return Err(io::Error::other("Profiles need a launch or attach request and an arguments object."));
                            }
                            let config = Config {
                                target: Target::Profile { request: profile.request.clone(), arguments: substitute(&profile.arguments, root, &source) },
                                source,
                                cwd: root.to_owned(),
                                lines: parse_lines(&self.lines)?,
                            };
                            let args: Vec<_> = profile.adapter_args.iter().map(std::ffi::OsString::from).collect();
                            Session::start_with_args(&adapter.expect("available adapter"), &args, config, ui.ctx())
                        })();
                        match result {
                            Ok(session) => self.session = Some(session),
                            Err(error) => self.notice = error.to_string(),
                        }
                    }
                }
            });
            ui.label("Use a local lldb-dap executable with a debug build.");
            picker(ui, "Choose Adapter…", &mut self.adapter);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.attach, false, "Launch");
                ui.selectable_value(&mut self.attach, true, "Attach");
            });
            if self.attach {
                ui.add(TextEdit::singleline(&mut self.pid).hint_text("Local process ID"));
            } else {
                picker(ui, "Choose Program…", &mut self.program);
                ui.add(
                    TextEdit::multiline(&mut self.target_args)
                        .desired_rows(2)
                        .hint_text("Program arguments, one per line"),
                );
            }
            if let Some(source) = source {
                ui.label(source.to_string_lossy());
            }
            ui.add(
                TextEdit::singleline(&mut self.lines).hint_text("Breakpoint lines in active file"),
            );
            let ready = self.adapter.is_some()
                && source.is_some()
                && if self.attach {
                    !self.pid.trim().is_empty()
                } else {
                    self.program.is_some()
                };
            let label = if self.attach {
                "Attach Debugger"
            } else {
                "Launch Debugger"
            };
            if ui.add_enabled(ready, egui::Button::new(label)).clicked() {
                let result = (|| {
                    let target = if self.attach {
                        let pid = self
                            .pid
                            .trim()
                            .parse::<u32>()
                            .ok()
                            .filter(|pid| *pid > 0)
                            .ok_or_else(|| io::Error::other("Enter a positive process ID."))?;
                        Target::Attach { pid }
                    } else {
                        let program = self
                            .program
                            .as_ref()
                            .expect("program selected")
                            .canonicalize()?;
                        if program.to_str().is_none() {
                            return Err(io::Error::other(
                                "Debugger paths must be representable as Unicode.",
                            ));
                        }
                        Target::Launch {
                            program,
                            args: self.target_args.lines().map(str::to_owned).collect(),
                        }
                    };
                    let config = Config {
                        target,
                        source: source.expect("workspace source selected").canonicalize()?,
                        cwd: root.to_owned(),
                        lines: parse_lines(&self.lines)?,
                    };
                    if config.source.to_str().is_none() || config.cwd.to_str().is_none() {
                        return Err(io::Error::other(
                            "Debugger paths must be representable as Unicode.",
                        ));
                    }
                    Session::start(
                        self.adapter.as_ref().expect("adapter selected"),
                        config,
                        ui.ctx(),
                    )
                })();
                match result {
                    Ok(session) => self.session = Some(session),
                    Err(error) => self.fail(error),
                }
            }
            if !self.notice.is_empty() {
                ui.label(&self.notice);
            }
        }
    }

    pub fn breakpoints(&self, source: &Path) -> &[u32] {
        self.gutter.get(source).map_or(&[], Vec::as_slice)
    }

    pub fn toggle_breakpoint(&mut self, source: &Path, line: u32) {
        let lines = self.gutter.entry(source.to_owned()).or_default();
        if let Some(index) = lines.iter().position(|entry| *entry == line) {
            lines.remove(index);
        } else if lines.len() < 100 {
            lines.push(line);
            lines.sort_unstable();
        }
        self.lines = lines
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        if let Some(session) = &mut self.session {
            let breakpoints: Vec<_> = lines.iter().map(|line| json!({"line": line})).collect();
            if let Err(error) = session.send("setBreakpoints", json!({"source": {"path": source}, "breakpoints": breakpoints, "sourceModified": true})) {
                session.notice = error.to_string();
            }
        }
    }

    pub fn tick(&mut self, context: &egui::Context) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if !session.ended {
            context.request_repaint_after(Duration::from_millis(150));
        }
        if let Err(error) = session.poll() {
            self.fail(error);
        } else if session.disconnecting && session.ended {
            self.session = None;
            self.notice = "Debugger stopped.".into();
        }
    }

    fn fail(&mut self, error: io::Error) {
        self.session = None;
        self.notice = error.to_string();
    }
}

fn picker(ui: &mut egui::Ui, label: &str, selection: &mut Option<PathBuf>) {
    if ui.button(label).clicked() {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            *selection = Some(path);
        }
    }
    if let Some(path) = selection {
        ui.label(path.to_string_lossy());
    }
}

fn parse_lines(input: &str) -> io::Result<Vec<u32>> {
    let mut lines = input
        .split(|character: char| character == ',' || character.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<u32>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| io::Error::other("Breakpoint lines must be positive numbers."))
        })
        .take(101)
        .collect::<io::Result<Vec<_>>>()?;
    if lines.len() > 100 {
        return Err(io::Error::other("Up to 100 breakpoints per launch."));
    }
    lines.sort_unstable();
    lines.dedup();
    Ok(lines)
}

impl Session {
    fn start(adapter: &Path, config: Config, context: &egui::Context) -> io::Result<Self> {
        Self::start_with_args(adapter, &[], config, context)
    }

    fn start_with_args(
        adapter: &Path,
        args: &[std::ffi::OsString],
        config: Config,
        context: &egui::Context,
    ) -> io::Result<Self> {
        let process = Process::spawn(adapter, args, &config.cwd, context)?;
        let mut session = Self {
            process,
            config,
            next_seq: 1,
            pending: BTreeMap::new(),
            epoch: 0,
            thread: None,
            supports_configuration_done: false,
            disconnecting: false,
            ended: false,
            notice: "Initializing debugger…".into(),
            breakpoints: Vec::new(),
            stack: Vec::new(),
            scopes: Vec::new(),
            variables: Vec::new(),
            variable_nodes: Vec::new(),
            watch_expressions: Vec::new(),
            evaluations: BTreeMap::new(),
            frame: None,
            output: String::new(),
        };
        session.send(
            "initialize",
            json!({
                "clientID": "forgeide", "adapterID": match &session.config.target {
                    Target::Profile { arguments, .. } => arguments["type"].as_str().unwrap_or("forgeide"),
                    _ => "lldb",
                },
                "linesStartAt1": true, "columnsStartAt1": true, "pathFormat": "path",
                "supportsRunInTerminalRequest": false
            }),
        )?;
        Ok(session)
    }

    fn send(&mut self, command: &'static str, args: Value) -> io::Result<()> {
        if self.pending.len() >= 32 {
            return Err(io::Error::other("Too many pending debugger requests."));
        }
        let seq = self.next_seq;
        self.process
            .send(&json!({"seq": seq, "type": "request", "command": command, "arguments": args}))?;
        self.pending.insert(
            seq,
            Pending {
                command,
                sent: Instant::now(),
                epoch: self.epoch,
                arguments: args,
            },
        );
        self.next_seq += 1;
        Ok(())
    }

    fn stop(&mut self) -> io::Result<()> {
        if self.ended {
            self.disconnecting = true;
        } else {
            let terminate = match &self.config.target {
                Target::Launch { .. } => true,
                Target::Profile { request, .. } => request == "launch",
                Target::Attach { .. } => false,
            };
            self.send("disconnect", json!({"terminateDebuggee": terminate}))?;
            self.disconnecting = true;
        }
        Ok(())
    }

    fn resume(&mut self, command: &'static str, thread: u64) -> io::Result<()> {
        self.send(command, json!({"threadId": thread}))?;
        self.clear_stop();
        self.notice = "Running…".into();
        Ok(())
    }

    fn clear_stop(&mut self) {
        self.epoch += 1;
        self.thread = None;
        self.stack.clear();
        self.scopes.clear();
        self.variables.clear();
        self.variable_nodes.clear();
        self.evaluations.clear();
        self.frame = None;
    }

    fn evaluate(&mut self, expression: &str) -> io::Result<()> {
        self.send(
            "evaluate",
            json!({"expression": expression, "frameId": self.frame, "context": "watch"}),
        )
    }

    fn poll(&mut self) -> io::Result<()> {
        if self.ended {
            return Ok(());
        }
        if let Some(pending) = self
            .pending
            .values()
            .find(|request| request.sent.elapsed() > Duration::from_secs(20))
        {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("Debugger request {} timed out.", pending.command),
            ));
        }
        for _ in 0..16 {
            match self.process.poll() {
                Ok(Some(message)) => self.handle(&message)?,
                Ok(None) => break,
                Err(_) if self.disconnecting || self.ended => {
                    self.ended = true;
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn handle(&mut self, message: &Value) -> io::Result<()> {
        #[cfg(test)]
        if std::env::var_os("FORGE_TEST_DAP_TRACE").is_some() {
            eprintln!(
                "debugger message: {} {} {} {}",
                message["type"], message["event"], message["command"], message["request_seq"]
            );
        }
        match message["type"].as_str() {
            Some("event") => self.event(message),
            Some("response") => self.response(message),
            Some("request") => {
                self.process.send(&json!({
                    "seq": self.next_seq, "type": "response", "request_seq": message["seq"],
                    "success": false, "command": message["command"], "message": "Reverse requests are disabled"
                }))?;
                self.next_seq += 1;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn event(&mut self, message: &Value) -> io::Result<()> {
        let body = &message["body"];
        match message["event"].as_str() {
            Some("initialized") => {
                let breakpoints = self
                    .config
                    .lines
                    .iter()
                    .map(|line| json!({"line": line}))
                    .collect::<Vec<_>>();
                self.send(
                    "setBreakpoints",
                    json!({
                        "source": {"path": self.config.source},
                        "breakpoints": breakpoints,
                        "sourceModified": false
                    }),
                )?;
            }
            Some("stopped") => {
                self.clear_stop();
                self.notice = format!(
                    "Stopped: {}",
                    body["reason"].as_str().unwrap_or("unknown reason")
                );
                if let Some(thread) = body["threadId"].as_u64() {
                    self.inspect(thread)?;
                } else {
                    self.send("threads", json!({}))?;
                }
            }
            Some("continued") if self.thread.is_some() => self.clear_stop(),
            Some("output") => {
                if let Some(output) = body["output"].as_str() {
                    self.output.extend(output.chars().take(4096));
                    if self.output.len() > 32 * 1024 {
                        let mut boundary = self.output.len() - 32 * 1024;
                        while !self.output.is_char_boundary(boundary) {
                            boundary += 1;
                        }
                        self.output.drain(..boundary);
                    }
                }
            }
            Some("exited") => {
                self.clear_stop();
                self.notice = format!("Program exited with code {}.", body["exitCode"]);
            }
            Some("terminated") => {
                self.clear_stop();
                self.notice = "Debug session ended.".into();
            }
            _ => {}
        }
        Ok(())
    }

    fn inspect(&mut self, thread: u64) -> io::Result<()> {
        self.thread = Some(thread);
        self.send(
            "stackTrace",
            json!({"threadId": thread, "startFrame": 0, "levels": 30}),
        )
    }

    fn response(&mut self, message: &Value) -> io::Result<()> {
        let Some(request) = message["request_seq"]
            .as_u64()
            .and_then(|seq| self.pending.remove(&seq))
        else {
            return Ok(());
        };
        if message["success"] != true {
            let message = message["message"].as_str().unwrap_or("request rejected");
            if request.command == "evaluate" {
                if self.epoch == request.epoch {
                    if let Some(expression) = request.arguments["expression"].as_str() {
                        self.evaluations.insert(expression.into(), message.into());
                    }
                }
                return Ok(());
            }
            return Err(io::Error::other(format!(
                "Debugger {}: {message}",
                request.command
            )));
        }
        let body = &message["body"];
        match request.command {
            "initialize" => {
                self.supports_configuration_done = body["supportsConfigurationDoneRequest"] == true;
                let (command, args) = match &self.config.target {
                    Target::Launch { program, args } => {
                        self.notice = "Launching program…".into();
                        (
                            "launch",
                            json!({
                                "program": program, "cwd": self.config.cwd, "args": args,
                                "stopOnEntry": self.config.lines.is_empty(), "disableASLR": false,
                            "initCommands": ["settings set target.disable-aslr false"]
                            }),
                        )
                    }
                    Target::Profile { request, arguments } => {
                        let command = if request == "launch" {
                            "launch"
                        } else {
                            "attach"
                        };
                        (command, arguments.clone())
                    }
                    Target::Attach { pid } => {
                        self.notice = "Attaching to process…".into();
                        ("attach", json!({"pid": pid, "stopOnEntry": true}))
                    }
                };
                self.send(command, args)?;
            }
            "setBreakpoints" => {
                self.breakpoints = items(body, "breakpoints")
                    .map(|breakpoint| {
                        let state = if breakpoint["verified"] == true {
                            "verified"
                        } else {
                            "pending"
                        };
                        format!(
                            "Breakpoint at line {} ({state})",
                            breakpoint["line"].as_u64().unwrap_or(0)
                        )
                    })
                    .collect();
                if self.supports_configuration_done && request.arguments["sourceModified"] != true {
                    self.send("configurationDone", json!({}))?;
                }
            }
            "launch" if self.thread.is_none() => self.notice = "Program launched.".into(),
            "attach" if self.thread.is_none() => self.notice = "Process attached.".into(),
            "threads" if self.epoch == request.epoch => {
                if let Some(thread) = body["threads"][0]["id"].as_u64() {
                    self.inspect(thread)?;
                }
            }
            "stackTrace" if self.epoch == request.epoch => {
                self.stack = items(body, "stackFrames").cloned().collect();
                if let Some(frame) = self.stack.first().and_then(|frame| frame["id"].as_u64()) {
                    self.frame = Some(frame);
                    self.send("scopes", json!({"frameId": frame}))?;
                    for expression in self.watch_expressions.clone() {
                        self.evaluate(&expression)?;
                    }
                }
            }
            "scopes" if self.epoch == request.epoch => {
                self.scopes = items(body, "scopes").cloned().collect();
                self.variables.clear();
                if let Some(scope) = self.scopes.iter().find(|scope| scope["expensive"] != true) {
                    if let Some(reference) =
                        scope["variablesReference"].as_u64().filter(|id| *id != 0)
                    {
                        self.send("variables", json!({"variablesReference": reference}))?;
                    }
                }
            }
            "evaluate" if self.epoch == request.epoch => {
                if let Some(expression) = request.arguments["expression"].as_str() {
                    self.evaluations.insert(
                        expression.into(),
                        body["result"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(4096)
                            .collect(),
                    );
                }
            }
            "variables" if self.epoch == request.epoch => {
                self.variable_nodes = items(body, "variables").cloned().collect();
                self.variables = items(body, "variables")
                    .map(|variable| {
                        format!(
                            "{} = {}",
                            variable["name"].as_str().unwrap_or(""),
                            variable["value"]
                                .as_str()
                                .unwrap_or("")
                                .chars()
                                .take(512)
                                .collect::<String>()
                        )
                    })
                    .collect();
            }
            "disconnect" => self.ended = true,
            _ => {}
        }
        Ok(())
    }
}

fn items<'a>(response: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    response[key].as_array().into_iter().flatten().take(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sorted_unique_breakpoints() {
        assert_eq!(parse_lines("6, 4 4").unwrap(), vec![4, 6]);
        assert!(parse_lines("0").is_err());
    }

    #[test]
    #[ignore = "requires an installed lldb-dap and a compiled fixture"]
    fn talks_to_real_lldb() -> io::Result<()> {
        let adapter = PathBuf::from(std::env::var_os("FORGE_TEST_DAP").unwrap());
        let root = PathBuf::from(std::env::var_os("FORGE_TEST_DAP_ROOT").unwrap());
        let config = Config {
            target: Target::Launch {
                program: root.join("sample"),
                args: vec![],
            },
            source: root.join("sample.c"),
            cwd: root,
            lines: vec![3],
        };
        let mut session = Session::start(&adapter, config, &egui::Context::default())?;
        session.watch_expressions.push("answer".into());
        let deadline = Instant::now() + Duration::from_secs(25);
        while Instant::now() < deadline {
            session.poll()?;
            if let Some(thread) = session.thread {
                if session
                    .variables
                    .iter()
                    .any(|variable| variable.contains("answer = 42"))
                    && session
                        .evaluations
                        .get("answer")
                        .is_some_and(|result| result.contains("42"))
                {
                    session.resume("continue", thread)?;
                    session.stop()?;
                    while !session.ended && Instant::now() < deadline {
                        session.poll()?;
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    assert!(session.ended);
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!(
            "{}; variables: {:?}; breakpoints: {:?}",
            session.notice, session.variables, session.breakpoints
        );
    }
}
