use crate::{
    file::OpenFile,
    language_edit::{self, Preview},
    protocol::Process,
};
use eframe::egui::{self, ScrollArea, TextEdit};
use forge_text::position::{Encoding, LineIndex};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

struct Slot {
    name: &'static str,
    extensions: &'static [&'static str],
    panel: Panel,
    last: Option<(PathBuf, String)>,
}

pub struct Hub {
    slots: Vec<Slot>,
    selected: usize,
    auto_complete: Option<(PathBuf, usize, Instant)>,
    profile_notice: String,
    pub action: Option<Feature>,
    pub destination: Option<(PathBuf, usize)>,
}

impl Default for Hub {
    fn default() -> Self {
        let slots = [
            ("Rust", &["rs"][..]),
            (
                "TypeScript / JavaScript",
                &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"][..],
            ),
            ("Python", &["py"][..]),
            ("C / C++", &["c", "h", "cpp", "hpp", "cc"][..]),
            ("Go", &["go"][..]),
            ("JSON", &["json"][..]),
        ]
        .into_iter()
        .map(|(name, extensions)| Slot {
            name,
            extensions,
            panel: Panel::default(),
            last: None,
        })
        .collect();
        Self {
            slots,
            selected: 0,
            auto_complete: None,
            profile_notice: String::new(),
            action: None,
            destination: None,
        }
    }
}

impl Hub {
    fn slot(&self, path: &Path) -> Option<usize> {
        let extension = path.extension()?.to_str()?;
        self.slots
            .iter()
            .position(|slot| slot.extensions.contains(&extension))
    }

    pub fn problem_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.last.is_some())
            .map(|slot| slot.panel.marks.len())
            .sum::<usize>()
            .min(2000)
    }

    pub fn problems(&self) -> Vec<(PathBuf, usize, String)> {
        self.slots
            .iter()
            .flat_map(|slot| {
                slot.last.as_ref().into_iter().flat_map(|(path, _)| {
                    slot.panel
                        .marks
                        .iter()
                        .map(move |(line, message)| (path.clone(), *line, message.clone()))
                })
            })
            .take(2000)
            .collect()
    }
    pub fn running_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.panel.session.is_some())
            .count()
    }
    fn detect_tools(&mut self, root: &Path) {
        let mut found = 0;
        for slot in &mut self.slots {
            if slot.panel.executable.is_some() || slot.panel.session.is_some() {
                continue;
            }
            let (program, args) = match slot.name {
                "Rust" => ("rust-analyzer", ""),
                "TypeScript / JavaScript" => ("typescript-language-server", "--stdio"),
                "Python" => ("pylsp", ""),
                "C / C++" => ("clangd", ""),
                "Go" => ("gopls", ""),
                _ => ("vscode-json-language-server", "--stdio"),
            };
            if let Some(path) = crate::project::locate(program, root).filter(|path| {
                path.canonicalize()
                    .is_ok_and(|path| !path.starts_with(root))
            }) {
                slot.panel.executable = Some(path);
                slot.panel.args = args.into();
                found += 1;
            }
        }
        self.profile_notice =
            format!("{found} installed tools detected. Select a language and start its server.");
    }

    pub fn show(&mut self, ui: &mut egui::Ui, root: &Path) {
        if ui.button("Detect installed language tools").clicked() {
            self.detect_tools(root);
        }
        ui.label(crate::chrome::small(format!(
            "{} language servers active",
            self.running_count()
        )));
        if ui
            .button("Load project language profiles")
            .on_hover_text(".forge/languages.json")
            .clicked()
        {
            if let Err(error) = self.load_profiles(root) {
                self.profile_notice = error.to_string();
            }
        }
        if !self.profile_notice.is_empty() {
            ui.label(&self.profile_notice);
        }
        egui::ComboBox::from_id_salt("language-profile")
            .selected_text(self.slots[self.selected].name)
            .show_ui(ui, |ui| {
                for (index, slot) in self.slots.iter().enumerate() {
                    ui.selectable_value(&mut self.selected, index, slot.name);
                }
            });
        self.slots[self.selected].panel.show(ui, root);
        self.action = self.slots[self.selected]
            .panel
            .action
            .take()
            .or(self.action.take());
    }

    fn load_profiles(&mut self, root: &Path) -> io::Result<()> {
        #[derive(serde::Deserialize)]
        struct Server {
            extension: String,
            program: String,
            #[serde(default)]
            args: Vec<String>,
        }
        #[derive(serde::Deserialize)]
        struct Profiles {
            version: u32,
            servers: Vec<Server>,
        }
        let file = std::fs::File::open(root.join(".forge/languages.json"))?;
        if file.metadata()?.len() > 256 * 1024 {
            return Err(io::Error::other("Language profiles exceed 256 KiB."));
        }
        let profiles: Profiles = serde_json::from_reader(file)?;
        if profiles.version != 1 || profiles.servers.len() > self.slots.len() {
            return Err(io::Error::other(
                "Use language schema version 1 with one server per language group.",
            ));
        }
        let mut pending = Vec::new();
        for server in profiles.servers {
            let index = self
                .slot(Path::new(&format!("source.{}", server.extension)))
                .ok_or_else(|| io::Error::other("Unsupported language extension in profile."))?;
            if pending.iter().any(|(slot, _, _)| *slot == index) {
                return Err(io::Error::other(
                    "Duplicate server in the same language group.",
                ));
            }
            if server.args.len() > 100
                || server
                    .args
                    .iter()
                    .any(|argument| argument.contains(['\n', '\r']))
            {
                return Err(io::Error::other(
                    "Language profiles need up to 100 single-line arguments.",
                ));
            }
            let executable = crate::project::locate(&server.program, root).ok_or_else(|| {
                io::Error::other(format!(
                    "Missing language tool — {}. Install it or configure its absolute path.",
                    server.program
                ))
            })?;
            pending.push((index, executable, server.args.join("\n")));
        }
        for (index, executable, args) in pending {
            self.slots[index].panel.executable = Some(executable);
            self.slots[index].panel.args = args;
        }
        self.profile_notice = "Profiles loaded. Start each selected server to enable it.".into();
        Ok(())
    }

    pub fn tick(
        &mut self,
        context: &egui::Context,
        file: Option<(&Path, &str)>,
        tabs: &[OpenFile],
    ) {
        for slot in &mut self.slots {
            if let Some((path, text)) = &mut slot.last {
                if let Some(tab) = tabs.iter().find(|tab| tab.path == *path) {
                    if *text != tab.text {
                        text.clone_from(&tab.text);
                    }
                }
            }
        }
        if let Some((path, text)) = file {
            if let Some(index) = self.slot(path) {
                let last = &mut self.slots[index].last;
                if last
                    .as_ref()
                    .is_none_or(|(previous, contents)| previous != path || contents != text)
                {
                    *last = Some((path.to_owned(), text.to_owned()));
                }
            }
        }
        for slot in &mut self.slots {
            slot.panel.tick(
                context,
                slot.last
                    .as_ref()
                    .map(|(path, text)| (path.as_path(), text.as_str())),
            );
            if let Some(destination) = slot.panel.destination.take() {
                self.destination = Some(destination);
            }
        }
        if self
            .auto_complete
            .as_ref()
            .is_some_and(|(_, _, due)| due.elapsed() >= Duration::from_millis(160))
        {
            if let Some((path, byte, _)) = self.auto_complete.take() {
                if let Some((active, text)) = file.filter(|(active, _)| *active == path) {
                    self.request_at(Feature::Completion, active, text, byte);
                }
            }
        }
    }

    pub fn queue_completion(&mut self, context: &egui::Context, path: &Path, byte: usize) {
        self.auto_complete = Some((path.to_owned(), byte, Instant::now()));
        context.request_repaint_after(Duration::from_millis(160));
    }

    pub fn completion_keys(&mut self, context: &egui::Context, path: &Path) {
        let Some(index) = self.slot(path) else { return };
        let panel = &mut self.slots[index].panel;
        let Some((pending, items)) = &panel.suggestions else {
            return;
        };
        if !url::Url::from_file_path(path).is_ok_and(|uri| uri.as_str() == pending.uri) {
            return;
        }

        if !panel
            .session
            .as_ref()
            .and_then(|session| session.document.as_ref())
            .is_some_and(|doc| doc.uri == pending.uri && doc.version == pending.version)
        {
            return;
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            panel.suggestions = None;
            return;
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
        {
            panel.completion_index = (panel.completion_index + 1) % items.len();
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
            panel.completion_index = (panel.completion_index + items.len() - 1) % items.len();
        }
        if context.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
        }) {
            panel.selected_completion = Some(panel.completion_index);
        }
    }

    pub fn inline_completion(
        &mut self,
        context: &egui::Context,
        path: &Path,
        position: egui::Pos2,
        editor: egui::Id,
        byte: usize,
    ) {
        let Some(index) = self.slot(path) else { return };
        let panel = &mut self.slots[index].panel;
        let Some((pending, items)) = &panel.suggestions else {
            return;
        };
        if !url::Url::from_file_path(path).is_ok_and(|uri| uri.as_str() == pending.uri) {
            return;
        }

        if pending.offset != byte {
            panel.suggestions = None;
            return;
        }
        if !panel
            .session
            .as_ref()
            .and_then(|session| session.document.as_ref())
            .is_some_and(|doc| doc.uri == pending.uri && doc.version == pending.version)
        {
            return;
        }
        egui::Area::new(editor.with("completion-popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .constrain(true)
            .show(context, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(310.0);
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for (index, item) in items.iter().enumerate() {
                                let label = item["label"].as_str().unwrap_or("Completion");
                                let response =
                                    ui.selectable_label(panel.completion_index == index, label);
                                if response.clicked() {
                                    panel.selected_completion = Some(index);
                                    context.memory_mut(|memory| memory.request_focus(editor));
                                }
                            }
                        });
                });
            });
    }

    pub fn request_at(&mut self, feature: Feature, path: &Path, text: &str, byte: usize) {
        if let Some(index) = self.slot(path) {
            self.selected = index;
            self.slots[index]
                .panel
                .request_at(feature, path, text, byte);
        }
    }

    pub fn take_preview(
        &mut self,
        roots: &[PathBuf],
        tabs: &[OpenFile],
    ) -> Option<io::Result<Preview>> {
        self.slots
            .iter_mut()
            .find_map(|slot| slot.panel.take_preview(roots, tabs))
    }

    pub fn diagnostics(&self, path: &Path) -> &[(usize, String)] {
        self.slot(path)
            .and_then(|index| {
                let slot = &self.slots[index];
                slot.last
                    .as_ref()
                    .filter(|(last, _)| last == path)
                    .map(|_| slot.panel.marks.as_slice())
            })
            .unwrap_or(&[])
    }
}

struct Document {
    uri: String,
    text: String,
    version: i32,
}

#[derive(Clone)]
pub enum Feature {
    Hover,
    Completion,
    Definition,
    References,
    CodeActions,
    Format,
    Rename(String),
}

struct Pending {
    id: u64,
    uri: String,
    version: i32,
    offset: usize,
    started: Instant,
    feature: Feature,
    attempt: u8,
}

struct Suggestion {
    context: Pending,
    item: Value,
}

struct Session {
    process: Process,
    document: Option<Document>,
    launched: Instant,
    ready: bool,
    sync: u64,
    hover: bool,
    completion: bool,
    rename: bool,
    definition: bool,
    formatting: bool,
    references: bool,
    code_actions: bool,
    next_id: u64,
    next_version: i32,
    pending: Option<Pending>,
    stopping: Option<Instant>,
}

#[derive(Default)]
pub struct Panel {
    executable: Option<PathBuf>,
    args: String,
    session: Option<Session>,
    diagnostics: Vec<String>,
    marks: Vec<(usize, String)>,
    locations: Vec<Value>,
    code_actions: Vec<Value>,
    action_context: Option<Pending>,
    selected_action: Option<usize>,
    retry: Option<(Pending, Instant)>,
    notice: String,
    hover_text: String,
    rename_to: String,
    pub action: Option<Feature>,
    suggestions: Option<(Pending, Vec<Value>)>,
    rename_result: Option<Suggestion>,
    selected_completion: Option<usize>,
    completion_index: usize,
    pub destination: Option<(PathBuf, usize)>,
}

impl Panel {
    pub fn show(&mut self, ui: &mut egui::Ui, root: &Path) {
        if let Some(session) = &mut self.session {
            if session.stopping.is_some() {
                ui.label("Stopping language server…");
            } else if ui.button("Stop Language Server").clicked() {
                match session.process.send(&request(2, "shutdown", Value::Null)) {
                    Ok(()) => session.stopping = Some(Instant::now()),
                    Err(error) => self.fail(error),
                }
            }
        } else {
            ui.label("The selected language server runs with your OS permissions in this folder.");
            if ui.button("Choose executable…").clicked() {
                self.executable = rfd::FileDialog::new()
                    .pick_file()
                    .or(self.executable.take());
            }
            if let Some(path) = &self.executable {
                ui.label(path.to_string_lossy());
                ui.add(
                    TextEdit::multiline(&mut self.args)
                        .desired_rows(2)
                        .hint_text("Arguments, one per line"),
                );
                if ui.button("Start Language Server").clicked() {
                    let args = self
                        .args
                        .lines()
                        .filter(|line| !line.is_empty())
                        .map(OsString::from)
                        .collect::<Vec<_>>();
                    match Session::start(path, &args, root, ui.ctx()) {
                        Ok(session) => {
                            self.session = Some(session);
                            self.notice = "Initializing…".into();
                        }
                        Err(error) => self.fail(error),
                    }
                }
            }
        }

        if self
            .session
            .as_ref()
            .is_some_and(|session| session.ready && session.stopping.is_none())
        {
            let session = self.session.as_ref().expect("ready session");
            if session.hover && ui.button("Inspect symbol at cursor").clicked() {
                self.action = Some(Feature::Hover);
            }
            if session.completion && ui.button("Show completions").clicked() {
                self.action = Some(Feature::Completion);
            }
            if session.definition && ui.button("Go to definition").clicked() {
                self.action = Some(Feature::Definition);
            }
            if session.references && ui.button("Find references").clicked() {
                self.action = Some(Feature::References);
            }
            if session.code_actions && ui.button("Code actions").clicked() {
                self.action = Some(Feature::CodeActions);
            }
            if session.formatting && ui.button("Preview formatting").clicked() {
                self.action = Some(Feature::Format);
            }
            if session.rename {
                ui.add(TextEdit::singleline(&mut self.rename_to).hint_text("New symbol name"));
                if ui
                    .add_enabled(
                        !self.rename_to.trim().is_empty(),
                        egui::Button::new("Preview rename"),
                    )
                    .clicked()
                {
                    self.action = Some(Feature::Rename(self.rename_to.trim().to_owned()));
                }
            }
        }

        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
        ScrollArea::vertical()
            .id_salt("language-service")
            .show(ui, |ui| {
                if !self.hover_text.is_empty() {
                    ui.heading("Symbol information");
                    ui.label(&self.hover_text);
                }
                if let Some((_, suggestions)) = &self.suggestions {
                    ui.heading("Completions");
                    for (index, item) in suggestions.iter().enumerate() {
                        if let Some(label) = item["label"].as_str() {
                            if ui
                                .button(label.chars().take(200).collect::<String>())
                                .clicked()
                            {
                                self.selected_completion = Some(index);
                            }
                        }
                    }
                }
                for location in &self.locations {
                    if let Some((path, line)) = definition_target(location) {
                        if ui.button(format!("{} · {line}", path.display())).clicked() {
                            self.destination = Some((path, line));
                        }
                    }
                }
                for (index, action) in self.code_actions.iter().enumerate() {
                    let title = action["title"].as_str().unwrap_or("Code action");
                    let supported = action["edit"].is_object() && action["command"].is_null() && action["disabled"].is_null();
                    if ui.add_enabled(supported, egui::Button::new(title)).on_hover_text("Workspace edits are reviewed before applying. Commands require an extension.").clicked() {
                        self.selected_action = Some(index);
                    }
                }
                if !self.diagnostics.is_empty() {
                    ui.heading("Diagnostics");
                    for diagnostic in &self.diagnostics {
                        ui.label(diagnostic);
                    }
                }
            });
    }

    pub fn tick(&mut self, context: &egui::Context, file: Option<(&Path, &str)>) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        context.request_repaint_after(Duration::from_millis(150));
        if session
            .stopping
            .is_some_and(|start| start.elapsed() > Duration::from_secs(2))
        {
            self.session = None;
            self.notice = "Language server stopped.".into();
            return;
        }
        if !session.ready && session.launched.elapsed() > Duration::from_secs(10) {
            self.fail(io::Error::new(
                io::ErrorKind::TimedOut,
                "Language server initialization timed out.",
            ));
            return;
        }

        let result = (|| -> io::Result<()> {
            if session.ready && session.stopping.is_none() && session.sync_file(file)? {
                self.diagnostics.clear();
                self.marks.clear();
                self.locations.clear();
                self.code_actions.clear();
                self.action_context = None;
                self.retry = None;
                self.hover_text.clear();
                self.suggestions = None;
            }
            if session
                .pending
                .as_ref()
                .is_some_and(|request| request.started.elapsed() > Duration::from_secs(4))
            {
                if let Some(pending) = session.pending.take() {
                    session
                        .process
                        .send(&notification("$/cancelRequest", json!({"id": pending.id})))?;
                }
                self.notice = "Language request timed out.".into();
            }
            for _ in 0..16 {
                let Some(message) = session.process.poll()? else {
                    break;
                };
                if message["id"] == 1 && message["method"].is_null() {
                    if let Some(error) = message.get("error") {
                        return Err(io::Error::other(format!("Initialization failed: {error}")));
                    }
                    let capabilities = &message["result"]["capabilities"];
                    if !capabilities["positionEncoding"].is_null()
                        && capabilities["positionEncoding"] != "utf-16"
                    {
                        return Err(io::Error::other(
                            "Language server did not accept UTF-16 positions.",
                        ));
                    }
                    let synchronization = &capabilities["textDocumentSync"];
                    session.sync = synchronization
                        .as_u64()
                        .or_else(|| synchronization["change"].as_u64())
                        .unwrap_or(0);
                    session.hover = capabilities["hoverProvider"]
                        .as_bool()
                        .unwrap_or(capabilities["hoverProvider"].is_object());
                    session.completion = capabilities["completionProvider"].is_object();
                    session.definition = capabilities["definitionProvider"] == true
                        || capabilities["definitionProvider"].is_object();
                    session.formatting = capabilities["documentFormattingProvider"] == true
                        || capabilities["documentFormattingProvider"].is_object();
                    session.references = capabilities["referencesProvider"] == true
                        || capabilities["referencesProvider"].is_object();
                    session.code_actions = capabilities["codeActionProvider"] == true
                        || capabilities["codeActionProvider"].is_object();
                    session.rename = capabilities["renameProvider"] == true
                        || capabilities["renameProvider"].is_object();
                    session
                        .process
                        .send(&notification("initialized", json!({})))?;
                    session.ready = true;
                    self.notice = "Language server connected.".into();
                } else if message["id"] == 2
                    && message["method"].is_null()
                    && session.stopping.is_some()
                {
                    session.process.send(&notification("exit", Value::Null))?;
                } else if message["method"] == "textDocument/publishDiagnostics" {
                    let params = &message["params"];
                    if session.document.as_ref().is_some_and(|document| {
                        params["uri"] == document.uri
                            && (params["version"].is_null()
                                || params["version"] == document.version)
                    }) {
                        self.marks = params["diagnostics"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .take(100)
                            .filter_map(|diagnostic| {
                                Some((
                                    diagnostic["range"]["start"]["line"].as_u64()? as usize + 1,
                                    diagnostic["message"].as_str()?.chars().take(1000).collect(),
                                ))
                            })
                            .collect();
                        self.diagnostics = params["diagnostics"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .take(100)
                            .map(|diagnostic| {
                                let line = diagnostic["range"]["start"]["line"]
                                    .as_u64()
                                    .unwrap_or(0)
                                    .saturating_add(1);
                                let message = diagnostic["message"]
                                    .as_str()
                                    .unwrap_or("")
                                    .chars()
                                    .take(1000)
                                    .collect::<String>();
                                let revision = if params["version"].is_null() {
                                    " (unversioned)"
                                } else {
                                    ""
                                };
                                format!("Line {line}{revision}: {message}")
                            })
                            .collect();
                    }
                } else if session.pending.as_ref().is_some_and(|pending| {
                    message["id"] == pending.id && message["method"].is_null()
                }) {
                    let pending = session.pending.take().expect("matched pending request");
                    if !session.document.as_ref().is_some_and(|document| {
                        document.uri == pending.uri && document.version == pending.version
                    }) {
                        self.notice =
                            "Document changed while waiting for the language server.".into();
                        continue;
                    }
                    if let Some(error) = message.get("error") {
                        if matches!(error["code"].as_i64(), Some(-32801 | -32802))
                            && pending.attempt < 2
                        {
                            self.retry = Some((pending, Instant::now()));
                            self.notice = "Language analysis changed. Retrying…".into();
                            context.request_repaint_after(Duration::from_millis(120));
                            continue;
                        }
                        self.notice = error["message"]
                            .as_str()
                            .unwrap_or("Language server rejected the request.")
                            .chars()
                            .take(1000)
                            .collect();
                        continue;
                    }
                    self.notice = "Language server connected.".into();
                    let item = message["result"].clone();
                    match pending.feature {
                        Feature::Hover => self.hover_text = hover_content(&item["contents"]),
                        Feature::Completion => {
                            let items = item.as_array().or_else(|| item["items"].as_array());
                            let items = items
                                .into_iter()
                                .flatten()
                                .take(60)
                                .cloned()
                                .collect::<Vec<_>>();
                            if items.is_empty() {
                                self.notice = "No completions at the cursor.".into();
                            } else {
                                self.completion_index = 0;
                                self.suggestions = Some((pending, items));
                            }
                        }
                        Feature::References => {
                            self.locations = item
                                .as_array()
                                .into_iter()
                                .flatten()
                                .take(200)
                                .cloned()
                                .collect();
                        }
                        Feature::CodeActions => {
                            self.code_actions = item
                                .as_array()
                                .into_iter()
                                .flatten()
                                .take(60)
                                .cloned()
                                .collect();
                            self.action_context = Some(pending);
                        }
                        Feature::Definition => match definition_target(&item) {
                            Some(destination) => self.destination = Some(destination),
                            None => self.notice = "No local definition at the cursor.".into(),
                        },
                        Feature::Format => {
                            let edits = json!({"changes": { (&pending.uri): item }});
                            self.rename_result = Some(Suggestion {
                                context: pending,
                                item: edits,
                            });
                        }
                        Feature::Rename(_) => {
                            self.rename_result = Some(Suggestion {
                                context: pending,
                                item,
                            })
                        }
                    }
                } else if message["method"].is_string() && !message["id"].is_null() {
                    let response = match message["method"].as_str() {
                        Some("workspace/configuration") => {
                            let count = message["params"]["items"]
                                .as_array()
                                .map_or(0, Vec::len)
                                .min(1000);
                            json!({"result": vec![Value::Null; count]})
                        }
                        Some("window/workDoneProgress/create") => json!({"result": null}),
                        _ => {
                            json!({"error": {"code": -32601, "message": "Client method unavailable"}})
                        }
                    };
                    let mut response = response;
                    response["jsonrpc"] = json!("2.0");
                    response["id"] = message["id"].clone();
                    session.process.send(&response)?;
                }
            }
            Ok(())
        })();

        if let Err(error) = result {
            if session.stopping.is_some() {
                self.session = None;
                self.notice = "Language server stopped.".into();
            } else {
                self.fail(error);
            }
        }
        if self
            .retry
            .as_ref()
            .is_some_and(|(_, due)| due.elapsed() >= Duration::from_millis(120))
        {
            if let Some((pending, _)) = self.retry.take() {
                let document = self
                    .session
                    .as_ref()
                    .and_then(|session| session.document.as_ref())
                    .filter(|document| {
                        document.uri == pending.uri && document.version == pending.version
                    })
                    .map(|document| document.text.clone());
                if let Some(text) = document {
                    if let Some(path) = url::Url::parse(&pending.uri)
                        .ok()
                        .and_then(|uri| uri.to_file_path().ok())
                    {
                        self.request_impl(
                            pending.feature,
                            &path,
                            &text,
                            pending.offset,
                            pending.attempt + 1,
                        );
                    }
                }
            }
        }
    }

    pub fn request_at(&mut self, feature: Feature, path: &Path, text: &str, byte: usize) {
        self.retry = None;
        self.request_impl(feature, path, text, byte, 0);
    }

    fn request_impl(
        &mut self,
        feature: Feature,
        path: &Path,
        text: &str,
        byte: usize,
        attempt: u8,
    ) {
        let Some(session) = &mut self.session else {
            return;
        };
        let Some(document) = &session.document else {
            self.notice = "Open a supported source file and wait for it to sync.".into();
            return;
        };
        if session.pending.is_some() || session.stopping.is_some() {
            self.notice = "Wait for the current language request to finish.".into();
            return;
        }
        if text != document.text
            || !url::Url::from_file_path(path).is_ok_and(|url| url.as_str() == document.uri)
        {
            self.notice = "The active document is still syncing.".into();
            return;
        }
        let Ok(position) = LineIndex::new(text).from_byte(byte, Encoding::Utf16) else {
            return;
        };
        let mut params = json!({
            "textDocument": {"uri": document.uri},
            "position": {"line": position.line, "character": position.character}
        });
        let method = match &feature {
            Feature::Hover if session.hover => "textDocument/hover",
            Feature::Completion if session.completion => "textDocument/completion",
            Feature::Definition if session.definition => "textDocument/definition",
            Feature::References if session.references => {
                params["context"] = json!({"includeDeclaration": true});
                "textDocument/references"
            }
            Feature::CodeActions if session.code_actions => {
                params["range"] = json!({"start": params["position"], "end": params["position"]});
                params["context"] = json!({"diagnostics": []});
                params
                    .as_object_mut()
                    .expect("code action params")
                    .remove("position");
                "textDocument/codeAction"
            }
            Feature::Format if session.formatting => {
                params
                    .as_object_mut()
                    .expect("format params")
                    .remove("position");
                params["options"] = json!({"tabSize": 4, "insertSpaces": true});
                "textDocument/formatting"
            }
            Feature::Rename(name) if session.rename => {
                params["newName"] = json!(name);
                "textDocument/rename"
            }
            _ => return,
        };
        let id = session.next_id;
        session.next_id += 1;
        match session.process.send(&request(id, method, params)) {
            Ok(()) => {
                self.suggestions = None;
                session.pending = Some(Pending {
                    id,
                    uri: document.uri.clone(),
                    version: document.version,
                    offset: byte,
                    started: Instant::now(),
                    feature,
                    attempt,
                });
                self.notice = "Waiting for language server…".into();
            }
            Err(error) => self.fail(error),
        }
    }

    pub fn take_preview(
        &mut self,
        roots: &[PathBuf],
        tabs: &[OpenFile],
    ) -> Option<io::Result<Preview>> {
        let document = self.session.as_ref()?.document.as_ref()?;
        if let Some(selected) = self.selected_action.take() {
            let pending = self.action_context.as_ref()?;
            if pending.uri == document.uri && pending.version == document.version {
                let action = self.code_actions.get(selected)?;
                return Some(
                    language_edit::rename(
                        &action["edit"],
                        roots,
                        (&document.uri, document.version),
                        tabs,
                    )
                    .map(|mut preview| {
                        preview.title = "Review code action";
                        preview
                    }),
                );
            }
        }
        if let Some(suggestion) = self.rename_result.take() {
            if document.uri == suggestion.context.uri
                && document.version == suggestion.context.version
            {
                return Some(
                    language_edit::rename(
                        &suggestion.item,
                        roots,
                        (&document.uri, document.version),
                        tabs,
                    )
                    .map(|mut preview| {
                        if matches!(suggestion.context.feature, Feature::Format) {
                            preview.title = "Review formatting";
                        }
                        preview
                    }),
                );
            }
        }
        let selected = self.selected_completion.take()?;
        let (pending, items) = self.suggestions.as_ref()?;
        if document.uri != pending.uri || document.version != pending.version {
            return None;
        }
        let path = url::Url::parse(&document.uri).ok()?.to_file_path().ok()?;
        let item = items.get(selected)?;
        Some(language_edit::completion(
            path,
            &document.text,
            item,
            pending.offset,
        ))
    }

    fn fail(&mut self, error: io::Error) {
        self.session = None;
        self.diagnostics.clear();
        self.marks.clear();
        self.locations.clear();
        self.code_actions.clear();
        self.action_context = None;
        self.hover_text.clear();
        self.suggestions = None;
        self.rename_result = None;
        self.notice = error.to_string();
    }
}

impl Session {
    fn start(
        program: &Path,
        args: &[OsString],
        root: &Path,
        context: &egui::Context,
    ) -> io::Result<Self> {
        let root_uri = url::Url::from_directory_path(root)
            .map_err(|_| io::Error::other("Invalid workspace path."))?;
        let process = Process::spawn(program, args, root, context)?;
        process.send(&request(
            1,
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": root_uri.as_str(),
                "capabilities": {
                    "general": { "positionEncodings": ["utf-16"] },
                    "workspace": {"workspaceEdit": {"documentChanges": true}},
                    "textDocument": {
                        "publishDiagnostics": {"versionSupport": true},
                        "completion": {"completionItem": {"snippetSupport": false}}
                    }
                },
                "clientInfo": {"name": "ForgeIDE", "version": env!("CARGO_PKG_VERSION")}
            }),
        ))?;
        Ok(Self {
            process,
            document: None,
            launched: Instant::now(),
            ready: false,
            sync: 0,
            hover: false,
            completion: false,
            rename: false,
            definition: false,
            formatting: false,
            references: false,
            code_actions: false,
            next_id: 100,
            next_version: 1,
            pending: None,
            stopping: None,
        })
    }

    fn sync_file(&mut self, file: Option<(&Path, &str)>) -> io::Result<bool> {
        let file = file.and_then(|(path, text)| {
            let language = match path.extension()?.to_str()? {
                "rs" => "rust",
                "ts" | "mts" | "cts" => "typescript",
                "tsx" => "typescriptreact",
                "js" | "mjs" | "cjs" => "javascript",
                "jsx" => "javascriptreact",
                "py" => "python",
                "c" | "h" => "c",
                "cpp" | "hpp" | "cc" => "cpp",
                "go" => "go",
                "json" => "json",
                _ => return None,
            };
            Some((url::Url::from_file_path(path).ok()?, text, language))
        });
        if let Some(previous) = &self.document {
            if file
                .as_ref()
                .is_none_or(|(url, _, _)| url.as_str() != previous.uri)
            {
                self.process.send(&notification(
                    "textDocument/didClose",
                    json!({
                        "textDocument": {"uri": previous.uri}
                    }),
                ))?;
                self.document = None;
                if file.is_none() {
                    return Ok(true);
                }
            }
        }
        let Some((uri, text, language)) = file else {
            return Ok(false);
        };
        if let Some(previous) = &mut self.document {
            if previous.text == text || self.sync == 0 {
                return Ok(false);
            }
            let version = self.next_version;
            self.next_version = next_version(version)?;
            let content = if self.sync == 2 {
                let end = LineIndex::new(&previous.text)
                    .from_byte(previous.text.len(), Encoding::Utf16)
                    .map_err(io::Error::other)?;
                json!({
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": end.line, "character": end.character}},
                    "text": text
                })
            } else {
                json!({ "text": text })
            };
            self.process.send(&notification(
                "textDocument/didChange",
                json!({
                    "textDocument": {"uri": previous.uri, "version": version},
                    "contentChanges": [content]
                }),
            ))?;
            previous.version = version;
            previous.text = text.to_owned();
        } else {
            let uri = uri.to_string();
            let version = self.next_version;
            self.next_version = next_version(version)?;
            self.process.send(&notification("textDocument/didOpen", json!({
                "textDocument": {"uri": uri, "languageId": language, "version": version, "text": text}
            })))?;
            self.document = Some(Document {
                uri,
                version,
                text: text.to_owned(),
            });
        }
        Ok(true)
    }
}

fn next_version(version: i32) -> io::Result<i32> {
    version
        .checked_add(1)
        .ok_or_else(|| io::Error::other("Restart the language server to reset document versions."))
}

fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params })
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn definition_target(value: &Value) -> Option<(PathBuf, usize)> {
    let location = if let Some(locations) = value.as_array() {
        locations.first()?
    } else {
        value
    };
    let uri = location["targetUri"]
        .as_str()
        .or_else(|| location["uri"].as_str())?;
    let range = location
        .get("targetSelectionRange")
        .unwrap_or(&location["range"]);
    let line = usize::try_from(range["start"]["line"].as_u64()?)
        .ok()?
        .checked_add(1)?;
    let path = url::Url::parse(uri)
        .ok()?
        .to_file_path()
        .ok()?
        .canonicalize()
        .ok()?;
    Some((path, line))
}

fn hover_content(value: &Value) -> String {
    fn segment(value: &Value) -> String {
        value
            .as_str()
            .or_else(|| value["value"].as_str())
            .unwrap_or("")
            .chars()
            .take(4000)
            .collect()
    }
    if let Some(segments) = value.as_array() {
        segments
            .iter()
            .take(8)
            .map(segment)
            .collect::<Vec<_>>()
            .join("\n\n")
    } else if value.is_null() {
        "No symbol information at the cursor.".into()
    } else {
        segment(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_rename_and_completion(
        mut panel: Panel,
        context: &egui::Context,
        root: &Path,
        source: &Path,
        text: &str,
    ) -> io::Result<()> {
        let position = text.find("forge_answer").expect("fixture symbol");
        panel.request_at(Feature::References, source, text, position);
        let references_started = Instant::now();
        while panel
            .session
            .as_ref()
            .is_some_and(|session| session.pending.is_some())
        {
            panel.tick(context, Some((source, text)));
            assert!(
                references_started.elapsed() < Duration::from_secs(5),
                "{}",
                panel.notice
            );
            std::thread::sleep(Duration::from_millis(15));
        }
        assert!(
            panel.locations.len() >= 2,
            "Expected declaration and call references, {}",
            panel.notice
        );
        panel.request_at(
            Feature::Rename("renamed_answer".into()),
            source,
            text,
            position,
        );
        let start = Instant::now();
        let rename = loop {
            panel.tick(context, Some((source, text)));
            if let Some(edit) = panel.take_preview(&[root.to_owned()], &[]) {
                break edit?;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "{}", panel.notice);
            std::thread::sleep(Duration::from_millis(15));
        };
        assert!(
            rename
                .files
                .iter()
                .any(|file| file.after.matches("renamed_answer").count() >= 2)
        );

        let incomplete = text
            .replace("forge_answer(),", "forge_ans,")
            .replace("forge_answer();", "forge_ans;");
        let position = incomplete
            .find("forge_ans,")
            .or_else(|| incomplete.find("forge_ans;"))
            .expect("fixture call")
            + "forge_ans".len();
        panel.tick(context, Some((source, &incomplete)));
        panel.request_at(Feature::Completion, source, &incomplete, position);
        let start = Instant::now();
        loop {
            panel.tick(context, Some((source, &incomplete)));
            if let Some((_, options)) = &panel.suggestions {
                if let Some(index) = options.iter().position(|option| {
                    option["label"]
                        .as_str()
                        .is_some_and(|label| label.starts_with("forge_answer"))
                }) {
                    panel.selected_completion = Some(index);
                    let completion = panel
                        .take_preview(&[root.to_owned()], &[])
                        .expect("selected completion")?;
                    assert!(completion.files[0].after.matches("forge_answer").count() >= 2);
                    return Ok(());
                }
            }
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "{}: expected forge_answer()",
                panel.notice
            );
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    #[test]
    #[ignore = "requires both language servers and their fixture roots"]
    fn talks_to_real_parallel_language_servers() -> io::Result<()> {
        let rust = PathBuf::from(std::env::var_os("FORGE_TEST_LSP").unwrap());
        let rust_root =
            PathBuf::from(std::env::var_os("FORGE_TEST_LSP_ROOT").unwrap()).canonicalize()?;
        let typescript = PathBuf::from(std::env::var_os("FORGE_TEST_TYPESCRIPT_LSP").unwrap());
        let ts_root = PathBuf::from(std::env::var_os("FORGE_TEST_TYPESCRIPT_ROOT").unwrap())
            .canonicalize()?;
        let tabs = vec![
            OpenFile::open(&rust_root.join("src/main.rs"))?,
            OpenFile::open(&ts_root.join("main.ts"))?,
        ];
        let context = egui::Context::default();
        let mut hub = Hub::default();
        hub.slots[0].panel.session = Some(Session::start(&rust, &[], &rust_root, &context)?);
        hub.slots[1].panel.session = Some(Session::start(
            &typescript,
            &["--stdio".into()],
            &ts_root,
            &context,
        )?);
        hub.tick(&context, Some((&tabs[0].path, &tabs[0].text)), &tabs);
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut requested = false;
        let mut last_request = Instant::now() - Duration::from_secs(2);
        while Instant::now() < deadline {
            hub.tick(&context, Some((&tabs[1].path, &tabs[1].text)), &tabs);
            for slot in &hub.slots[..2] {
                assert!(slot.panel.session.is_some(), "{}", slot.panel.notice);
            }
            if (!requested || last_request.elapsed() >= Duration::from_secs(1))
                && hub.slots[..2].iter().all(|slot| {
                    slot.panel
                        .session
                        .as_ref()
                        .is_some_and(|session| session.ready && session.document.is_some())
                })
            {
                for tab in &tabs {
                    hub.request_at(
                        Feature::Hover,
                        &tab.path,
                        &tab.text,
                        tab.text.find("forge_answer").unwrap(),
                    );
                }
                requested = true;
                last_request = Instant::now();
            }
            if requested
                && hub.slots[..2]
                    .iter()
                    .all(|slot| slot.panel.hover_text.contains("forge_answer"))
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        Err(io::Error::other(format!(
            "Parallel hover failed. Rust {} {:?}; TypeScript {} {:?}",
            hub.slots[0].panel.notice,
            hub.slots[0].panel.hover_text,
            hub.slots[1].panel.notice,
            hub.slots[1].panel.hover_text,
        )))
    }

    #[test]
    #[ignore = "requires an installed TypeScript Language Server wrapper"]
    fn talks_to_real_typescript_server() -> io::Result<()> {
        let server = PathBuf::from(std::env::var_os("FORGE_TEST_TYPESCRIPT_LSP").unwrap());
        let root = PathBuf::from(std::env::var_os("FORGE_TEST_TYPESCRIPT_ROOT").unwrap())
            .canonicalize()?;
        let source = root.join("main.ts");
        let text = std::fs::read_to_string(&source)?;
        let context = egui::Context::default();
        let mut panel = Panel {
            session: Some(Session::start(
                &server,
                &["--stdio".into()],
                &root,
                &context,
            )?),
            ..Panel::default()
        };
        let position = text.find("forge_answer()").unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut candidate = Instant::now();
        while Instant::now() < deadline {
            panel.tick(&context, Some((&source, &text)));
            assert!(panel.session.is_some(), "{}", panel.notice);
            if panel.hover_text.contains("function forge_answer") {
                return check_rename_and_completion(panel, &context, &root, &source, &text);
            }
            if candidate.elapsed() > Duration::from_secs(1) {
                panel.request_at(Feature::Hover, &source, &text, position);
                candidate = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("{}; hover: {}", panel.notice, panel.hover_text);
    }

    #[test]
    #[ignore = "requires a Rust Analyzer executable and an isolated Rust fixture"]
    fn talks_to_real_rust_analyzer() -> io::Result<()> {
        let analyzer = PathBuf::from(std::env::var_os("FORGE_TEST_LSP").unwrap());
        let root =
            PathBuf::from(std::env::var_os("FORGE_TEST_LSP_ROOT").unwrap()).canonicalize()?;
        let source = root.join("src/main.rs");
        let text = std::fs::read_to_string(&source)?;
        let context = egui::Context::default();
        let mut panel = Panel {
            session: Some(Session::start(&analyzer, &[], &root, &context)?),
            ..Panel::default()
        };
        let position = text.find("forge_answer()").unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut candidate = Instant::now();
        while Instant::now() < deadline {
            panel.tick(&context, Some((&source, &text)));
            assert!(panel.session.is_some(), "{}", panel.notice);
            if panel.hover_text.contains("fn forge_answer") {
                return check_rename_and_completion(panel, &context, &root, &source, &text);
            }
            if candidate.elapsed() > Duration::from_secs(1) {
                panel.request_at(Feature::Hover, &source, &text, position);
                candidate = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("{}; hover: {}", panel.notice, panel.hover_text);
    }
}
