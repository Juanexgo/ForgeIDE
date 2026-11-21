mod accounts;
mod chrome;
mod conflicts;
mod debugger;
mod extension;
mod extension_manager;
mod extension_registry;
mod extension_vsix;
mod file;
mod find;
mod folding;
mod git;
mod history;
mod language_edit;
mod lsp;
mod multicursor;
mod overview;
mod packages;
mod project;
mod protocol;
mod recovery;
mod search;
mod session;
mod settings;
mod snippets;
mod syntax;
mod terminal;
mod testing;
mod workbench;
mod workspace_files;

use eframe::egui::{
    self, Color32, FontId, Frame, Key, Modifiers, RichText, ScrollArea, TextEdit, text::LayoutJob,
};
use file::OpenFile;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

use chrome::{
    Icon, theme_accent, theme_border, theme_editor, theme_elevated, theme_green, theme_muted,
    theme_panel, theme_rail, theme_red, theme_text,
};

fn main() -> eframe::Result {
    let input = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if input.as_slice() == [PathBuf::from("--extension-worker")] {
        if let Err(error) = extension::worker() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if input.first().is_some_and(|argument| {
        argument.as_path() == Path::new("--extension-inspect")
            || argument.as_path() == Path::new("--extension-package")
    }) {
        let result = match input.as_slice() {
            [flag, path] if flag.as_path() == Path::new("--extension-inspect") => {
                packages::inspect_file(path).and_then(|report| {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                    Ok(())
                })
            }
            [flag, source, output] if flag.as_path() == Path::new("--extension-package") => {
                packages::package_native(source, output)
            }
            _ => Err(std::io::Error::other(
                "Usage: forgeide --extension-inspect PACKAGE | --extension-package MANIFEST OUTPUT.forge-extension",
            )),
        };
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    eframe::run_native(
        "ForgeIDE",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title(format!("ForgeIDE {}", env!("CARGO_PKG_VERSION")))
                .with_inner_size([1180.0, 760.0])
                .with_min_inner_size([640.0, 420.0]),
            ..Default::default()
        },
        Box::new(move |context| {
            chrome::apply(&context.egui_ctx);
            let mut app = Editor {
                restore_session: input.is_empty(),
                preferences: settings::Preferences::load(),
                packages: packages::Library::load(),
                accounts: accounts::Panel::load(),
                ..Editor::default()
            };
            app.preferences.apply(&context.egui_ctx);
            if app.restore_session {
                app.restore_workspace(&context.egui_ctx);
            }
            for path in input {
                if path.is_dir() {
                    if app.root.is_some() {
                        app.add_folder(path, &context.egui_ctx);
                    } else {
                        app.open_folder(path, &context.egui_ctx);
                    }
                } else {
                    app.open_file(&path);
                }
            }
            Ok(Box::new(app))
        }),
    )
}

#[derive(Default)]
struct Editor {
    restore_session: bool,
    preferences: settings::Preferences,
    settings_open: bool,
    tasks: project::Tasks,
    tests: testing::Panel,
    packages: packages::Library,
    accounts: accounts::Panel,
    accounts_open: bool,
    problems_open: bool,
    panel_focus: bool,
    debug_location: Option<(PathBuf, usize)>,
    workspace_files: workspace_files::Controller,
    session_saved: bool,
    disk_scan: Option<mpsc::Receiver<Vec<DiskObservation>>>,
    last_disk_scan: Option<std::time::Instant>,
    root: Option<PathBuf>,
    extra_roots: Vec<PathBuf>,
    tools_root: Option<PathBuf>,
    toolsets: std::collections::HashMap<PathBuf, Toolset>,
    sidebar: Sidebar,
    folder_state: std::collections::HashMap<PathBuf, bool>,
    palette: workbench::Palette,
    hide_minimap: bool,
    cursor: Option<(usize, usize)>,
    entries: Vec<PathBuf>,
    directories: Vec<PathBuf>,
    scan: Option<mpsc::Receiver<FileListing>>,
    filter: String,
    tabs: Vec<OpenFile>,
    active: Option<usize>,
    split: Option<SplitEditor>,
    picking_split: bool,
    status: Option<(String, bool)>,
    pending_close: Option<CloseTarget>,
    layout_cache: [Option<(PathBuf, LayoutJob)>; 2],
    find: find::Panel,
    folds: std::collections::HashMap<PathBuf, folding::Folds>,
    workspace_query: String,
    workspace_search: Option<WorkspaceSearch>,
    workspace_regex: bool,
    workspace_ignore_case: bool,
    workspace_path_filter: String,
    workspace_replace: bool,
    workspace_replacement: String,
    workspace_review: Option<language_edit::Preview>,
    listing_incomplete: bool,
    search_results: Option<search::Results>,
    pending_jump: Option<usize>,
    recovery_due: Option<std::time::Instant>,
    terminal_open: bool,
    terminal: Vec<terminal::Terminal>,
    active_terminal: usize,
    git_panel: git::Panel,
    lsp_panel: lsp::Hub,
    language_preview: Option<language_edit::Preview>,
    undo_batch: Option<language_edit::Preview>,
    timeline: history::Timeline,
    history_open: bool,
    history_paths: Vec<PathBuf>,
    review_selection: std::collections::HashMap<PathBuf, Vec<bool>>,
    review_signature: u64,
    redo_batch: Option<language_edit::Preview>,
    multiple: Option<MultiEdit>,
    snippet: Option<snippets::Live>,
    extension_panel: extension::Panel,
    debugger: debugger::Panel,
}

#[derive(Default)]
struct Toolset {
    git: git::Panel,
    language: lsp::Hub,
    debugger: debugger::Panel,
    tasks: project::Tasks,
    tests: testing::Panel,
    extensions: extension::Panel,
}

#[derive(Clone, Copy)]
struct SplitEditor {
    companion: usize,
    active_right: bool,
    ratio: f32,
}

fn editor_id(path: &Path, right: bool) -> egui::Id {
    egui::Id::new(("editor-buffer", path, right))
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Sidebar {
    #[default]
    Files,
    Search,
    Git,
    Language,
    Debug,
    Tests,
    Extensions,
}

impl Sidebar {
    fn icon(self) -> Icon {
        match self {
            Self::Files => Icon::Files,
            Self::Search => Icon::Search,
            Self::Git => Icon::Git,
            Self::Language => Icon::Code,
            Self::Debug => Icon::Debug,
            Self::Tests => Icon::Tests,
            Self::Extensions => Icon::Extensions,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Files => "Explorer",
            Self::Search => "Search",
            Self::Git => "Source Control",
            Self::Language => "Language Server",
            Self::Debug => "Debugger",
            Self::Tests => "Testing",
            Self::Extensions => "Extensions",
        }
    }
}

struct MultiEdit {
    editor: egui::Id,
    selections: multicursor::Selections,
    original: Option<String>,
    started: std::time::Instant,
}

struct WorkspaceSearch {
    partial: bool,
    outcome: mpsc::Receiver<std::io::Result<search::Outcome>>,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for WorkspaceSearch {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

struct DiskObservation {
    path: PathBuf,
    expected: Option<file::DiskStamp>,
    actual: Option<file::DiskStamp>,
}

#[derive(Clone, Copy)]
enum CloseTarget {
    Tab(usize),
    Window,
}

struct FileListing {
    paths: Vec<PathBuf>,
    directories: Vec<PathBuf>,
    limited: bool,
    skipped: bool,
    error: Option<String>,
    complete: bool,
}

impl Editor {
    fn restore_workspace(&mut self, context: &egui::Context) {
        match session::load() {
            Ok(saved) => {
                if let Some(root) = saved
                    .root
                    .filter(|path| path.is_dir() && path.is_absolute())
                {
                    self.open_folder(root, context);
                }
                for root in saved
                    .extra_roots
                    .into_iter()
                    .filter(|path| path.is_dir() && path.is_absolute())
                {
                    self.add_folder(root, context);
                }
                for file in saved
                    .files
                    .into_iter()
                    .filter(|path| path.is_file() && path.is_absolute())
                {
                    self.open_file(&file);
                }
                if let Some(active) = saved.active {
                    self.active = self
                        .tabs
                        .iter()
                        .position(|file| file.path == active)
                        .or(self.active);
                }
                self.split = saved.split.and_then(|split| {
                    let companion = self
                        .tabs
                        .iter()
                        .position(|file| file.path == split.companion)?;
                    self.active.filter(|active| *active != companion)?;
                    Some(SplitEditor {
                        companion,
                        active_right: split.active_right,
                        ratio: split.ratio.clamp(0.25, 0.75),
                    })
                });
            }
            Err(error) => {
                self.status = Some((
                    format!("Workspace session could not be restored: {error}"),
                    true,
                ))
            }
        }
    }

    fn save_workspace(&mut self) {
        if !self.restore_session || self.session_saved {
            return;
        }
        let saved = session::WorkspaceSession {
            root: self.root.clone(),
            extra_roots: self.extra_roots.clone(),
            files: self
                .tabs
                .iter()
                .map(|file| file.path.clone())
                .take(500)
                .collect(),
            active: self.active.map(|index| self.tabs[index].path.clone()),
            split: self.split.map(|split| session::SplitSession {
                companion: self.tabs[split.companion].path.clone(),
                active_right: split.active_right,
                ratio: split.ratio,
            }),
        };
        if let Err(error) = session::save(&saved) {
            eprintln!("Could not save workspace session: {error}");
        }
        self.session_saved = true;
    }

    fn roots(&self) -> Vec<PathBuf> {
        self.root.iter().chain(&self.extra_roots).cloned().collect()
    }

    fn root_for(&self, path: &Path) -> Option<&Path> {
        self.root
            .iter()
            .chain(&self.extra_roots)
            .filter(|root| path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .map(PathBuf::as_path)
    }

    fn tool_root(&self) -> Option<&Path> {
        self.active
            .and_then(|index| self.root_for(&self.tabs[index].path))
            .or(self.root.as_deref())
    }

    fn sync_tool_root(&mut self) {
        let root = self.tool_root().map(Path::to_owned);
        if self.tools_root == root {
            return;
        }
        let previous = Toolset {
            git: std::mem::take(&mut self.git_panel),
            language: std::mem::take(&mut self.lsp_panel),
            debugger: std::mem::take(&mut self.debugger),
            tasks: std::mem::take(&mut self.tasks),
            tests: std::mem::take(&mut self.tests),
            extensions: std::mem::take(&mut self.extension_panel),
        };
        if let Some(previous_root) = self.tools_root.take() {
            self.toolsets.insert(previous_root, previous);
        }
        if let Some(root) = &root {
            if let Some(tools) = self.toolsets.remove(root) {
                self.git_panel = tools.git;
                self.lsp_panel = tools.language;
                self.debugger = tools.debugger;
                self.tasks = tools.tasks;
                self.tests = tools.tests;
                self.extension_panel = tools.extensions;
            }
        }
        self.tools_root = root;
    }

    fn open_folder(&mut self, root: PathBuf, context: &egui::Context) {
        let root = match root.canonicalize().and_then(|path| {
            if path.is_dir() {
                Ok(path)
            } else {
                Err(std::io::Error::other("Choose a project folder."))
            }
        }) {
            Ok(root) => root,
            Err(error) => {
                self.status = Some((error.to_string(), true));
                return;
            }
        };
        let replacing = self.root.as_ref() != Some(&root);
        if replacing {
            self.extra_roots.clear();
            self.folder_state.clear();
            self.language_preview = None;
        }
        self.folder_state.entry(root.clone()).or_insert(true);
        self.root = Some(root);
        self.sync_tool_root();
        if replacing {
            self.toolsets.clear();
        }
        self.scan_workspace(context);
    }

    fn add_folder(&mut self, path: PathBuf, context: &egui::Context) {
        if self.root.is_none() {
            self.open_folder(path, context);
            return;
        }
        let result = path.canonicalize().and_then(|path| {
            if !path.is_dir() {
                return Err(std::io::Error::other("Choose a project folder."));
            }
            if self.roots().len() >= 16 {
                return Err(std::io::Error::other(
                    "A workspace supports up to 16 folders.",
                ));
            }
            Ok(path)
        });
        match result {
            Ok(path) if !self.roots().contains(&path) => {
                self.folder_state.entry(path.clone()).or_insert(true);
                self.extra_roots.push(path);
                self.scan_workspace(context);
            }
            Ok(_) => {}
            Err(error) => self.status = Some((error.to_string(), true)),
        }
    }

    fn remove_folder(&mut self, path: &Path, context: &egui::Context) {
        if self.root.as_deref() == Some(path) {
            self.root = if self.extra_roots.is_empty() {
                None
            } else {
                Some(self.extra_roots.remove(0))
            };
        } else {
            self.extra_roots.retain(|root| root != path);
        }
        self.sync_tool_root();
        self.toolsets.remove(path);
        self.scan_workspace(context);
    }

    fn scan_workspace(&mut self, context: &egui::Context) {
        self.entries.clear();
        self.directories.clear();
        self.workspace_search = None;
        self.workspace_review = None;
        self.search_results = None;
        self.listing_incomplete = true;
        let roots = self.roots();
        if roots.is_empty() {
            self.scan = None;
            self.listing_incomplete = false;
            return;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        self.scan = Some(receiver);
        let context = context.clone();
        std::thread::spawn(move || {
            let mut paths = std::collections::BTreeSet::new();
            let mut directories = std::collections::BTreeSet::new();
            let mut limited = false;
            let mut skipped = false;
            let mut errors = Vec::new();
            for (index, root) in roots.iter().enumerate() {
                let list = list_files(root);
                paths.extend(list.paths);
                directories.extend(list.directories);
                limited |= list.limited || paths.len() + directories.len() > 10_000;
                skipped |= list.skipped;
                if let Some(error) = list.error {
                    errors.push(format!("{} — {error}", root.display()));
                }
                let complete = index + 1 == roots.len() || limited;
                if sender
                    .send(FileListing {
                        paths: paths.iter().take(10_000).cloned().collect(),
                        directories: directories.iter().take(10_000).cloned().collect(),
                        limited,
                        skipped,
                        error: (!errors.is_empty()).then(|| errors.join("\n")),
                        complete,
                    })
                    .is_err()
                {
                    break;
                }
                context.request_repaint();
                if complete {
                    break;
                }
            }
        });
    }

    fn open_file(&mut self, path: &Path) {
        if let Ok(canonical) = path.canonicalize() {
            if let Some(index) = self.tabs.iter().position(|tab| tab.path == canonical) {
                self.set_active(index);
                return;
            }
        }
        match OpenFile::open(path) {
            Ok(file) => {
                self.tabs.push(file);
                self.set_active(self.tabs.len() - 1);
                self.status = None;
            }
            Err(error) => self.status = Some((format!("{} — {error}", file_label(path)), true)),
        }
    }

    fn set_active(&mut self, index: usize) {
        if let (Some(active), Some(split)) = (self.active, self.split.as_mut()) {
            if split.companion == index {
                split.companion = active;
                split.active_right = !split.active_right;
            }
        }
        if self.active != Some(index) {
            self.multiple = None;
            self.snippet = None;
        }
        self.active = Some(index);
        self.find.reset_selection();
        let path = &self.tabs[index].path;
        if let Some(root) = self.root_for(path).map(Path::to_owned) {
            for parent in path
                .ancestors()
                .skip(1)
                .take_while(|parent| *parent != root)
            {
                self.folder_state.remove(parent);
            }
        }
    }

    fn toggle_split(&mut self, context: &egui::Context) {
        if self.split.take().is_some() {
            return;
        }
        let Some(active) = self.active else { return };
        if let Some(companion) = (0..self.tabs.len())
            .filter(|index| *index != active)
            .min_by_key(|index| index.abs_diff(active))
        {
            self.split = Some(SplitEditor {
                companion,
                active_right: false,
                ratio: 0.5,
            });
        } else if self.root.is_some() {
            self.show_quick_open();
            self.picking_split = true;
        } else if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.open_in_split(&path);
            context.request_repaint();
        }
    }

    fn open_in_split(&mut self, path: &Path) {
        let prior = self.active;
        let previous_split = self.split;
        self.open_file(path);
        if let (Some(companion), Some(active)) = (prior, self.active) {
            if companion != active {
                self.split = Some(SplitEditor {
                    companion,
                    active_right: !previous_split.is_some_and(|split| split.active_right),
                    ratio: previous_split.map_or(0.5, |split| split.ratio),
                });
            }
        }
    }

    fn watch_open_files(&mut self, context: &egui::Context) {
        if self.tabs.is_empty() {
            return;
        }
        if let Some(receiver) = &self.disk_scan {
            match receiver.try_recv() {
                Ok(results) => {
                    self.disk_scan = None;
                    for result in results {
                        if result.actual != result.expected {
                            if let Some(file) = self.tabs.iter_mut().find(|file| {
                                file.path == result.path
                                    && file.disk_stamp() == result.expected.as_ref()
                            }) {
                                file.external_change = true;
                            }
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => self.disk_scan = None,
            }
        }
        context.request_repaint_after(std::time::Duration::from_secs(2));
        if self
            .last_disk_scan
            .is_some_and(|last| last.elapsed() < std::time::Duration::from_secs(2))
        {
            return;
        }
        let files = self
            .tabs
            .iter()
            .map(|file| (file.path.clone(), file.disk_stamp().cloned()))
            .collect::<Vec<_>>();
        let (send, receive) = mpsc::sync_channel(1);
        let context = context.clone();
        self.disk_scan = Some(receive);
        self.last_disk_scan = Some(std::time::Instant::now());
        std::thread::spawn(move || {
            let results = files
                .into_iter()
                .map(|(path, expected)| {
                    let actual = file::DiskStamp::read(&path).ok();
                    DiskObservation {
                        path,
                        expected,
                        actual,
                    }
                })
                .collect();
            let _ = send.send(results);
            context.request_repaint();
        });
    }

    fn save_tab(&mut self, index: usize) -> bool {
        let file = &mut self.tabs[index];
        let name = file_label(&file.path);
        match file.save() {
            Ok(()) => {
                self.status = Some((format!("Saved {name}"), false));
                true
            }
            Err(error) => {
                self.status = Some((format!("{name} — {error}"), true));
                false
            }
        }
    }

    fn save_all(&mut self) {
        let mut saved = 0;
        let mut skipped = Vec::new();
        for file in self.tabs.iter_mut().filter(|file| file.is_dirty()) {
            match file.save() {
                Ok(()) => saved += 1,
                Err(error) => skipped.push(format!("{} · {error}", file_label(&file.path))),
            }
        }
        let has_error = !skipped.is_empty();
        let message = if has_error {
            format!(
                "Saved {saved} · {} files still need attention · {}",
                skipped.len(),
                skipped.join(" | ")
            )
        } else if saved == 0 {
            "All files are up to date.".into()
        } else {
            format!("Saved {saved} files")
        };
        self.status = Some((message, has_error));
    }

    fn close_tab(&mut self, index: usize) {
        if let Err(error) = self.tabs[index].clear_checkpoint() {
            self.status = Some((format!("Could not remove recovery file: {error}"), true));
        }
        let previous = self.active;
        let companion = self.split.map(|split| split.companion);
        self.tabs.remove(index);
        let reindex = |remaining: usize| remaining - usize::from(remaining > index);
        if previous == Some(index) {
            self.active = companion
                .map(reindex)
                .or_else(|| self.tabs.len().checked_sub(1).map(|last| last.min(index)));
            self.split = None;
        } else {
            self.active = previous.map(reindex);
            if companion == Some(index) {
                self.split = None;
            } else if let Some(split) = &mut self.split {
                split.companion = reindex(split.companion);
            }
        }
    }

    fn close(&mut self, target: CloseTarget, context: &egui::Context) {
        match target {
            CloseTarget::Tab(index) => self.close_tab(index),
            CloseTarget::Window => {
                self.save_workspace();
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        self.pending_close = None;
    }

    fn request_tab_close(&mut self, context: &egui::Context) {
        if let Some(index) = self.active {
            self.request_close_index(index, context);
        }
    }

    fn request_close_index(&mut self, index: usize, context: &egui::Context) {
        if self.tabs[index].is_dirty() {
            self.pending_close = Some(CloseTarget::Tab(index));
        } else {
            self.close(CloseTarget::Tab(index), context);
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.open_file(&path);
        }
    }

    fn folder_dialog(&mut self, context: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.open_folder(path, context);
        }
    }

    fn create_dialog(&mut self, copy: bool, context: &egui::Context) {
        let mut dialog = rfd::FileDialog::new();
        if let Some(root) = &self.root {
            dialog = dialog.set_directory(root);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        let result = if copy {
            self.tabs[self.active.expect("copy requires an active file")].save_copy(&path)
        } else {
            OpenFile::create_at(&path, "", false)
        };
        match result {
            Ok(file) => {
                self.tabs.push(file);
                self.set_active(self.tabs.len() - 1);
                self.status = Some((format!("Created {}", file_label(&path)), false));
                if let Some(root) = self.root.clone() {
                    self.open_folder(root, context);
                }
            }
            Err(error) => self.status = Some((format!("{} — {error}", file_label(&path)), true)),
        }
    }

    fn top_bar(&mut self, context: &egui::Context) {
        egui::TopBottomPanel::top("toolbar")
            .frame(
                chrome::panel(theme_panel(), 12, 8).stroke(egui::Stroke::new(1.0, theme_border())),
            )
            .show(context, |ui| {
                let open_label = if cfg!(target_os = "macos") {
                    "⌘ P"
                } else {
                    "Ctrl P"
                };
                ui.horizontal(|ui| {
                    let (brand, _) =
                        ui.allocate_exact_size(egui::vec2(27.0, 27.0), egui::Sense::hover());
                    ui.painter().rect_filled(brand, 7.0, chrome::theme_active());
                    chrome::symbol(ui, brand.center(), Icon::Forge, theme_accent());
                    ui.label(RichText::new("ForgeIDE").strong().size(15.0));

                    ui.add_space(7.0);
                    ui.menu_button("File", |ui| {
                        if ui.button("New File…").clicked() {
                            ui.close_menu();
                            self.create_dialog(false, context);
                        }
                        if ui.button("Open File…").clicked() {
                            ui.close_menu();
                            self.open_dialog();
                        }
                        if ui.button("Open Folder…").clicked() {
                            ui.close_menu();
                            self.folder_dialog(context);
                        }
                        ui.separator();
                        if let Some(index) = self.active {
                            if ui.button("Save").clicked() {
                                ui.close_menu();
                                self.save_tab(index);
                            }
                            if ui.button("Save All").clicked() {
                                ui.close_menu();
                                self.save_all();
                            }
                            if ui.button("Save Copy…").clicked() {
                                ui.close_menu();
                                self.create_dialog(true, context);
                            }
                            if ui.button("Close Tab").clicked() {
                                ui.close_menu();
                                self.request_tab_close(context);
                            }
                            ui.separator();
                        }
                        if ui.button("Restore Edits…").clicked() {
                            ui.close_menu();
                            self.restore_dialog();
                        }
                    });
                    let gap = ((ui.available_width() - 370.0) * 0.25).max(8.0);
                    ui.add_space(gap);
                    let width = (ui.available_width() - 140.0).clamp(100.0, 345.0);
                    if ui
                        .add_sized(
                            [width, 29.0],
                            egui::Button::new(
                                RichText::new(format!("Go to File…     {open_label}"))
                                    .color(theme_muted()),
                            )
                            .fill(theme_editor()),
                        )
                        .clicked()
                    {
                        self.show_quick_open();
                    }
                    chrome::right(ui, |ui| {
                        ui.add_enabled_ui(self.active.is_some(), |ui| {
                            if chrome::icon_button(
                                ui,
                                Icon::Split,
                                "Toggle Split Editor",
                                self.split.is_some(),
                                29.0,
                            )
                            .clicked()
                            {
                                self.toggle_split(context);
                            }
                        });
                        if ui.button("Settings").clicked() {
                            self.settings_open = !self.settings_open;
                        }
                        if ui
                            .button("Commands")
                            .on_hover_text(if cfg!(target_os = "macos") {
                                "⌘ ⇧ P"
                            } else {
                                "Ctrl Shift P"
                            })
                            .clicked()
                        {
                            self.palette.open(workbench::Mode::Commands);
                        }
                        if chrome::icon_button(
                            ui,
                            Icon::Terminal,
                            "Toggle Terminal",
                            self.terminal_open,
                            29.0,
                        )
                        .clicked()
                        {
                            self.terminal_open = !self.terminal_open;
                        }
                    });
                });
            });
    }

    fn show_quick_open(&mut self) {
        self.picking_split = false;
        self.palette.open(workbench::Mode::Files);
    }

    fn show_palette(&mut self, context: &egui::Context) {
        use workbench::{Choice, Command};
        let actions = workbench::Actions {
            file_open: self.active.is_some(),
            workspace_open: self.root.is_some(),
            can_undo: self.undo_batch.is_some(),
            can_redo: self.redo_batch.is_some(),
        };
        let files = self
            .tabs
            .iter()
            .map(|tab| tab.path.clone())
            .chain(self.entries.iter().cloned());
        let active = self.active.map(|index| {
            let file = &self.tabs[index];
            (file.path.as_path(), file.text.as_str())
        });
        match self.palette.show(
            context,
            self.root.as_deref(),
            files,
            actions,
            active,
            &self.packages.commands(),
        ) {
            Some(Choice::File(path)) => {
                if std::mem::take(&mut self.picking_split) {
                    self.open_in_split(&path);
                } else {
                    self.open_file(&path);
                }
                if let Some(file) = self.active.map(|index| &self.tabs[index]) {
                    context.memory_mut(|memory| {
                        memory.request_focus(editor_id(
                            &file.path,
                            self.split.is_some_and(|split| split.active_right),
                        ))
                    });
                }
            }
            Some(Choice::ExtensionCommand(id)) => self.run_extension_command(&id, context),
            Some(Choice::Line(line)) => {
                self.pending_jump = Some(line);
                context.request_repaint();
            }
            Some(Choice::Command(command)) => match command {
                Command::NewFile => self.create_dialog(false, context),
                Command::OpenFile => self.open_dialog(),
                Command::OpenFolder => self.folder_dialog(context),
                Command::SaveFile => {
                    if let Some(index) = self.active {
                        self.save_tab(index);
                    }
                }
                Command::SaveAll => self.save_all(),
                Command::CloseTab => self.request_tab_close(context),
                Command::SearchWorkspace => self.select_sidebar(Sidebar::Search),
                Command::ShowExplorer => self.select_sidebar(Sidebar::Files),
                Command::GoToSymbol => self.palette.open(workbench::Mode::Symbols),
                Command::ShowGit => self.select_sidebar(Sidebar::Git),
                Command::ShowDebugger => self.select_sidebar(Sidebar::Debug),
                Command::ShowTests => self.select_sidebar(Sidebar::Tests),
                Command::ShowLanguageServer => self.select_sidebar(Sidebar::Language),
                Command::ShowExtensions => self.select_sidebar(Sidebar::Extensions),
                Command::ToggleSidebar => self.toggle_sidebar(),
                Command::ShowAccounts => self.accounts_open = true,
                Command::ShowProblems => self.problems_open = true,
                Command::ToggleTerminal => self.terminal_open = !self.terminal_open,
                Command::ToggleMinimap => self.hide_minimap = !self.hide_minimap,
                Command::ToggleSplit => self.toggle_split(context),
                Command::Find => self.find.open(false),
                Command::Replace => self.find.open(true),
                Command::UndoBatch => self.replay_batch(false),
                Command::RedoBatch => self.replay_batch(true),
                Command::Restore => self.restore_dialog(),
            },
            None => {
                if self.palette.mode.is_none() {
                    self.picking_split = false;
                }
            }
        }
    }

    fn select_sidebar(&mut self, sidebar: Sidebar) {
        self.sidebar = sidebar;
        let was_hidden = !self.preferences.sidebar_visible;
        self.preferences.sidebar_visible = true;
        self.panel_focus = true;
        if was_hidden {
            if let Err(error) = self.preferences.save() {
                self.status = Some((error.to_string(), true));
            }
        }
    }
    fn toggle_sidebar(&mut self) {
        self.preferences.sidebar_visible = !self.preferences.sidebar_visible;
        if let Err(error) = self.preferences.save() {
            self.status = Some((error.to_string(), true));
        }
    }
    fn view_shortcuts(&mut self, context: &egui::Context) {
        if self.pending_close.is_some()
            || self.language_preview.is_some()
            || self.workspace_files.is_open()
            || self.palette.mode.is_some()
        {
            return;
        }
        if context.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::B)) {
            self.toggle_sidebar();
        }
        for (modifiers, key, sidebar) in [
            (
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::E,
                Sidebar::Files,
            ),
            (
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::F,
                Sidebar::Search,
            ),
            (Modifiers::COMMAND | Modifiers::SHIFT, Key::G, Sidebar::Git),
            (
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::D,
                Sidebar::Debug,
            ),
            (
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::X,
                Sidebar::Extensions,
            ),
            (Modifiers::COMMAND | Modifiers::ALT, Key::T, Sidebar::Tests),
            (
                Modifiers::COMMAND | Modifiers::ALT,
                Key::L,
                Sidebar::Language,
            ),
        ] {
            if context.input_mut(|input| input.consume_key(modifiers, key)) {
                self.select_sidebar(sidebar);
            }
        }
        if context.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::Num0)) {
            self.select_sidebar(self.sidebar);
        }
        if context.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::Num1)) {
            if let Some(file) = self.active.map(|index| &self.tabs[index]) {
                let right = self.split.is_some_and(|split| split.active_right);
                context.memory_mut(|memory| memory.request_focus(editor_id(&file.path, right)));
            }
        }
        if context
            .input_mut(|input| input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::M))
        {
            self.problems_open = true;
        }
        if context.memory(|memory| memory.has_focus(terminal::input_id())) {
            return;
        }
        if context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::F9)) {
            if let Some(file) = self.active.map(|index| &self.tabs[index]) {
                let right = self.split.is_some_and(|split| split.active_right);
                let state =
                    TextEdit::load_state(context, editor_id(&file.path, right)).unwrap_or_default();
                let caret = state
                    .cursor
                    .char_range()
                    .map_or(0, |range| range.primary.index);
                let line = file
                    .text
                    .chars()
                    .take(caret)
                    .filter(|character| *character == '\n')
                    .count()
                    + 1;
                self.debugger.toggle_breakpoint(&file.path, line as u32);
            }
        }
        if let Some(root) = self.tool_root().map(Path::to_owned) {
            let source = self
                .active
                .map(|index| self.tabs[index].path.clone())
                .filter(|path| path.starts_with(&root));
            if self.debugger.handle_keys(context, &root, source.as_deref()) {
                self.select_sidebar(Sidebar::Debug);
            }
        }
    }
    fn show_problems(&mut self, context: &egui::Context) {
        if !self.problems_open {
            return;
        }
        let mut open = true;
        let mut destination = None;
        let problems = self.lsp_panel.problems();
        egui::Window::new("Problems").default_size([680.0,340.0]).open(&mut open).show(context,|ui| {
            ui.label(format!("{} diagnostics published for active language documents",problems.len()));
            if problems.is_empty() { ui.label("No diagnostics currently published. Start the language servers for language checks."); }
            ScrollArea::vertical().show(ui,|ui| {
                for (path,line,message) in &problems {
                    if ui.button(format!("{}:{line}  {message}",path.display())).clicked() { destination = Some((path.clone(),*line)); }
                }
            });
        });
        self.problems_open = open;
        if let Some((path, line)) = destination {
            self.open_file(&path);
            self.pending_jump = Some(line);
        }
    }

    fn activity_rail(&mut self, context: &egui::Context) {
        egui::SidePanel::left("activity-rail")
            .exact_width(51.0)
            .resizable(false)
            .frame(chrome::panel(theme_rail(), 6, 10))
            .show_separator_line(true)
            .show(context, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for section in [
                    Sidebar::Files,
                    Sidebar::Search,
                    Sidebar::Git,
                    Sidebar::Language,
                    Sidebar::Debug,
                    Sidebar::Tests,
                    Sidebar::Extensions,
                ] {
                    let response = chrome::icon_button(
                        ui,
                        section.icon(),
                        section.name(),
                        self.sidebar == section && self.preferences.sidebar_visible,
                        38.0,
                    );
                    if section == Sidebar::Git && self.git_panel.changed_count() > 0 {
                        let center = response.rect.right_top() + egui::vec2(-4.0, 7.0);
                        ui.painter().circle_filled(center, 8.0, theme_accent());
                        ui.painter().text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            self.git_panel.changed_count().min(99).to_string(),
                            FontId::proportional(9.0),
                            theme_rail(),
                        );
                    }
                    let (badge, color) = match section {
                        Sidebar::Language => (self.lsp_panel.problem_count(), theme_red()),
                        Sidebar::Tests => (self.tests.summary().1, theme_red()),
                        Sidebar::Extensions => (self.packages.update_count(), theme_accent()),
                        _ => (0, theme_accent()),
                    };
                    if badge > 0 {
                        let center = response.rect.right_top() + egui::vec2(-4.0, 7.0);
                        ui.painter().circle_filled(center, 8.0, color);
                        ui.painter().text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            badge.min(99).to_string(),
                            FontId::proportional(9.0),
                            theme_rail(),
                        );
                    }
                    if response.clicked() {
                        if self.sidebar == section && self.preferences.sidebar_visible {
                            self.toggle_sidebar();
                        } else {
                            self.select_sidebar(section);
                        }
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if chrome::icon_button(
                        ui,
                        Icon::Account,
                        "GitHub accounts",
                        self.accounts_open,
                        38.0,
                    )
                    .clicked()
                    {
                        self.accounts_open = !self.accounts_open;
                    }
                });
            });
    }

    fn restore_dialog(&mut self) {
        let result = (|| -> std::io::Result<()> {
            let directory = recovery::directory()?;
            let Some(path) = rfd::FileDialog::new()
                .set_directory(directory)
                .add_filter("ForgeIDE recovery", &["journal"])
                .pick_file()
            else {
                return Ok(());
            };
            let record = recovery::read(&path)?;
            let file = OpenFile::recover(record)?;
            if self.tabs.iter().any(|tab| tab.path == file.path) {
                return Err(std::io::Error::other(
                    "Close the existing tab before recovering this file.",
                ));
            }
            self.tabs.push(file);
            self.set_active(self.tabs.len() - 1);
            self.status = Some((
                "Recovery opened. Save or use Save Copy to keep the recovered edits.".into(),
                false,
            ));
            Ok(())
        })();
        if let Err(error) = result {
            self.status = Some((error.to_string(), true));
        }
    }

    fn flush_recovery(&mut self, context: &egui::Context) {
        let Some(due) = self.recovery_due else { return };
        let remaining = due.saturating_duration_since(std::time::Instant::now());
        if !remaining.is_zero() {
            context.request_repaint_after(remaining);
            return;
        }
        self.recovery_due = None;
        let result = recovery::directory().and_then(|directory| {
            self.tabs
                .iter_mut()
                .try_for_each(|file| file.checkpoint(&directory))
        });
        if let Err(error) = result {
            self.status = Some((format!("Recovery checkpoint failed: {error}"), true));
        }
    }

    fn workspace_operations(&mut self, context: &egui::Context) {
        if let Some(outcome) = self.workspace_files.poll() {
            match outcome {
                workspace_files::Outcome::Created(path, folder) => {
                    if folder {
                        self.folder_state.insert(path, true);
                    } else {
                        self.open_file(&path);
                    }
                }
                workspace_files::Outcome::Renamed(before, after) => {
                    for file in &mut self.tabs {
                        if let Ok(relative) = file.path.strip_prefix(&before) {
                            let target = after.join(relative);
                            if let Err(error) = file.relocate(&target) {
                                self.status = Some((error.to_string(), true));
                            }
                        }
                    }
                    let previous = std::mem::take(&mut self.folder_state);
                    self.folder_state = previous
                        .into_iter()
                        .map(|(path, expanded)| {
                            let next = path
                                .strip_prefix(&before)
                                .map_or_else(|_| path.clone(), |relative| after.join(relative));
                            (next, expanded)
                        })
                        .collect();
                }
                workspace_files::Outcome::Removed(path) => {
                    for index in (0..self.tabs.len()).rev() {
                        if self.tabs[index].path.starts_with(&path) {
                            if self.tabs[index].is_dirty() {
                                self.tabs[index].external_change = true;
                            } else {
                                self.close_tab(index);
                            }
                        }
                    }
                }
                workspace_files::Outcome::Restored(path) => {
                    if path.is_file() {
                        self.open_file(&path);
                    }
                }
            }
            self.palette.refresh();
            self.scan_workspace(context);
            self.save_workspace();
            context.request_repaint();
        }
        if self.pending_close.is_none() && self.language_preview.is_none() {
            self.workspace_files.show(context, &self.tabs);
        }
    }

    fn run_extension_command(&mut self, id: &str, context: &egui::Context) {
        let Some((extension, module)) = self.packages.command(id) else {
            self.status = Some((
                "This extension command is disabled or unavailable.".into(),
                true,
            ));
            return;
        };
        let Some(index) = self.active else {
            self.status = Some((
                "Open a file before running an extension command.".into(),
                true,
            ));
            return;
        };
        let file = &self.tabs[index];
        let root = self
            .tool_root()
            .or_else(|| file.path.parent())
            .unwrap_or(Path::new("."))
            .to_owned();
        self.extension_panel
            .run_command(extension, module, &root, file, context);
    }

    fn show_extensions(&mut self, ui: &mut egui::Ui, context: &egui::Context, root: Option<&Path>) {
        ScrollArea::vertical()
            .id_salt("extension-manager")
            .show(ui, |ui| {
                if let Some(action) = self.packages.show(ui) {
                    self.palette.refresh();
                    match action {
                        packages::Action::Theme(theme) => {
                            self.preferences.light = !theme.dark;
                            self.preferences.custom_theme = Some(theme);
                            self.preferences.apply(context);
                            self.layout_cache = [None, None];
                            if let Err(error) = self.preferences.save() {
                                self.status = Some((error.to_string(), true));
                            }
                        }
                        packages::Action::Transform(name, module) => {
                            self.extension_panel.use_package(name, module)
                        }
                        packages::Action::RunCommand(id) => {
                            self.run_extension_command(&id, context)
                        }
                    }
                }
                ui.separator();
                self.extension_panel.sync_packages(&self.packages, root);
                let file = self.active.map(|index| &self.tabs[index]);
                let cwd = root.or_else(|| file.and_then(|file| file.path.parent()));
                if let Some(cwd) = cwd {
                    ui.collapsing("Local WebAssembly", |ui| {
                        self.extension_panel.show(ui, cwd, file)
                    });
                } else {
                    ui.label(chrome::small("Open a document to run extension commands."));
                }
            });
    }

    fn explorer(&mut self, context: &egui::Context) {
        if !self.preferences.sidebar_visible {
            return;
        }

        let mut picked = None;
        let tool_root = self.tool_root().map(Path::to_owned);
        let roots = self.roots();
        egui::SidePanel::left("explorer")
            .default_width(233.0)
            .width_range(170.0..=390.0)
            .frame(chrome::panel(theme_panel(), 11, 12))
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    let heading = ui.add(
                        egui::Label::new(chrome::label(self.sidebar.name().to_uppercase()))
                            .sense(egui::Sense::focusable_noninteractive()),
                    );
                    if self.panel_focus && !matches!(self.sidebar, Sidebar::Files | Sidebar::Search)
                    {
                        heading.request_focus();
                        self.panel_focus = false;
                    }
                    if self.root.is_some() && self.sidebar == Sidebar::Files {
                        chrome::right(ui, |ui| {
                            ui.menu_button("…", |ui| {
                                if let Some(root) = tool_root.as_deref() {
                                    if ui.button("New file in workspace…").clicked() {
                                        self.workspace_files.new_item(root, root, false);
                                        ui.close_menu();
                                    }
                                    if ui.button("New folder in workspace…").clicked() {
                                        self.workspace_files.new_item(root, root, true);
                                        ui.close_menu();
                                    }
                                }
                                ui.menu_button("Removed items", |ui| {
                                    self.workspace_files.recovery_menu(ui)
                                });
                                if ui.button("Refresh Files").clicked() {
                                    ui.close_menu();
                                    picked = Some(ExplorerAction::Refresh);
                                }
                                if ui.button("Add Folder to Workspace…").clicked() {
                                    ui.close_menu();
                                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                        picked = Some(ExplorerAction::Add(path));
                                    }
                                }
                                if ui.button("Open Folder…").clicked() {
                                    ui.close_menu();
                                    self.folder_dialog(context);
                                }
                            });
                        });
                    }
                });
                ui.add_space(3.0);
                if self.sidebar == Sidebar::Extensions {
                    self.show_extensions(ui, context, tool_root.as_deref());
                    return;
                }
                if self.sidebar == Sidebar::Git {
                    egui::CollapsingHeader::new("Accounts")
                        .id_salt("github-account")
                        .show(ui, |ui| self.accounts.show(ui));
                    ui.separator();
                }
                if self.sidebar == Sidebar::Git && tool_root.is_none() {
                    self.git_panel.show_without_folder(ui);
                    if let Some(path) = self.git_panel.take_cloned() {
                        picked = Some(ExplorerAction::Add(path));
                    }
                    return;
                }
                let Some(root) = tool_root.as_ref() else {
                    ui.label(chrome::small("Open a folder to use workspace tools."));
                    if ui.button("Open Folder…").clicked() {
                        self.folder_dialog(context);
                    }
                    return;
                };
                match self.sidebar {
                    Sidebar::Extensions => return,
                    Sidebar::Debug => {
                        let unsaved = self.tabs.iter().filter(|file| file.path.starts_with(root) && file.is_dirty()).count();
                        if unsaved > 0 { ui.colored_label(theme_red(),format!("{unsaved} unsaved documents. Debugging uses saved files and the selected build.")); }
                        let source = self
                            .active
                            .map(|index| &self.tabs[index].path)
                            .filter(|source| source.starts_with(root))
                            .map(PathBuf::as_path);
                        self.debugger.show(ui, root, source);
                        return;
                    }
                    Sidebar::Tests => {
                        let revision = testing::revision(&self.tabs, root);
                        let unsaved = self
                            .tabs
                            .iter()
                            .filter(|file| file.path.starts_with(root) && file.is_dirty())
                            .count();
                        self.tests.show(ui, root, revision, unsaved);
                        return;
                    }
                    Sidebar::Language => {
                        self.lsp_panel.show(ui, root);
                        return;
                    }
                    Sidebar::Git => {
                        self.git_panel.show(ui, root);
                        if let Some(path) = self.git_panel.take_cloned() {
                            picked = Some(ExplorerAction::Add(path));
                        }
                        if let Some(path) = self.git_panel.take_open() {
                            picked = Some(ExplorerAction::Open(path));
                        }
                        return;
                    }
                    Sidebar::Files => {
                        let filter_response = ui.add(
                            TextEdit::singleline(&mut self.filter)
                                .hint_text("Filter files")
                                .desired_width(f32::INFINITY),
                        );
                        if std::mem::take(&mut self.panel_focus) {
                            filter_response.request_focus();
                        }
                        if self.scan.is_some() {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(chrome::small("Indexing folders…"));
                            });
                        }
                        ui.add_space(4.0);
                        let active_path = self.active.map(|index| self.tabs[index].path.as_path());
                        let mut visible = Vec::new();
                        for root in &roots {
                            visible.push(ExplorerRow {
                                path: root,
                                name: file_label(root),
                                depth: 0,
                                folder: true,
                            });
                            if folder_open(root, &self.folder_state, active_path)
                                || !self.filter.trim().is_empty()
                            {
                                let mut rows = explorer_rows(
                                    root,
                                    &self.entries,
                                    &self.directories,
                                    &self.filter,
                                    &self.folder_state,
                                    active_path,
                                );
                                // Nested roots appear in their own section.
                                rows.retain(|row| self.root_for(row.path) == Some(root.as_path()));
                                for row in &mut rows {
                                    row.depth += 1;
                                }
                                visible.extend(rows);
                            }
                        }
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ScrollArea::vertical().id_salt("workspace-files").show_rows(
                            ui,
                            chrome::EXPLORER_ROW_HEIGHT,
                            visible.len(),
                            |ui, range| {
                                for row in &visible[range] {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 0.0;
                                        ui.add_space((row.depth.min(8) * 13) as f32);
                                        if row.folder {
                                            if row.depth == 0 {
                                                ui.menu_button("…", |ui| {
                                                    if ui.button("Remove from workspace").clicked()
                                                    {
                                                        picked = Some(ExplorerAction::Remove(
                                                            row.path.to_owned(),
                                                        ));
                                                        ui.close_menu();
                                                    }
                                                });
                                            }
                                            let expanded = folder_open(
                                                row.path,
                                                &self.folder_state,
                                                active_path,
                                            );
                                            let response =
                                                chrome::folder_row(ui, &row.name, expanded);
                                            if response.clicked() {
                                                self.folder_state
                                                    .insert(row.path.to_owned(), !expanded);
                                            }
                                            if let Some(root) = roots
                                                .iter()
                                                .filter(|root| row.path.starts_with(root))
                                                .max_by_key(|root| root.components().count())
                                            {
                                                response.context_menu(|ui| {
                                                    self.workspace_files.menu(
                                                        ui,
                                                        root,
                                                        row.path,
                                                        true,
                                                        row.depth == 0,
                                                    )
                                                });
                                            }
                                        } else {
                                            let row_action = chrome::file_row(
                                                ui,
                                                row.path,
                                                &row.name,
                                                active_path == Some(row.path),
                                            );
                                            if row_action.clicked() {
                                                picked =
                                                    Some(ExplorerAction::Open(row.path.to_owned()));
                                            }
                                            row_action.context_menu(|ui| {
                                                if let Some(root) = roots
                                                    .iter()
                                                    .filter(|root| row.path.starts_with(root))
                                                    .max_by_key(|root| root.components().count())
                                                {
                                                    self.workspace_files
                                                        .menu(ui, root, row.path, false, false);
                                                    ui.separator();
                                                }
                                                if ui
                                                    .add_enabled(
                                                        active_path
                                                            .is_some_and(|path| path != row.path),
                                                        egui::Button::new("Open to Side"),
                                                    )
                                                    .clicked()
                                                {
                                                    picked = Some(ExplorerAction::OpenSide(
                                                        row.path.to_owned(),
                                                    ));
                                                    ui.close_menu();
                                                }
                                            });
                                        }
                                    });
                                }
                            },
                        );
                        if visible.is_empty() {
                            ui.label(chrome::small("No matching files"));
                        }
                        return;
                    }
                    Sidebar::Search => {}
                }
                let query = ui.add(
                    TextEdit::singleline(&mut self.workspace_query)
                        .hint_text("Search file contents")
                        .desired_width(f32::INFINITY),
                );
                if std::mem::take(&mut self.panel_focus) {
                    query.request_focus();
                }
                ui.horizontal_wrapped(|ui| {
                    ui.toggle_value(&mut self.workspace_regex, "Regex");
                    ui.toggle_value(&mut self.workspace_ignore_case, "Ignore case");
                    ui.toggle_value(&mut self.workspace_replace, "Replace");
                });
                ui.add(
                    TextEdit::singleline(&mut self.workspace_path_filter)
                        .hint_text("Filter paths")
                        .desired_width(f32::INFINITY),
                );
                if self.workspace_replace {
                    ui.add(
                        TextEdit::singleline(&mut self.workspace_replacement)
                            .hint_text("Replace with")
                            .desired_width(f32::INFINITY),
                    )
                    .on_hover_text("Regex replacements support $1 and named captures.");
                }
                let ready = !self.workspace_query.is_empty()
                    && self.workspace_search.is_none()
                    && (!self.workspace_replace || self.scan.is_none());
                let name = if self.workspace_replace {
                    "Review Replacements"
                } else {
                    "Search Workspace"
                };
                let submit = ui
                    .add_enabled(ready, egui::Button::new(name).fill(chrome::theme_active()))
                    .clicked();
                if ready
                    && (submit
                        || query.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)))
                {
                    self.start_workspace_search(context);
                }
                if self.workspace_search.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        if ui.button("Cancel").clicked() {
                            self.workspace_search = None;
                        }
                    });
                }
                if let Some(results) = &self.search_results {
                    let summary = format!(
                        "{} matches · {} files skipped{}",
                        results.matches.len(),
                        results.skipped_files,
                        if results.capped || self.listing_incomplete {
                            " · Partial results"
                        } else {
                            ""
                        }
                    );
                    ui.label(chrome::small(summary));
                    ScrollArea::vertical().id_salt("search-results").show_rows(
                        ui,
                        43.0,
                        results.matches.len(),
                        |ui, rows| {
                            for hit in &results.matches[rows] {
                                let relative = self
                                    .root
                                    .as_deref()
                                    .and_then(|root| hit.path.strip_prefix(root).ok())
                                    .unwrap_or(&hit.path);
                                let label = format!("{}:{}", relative.display(), hit.line);
                                if chrome::palette_row(ui, &label, &hit.excerpt, false).clicked() {
                                    picked = Some(ExplorerAction::Jump(hit.path.clone(), hit.line));
                                }
                            }
                        },
                    );
                }
            });
        match picked {
            Some(ExplorerAction::Open(path)) => self.open_file(&path),
            Some(ExplorerAction::OpenSide(path)) => self.open_in_split(&path),
            Some(ExplorerAction::Jump(path, line)) => {
                self.open_file(&path);
                if self
                    .active
                    .is_some_and(|index| path == self.tabs[index].path)
                {
                    self.pending_jump = Some(line);
                }
            }
            Some(ExplorerAction::Refresh) => self.scan_workspace(context),
            Some(ExplorerAction::Add(path)) => self.add_folder(path, context),
            Some(ExplorerAction::Remove(path)) => self.remove_folder(&path, context),
            None => {}
        }
    }

    fn start_workspace_search(&mut self, context: &egui::Context) {
        let roots = self.roots();
        if self.workspace_replace && self.listing_incomplete {
            self.status = Some((
                "The folder listing is incomplete. Open a narrower folder to review replacements."
                    .into(),
                true,
            ));
            return;
        }
        let mut buffers = std::collections::HashMap::new();
        let mut snapshot_bytes = 0;
        for tab in self
            .tabs
            .iter()
            .filter(|tab| roots.iter().any(|root| tab.path.starts_with(root)))
        {
            let snapshot = if tab.text.len() as u64 <= search::FILE_LIMIT {
                snapshot_bytes += tab.text.len();
                if snapshot_bytes > 64 * 1024 * 1024 {
                    self.status = Some((
                        "Open documents exceed the search snapshot budget.".into(),
                        true,
                    ));
                    return;
                }
                Some(tab.text.clone())
            } else {
                None
            };
            buffers.insert(tab.path.clone(), snapshot);
        }
        let request = search::Request {
            query: self.workspace_query.clone(),
            regex: self.workspace_regex,
            ignore_case: self.workspace_ignore_case,
            path_filter: self.workspace_path_filter.clone(),
            replacement: self
                .workspace_replace
                .then(|| self.workspace_replacement.clone()),
        };
        let (sender, outcome) = mpsc::sync_channel(1);
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop = cancelled.clone();
        self.workspace_search = Some(WorkspaceSearch {
            partial: self.listing_incomplete || self.scan.is_some(),
            outcome,
            cancelled,
        });
        self.workspace_review = None;
        self.search_results = None;
        self.status = None;
        let entries = self.entries.clone();
        let context = context.clone();
        std::thread::spawn(move || {
            let result = search::run(&roots, &entries, &buffers, request, &stop);
            let _ = sender.send(result);
            context.request_repaint();
        });
    }

    fn poll_workspace_search(&mut self) {
        let Some(job) = &self.workspace_search else {
            return;
        };
        let result = match job.outcome.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(std::io::Error::other("The search stopped unexpectedly."))
            }
        };
        let partial = job.partial;
        self.workspace_search = None;
        match result {
            Ok(search::Outcome::Search(mut results)) => {
                results.capped |= partial;
                self.search_results = Some(results);
            }
            Ok(search::Outcome::Review {
                preview,
                skipped_files,
            }) => {
                if skipped_files > 0 {
                    self.status = Some((
                        format!("{skipped_files} files skipped (size, encoding, or access)."),
                        false,
                    ));
                }
                self.workspace_review = Some(preview);
            }
            Err(error) => self.status = Some((error.to_string(), true)),
        }
    }

    fn terminal_panel(&mut self, context: &egui::Context) {
        let root = self.tool_root().map(Path::to_owned);
        egui::TopBottomPanel::bottom("terminal")
            .resizable(true)
            .default_height(240.0)
            .min_height(140.0)
            .frame(chrome::panel(theme_panel(), 12, 8))
            .show(context, |ui| {
                ui.set_min_height(ui.available_height());
                ui.horizontal_wrapped(|ui| {
                    ui.label(chrome::label("TERMINAL"));
                    for index in 0..self.terminal.len() {
                        if ui
                            .selectable_label(
                                self.active_terminal == index,
                                format!("Shell {}", index + 1),
                            )
                            .clicked()
                        {
                            self.active_terminal = index;
                        }
                    }
                    if let Some(root) = &root {
                        if ui.button("+").on_hover_text("New terminal").clicked() {
                            match terminal::Terminal::start(root, context) {
                                Ok(terminal) => {
                                    self.terminal.push(terminal);
                                    self.active_terminal = self.terminal.len() - 1;
                                }
                                Err(error) => self.status = Some((error.to_string(), true)),
                            }
                        }
                    }
                    if ui
                        .add_enabled(!self.terminal.is_empty(), egui::Button::new("Close shell"))
                        .clicked()
                    {
                        self.terminal.remove(self.active_terminal);
                        self.active_terminal = self
                            .active_terminal
                            .min(self.terminal.len().saturating_sub(1));
                    }
                    if ui.button("Hide").clicked() {
                        self.terminal_open = false;
                    }
                });
                ui.collapsing("Project tasks", |ui| {
                    if let Some(root) = &root {
                        if let Some(index) = self.tasks.show(ui, root) {
                            match self.tasks.launch(index, root, context) {
                                Ok(terminal) => {
                                    self.terminal.push(terminal);
                                    self.active_terminal = self.terminal.len() - 1;
                                }
                                Err(error) => self.status = Some((error.to_string(), true)),
                            }
                        }
                    }
                });
                ui.separator();
                if let Some(terminal) = self.terminal.get_mut(self.active_terminal) {
                    ui.label(chrome::small(terminal.root().to_string_lossy()));
                    terminal.show(ui);
                } else {
                    ui.label("Use + to start a terminal in the project folder.");
                }
            });
    }

    fn empty_editor(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        let offset = (ui.available_height() * 0.23).clamp(30.0, 160.0);
        ui.vertical_centered(|ui| {
            ui.add_space(offset);
            let (badge, _) = ui.allocate_exact_size(egui::vec2(52.0, 52.0), egui::Sense::hover());
            ui.painter().rect_filled(badge, 14, chrome::theme_active());
            chrome::symbol(ui, badge.center(), Icon::Forge, theme_accent());
            ui.add_space(9.0);
            ui.heading(
                RichText::new(if self.root.is_some() {
                    "Open a file"
                } else {
                    "Open a project"
                })
                .size(28.0),
            );
            ui.label(
                RichText::new(if self.root.is_some() {
                    "Choose a file in Explorer or search by name."
                } else {
                    "Choose a folder to start a workspace."
                })
                .color(theme_muted()),
            );
            ui.add_space(12.0);
            let has_project = self.root.is_some();
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(if has_project {
                            "Go to File…"
                        } else {
                            "Open Folder…"
                        })
                        .strong()
                        .color(theme_editor()),
                    )
                    .fill(theme_accent()),
                )
                .clicked()
            {
                if has_project {
                    self.show_quick_open();
                } else {
                    self.folder_dialog(context);
                }
            }
            if ui.button("Open File…").clicked() {
                self.open_dialog();
            }
        });
    }

    fn editor(&mut self, context: &egui::Context) {
        egui::CentralPanel::default()
            .frame(Frame::new().fill(theme_editor()))
            .show(context, |ui| {
                if self.tabs.is_empty() {
                    self.cursor = None;
                    self.empty_editor(ui, context);
                    return;
                }
                let mut close = None;
                let mut activate = None;
                chrome::panel(theme_rail(), 8, 5).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ScrollArea::horizontal()
                        .id_salt("editor-tabs")
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                for (index, buffer) in self.tabs.iter().enumerate() {
                                    let active = self.active == Some(index);
                                    let background = if active {
                                        theme_elevated()
                                    } else {
                                        theme_rail()
                                    };
                                    let frame = Frame::new()
                                        .fill(background)
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::symmetric(6, 2));
                                    let tab = frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 4.0;
                                            chrome::file_mark(ui, &buffer.path);
                                            let name = file_label(&buffer.path);
                                            if ui
                                                .add(
                                                    egui::Button::new(RichText::new(name).color(
                                                        if active {
                                                            theme_text()
                                                        } else {
                                                            theme_muted()
                                                        },
                                                    ))
                                                    .frame(false),
                                                )
                                                .on_hover_text(buffer.path.display().to_string())
                                                .clicked()
                                            {
                                                activate = Some(index);
                                            }
                                            if buffer.is_dirty() {
                                                let (dot, _) = ui.allocate_exact_size(
                                                    egui::vec2(11.0, 17.0),
                                                    egui::Sense::hover(),
                                                );
                                                ui.painter().circle_filled(
                                                    dot.center(),
                                                    3.0,
                                                    theme_accent(),
                                                );
                                            }
                                            if chrome::icon_button(
                                                ui,
                                                Icon::Close,
                                                "Close Tab",
                                                false,
                                                20.0,
                                            )
                                            .clicked()
                                            {
                                                close = Some(index);
                                            }
                                        });
                                    });
                                    if active {
                                        let rect = tab.response.rect;
                                        ui.painter().line_segment(
                                            [
                                                rect.left_bottom() + egui::vec2(7.0, -1.0),
                                                rect.right_bottom() + egui::vec2(-7.0, -1.0),
                                            ],
                                            egui::Stroke::new(1.4, theme_accent()),
                                        );
                                    }
                                }
                            });
                        });
                });
                if let Some(index) = activate {
                    self.set_active(index);
                }
                if let Some(index) = close {
                    self.request_close_index(index, context);
                }
                if let Some(file) = self.active.map(|index| &self.tabs[index]) {
                    chrome::panel(theme_editor(), 16, 5).show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let relative = self
                                .root
                                .as_deref()
                                .and_then(|root| file.path.strip_prefix(root).ok());
                            let components = relative
                                .map(|path| path.components().collect::<Vec<_>>())
                                .unwrap_or_default();
                            if let Some(root) = &self.root {
                                ui.label(chrome::small(file_label(root)));
                                ui.label(RichText::new("›").color(chrome::theme_dim()));
                            }
                            for component in components
                                .iter()
                                .take(components.len().saturating_sub(1))
                                .take(5)
                            {
                                ui.add(
                                    egui::Label::new(chrome::small(
                                        component.as_os_str().to_string_lossy(),
                                    ))
                                    .truncate(),
                                );
                                ui.label(RichText::new("›").color(chrome::theme_dim()));
                            }
                            chrome::file_mark(ui, &file.path);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(file_label(&file.path))
                                        .size(12.0)
                                        .color(theme_text()),
                                )
                                .truncate(),
                            )
                            .on_hover_text(file.path.display().to_string());
                        });
                    });
                }
                let (divider, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 1.0),
                    egui::Sense::hover(),
                );
                ui.painter().hline(
                    divider.x_range(),
                    divider.center().y,
                    egui::Stroke::new(1.0, theme_border()),
                );
                if let Some(active) = self.active {
                    if let Some(split) = self.split {
                        self.split_panes(ui, context, active, split);
                    } else {
                        let (_, jump) = self.draw_document(ui, context, active, false, true);
                        if jump.is_some() {
                            self.pending_jump = jump;
                        }
                    }
                }
            });
    }

    fn split_panes(
        &mut self,
        ui: &mut egui::Ui,
        context: &egui::Context,
        active: usize,
        split: SplitEditor,
    ) {
        let bounds = ui.available_rect_before_wrap();
        let (_, _) = ui.allocate_exact_size(bounds.size(), egui::Sense::hover());
        if bounds.width() <= 12.0 || bounds.height() <= 30.0 {
            return;
        }
        let stacked = bounds.width() < 490.0;
        let divider_size = 7.0;
        let mut left = bounds;
        let mut right_rect = bounds;
        let divisor = if stacked {
            bounds.height()
        } else {
            bounds.width()
        };
        let edge = if stacked { bounds.top() } else { bounds.left() }
            + (divisor - divider_size) * split.ratio;
        let divider = if stacked {
            left.max.y = edge;
            right_rect.min.y = edge + divider_size;
            egui::Rect::from_min_max(
                egui::pos2(bounds.left(), edge),
                egui::pos2(bounds.right(), edge + divider_size),
            )
        } else {
            left.max.x = edge;
            right_rect.min.x = edge + divider_size;
            egui::Rect::from_min_max(
                egui::pos2(edge, bounds.top()),
                egui::pos2(edge + divider_size, bounds.bottom()),
            )
        };
        let response = ui.interact(divider, egui::Id::new("split-divider"), egui::Sense::drag());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Resize editor panels")
        });
        ui.painter().rect_filled(
            divider,
            0.0,
            if response.hovered() || response.dragged() {
                theme_accent()
            } else {
                theme_border()
            },
        );
        if response.hovered() || response.dragged() {
            context.set_cursor_icon(if stacked {
                egui::CursorIcon::ResizeVertical
            } else {
                egui::CursorIcon::ResizeHorizontal
            });
        }
        if let Some(pointer) = response
            .interact_pointer_pos()
            .filter(|_| response.dragged())
        {
            if let Some(state) = &mut self.split {
                let offset = if stacked {
                    pointer.y - bounds.top()
                } else {
                    pointer.x - bounds.left()
                };
                state.ratio = (offset / (divisor - divider_size)).clamp(0.25, 0.75);
                context.request_repaint();
            }
        }
        let mut focused_elsewhere = None;
        let mut requested_line = None;
        for (position, rect) in [(false, left), (true, right_rect)] {
            let focused = position == split.active_right;
            let index = if focused { active } else { split.companion };
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt(("split-pane", position))
                    .max_rect(rect),
                |ui| {
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    ui.set_max_size(rect.size());
                    chrome::panel(
                        if focused {
                            theme_elevated()
                        } else {
                            theme_rail()
                        },
                        10,
                        4,
                    )
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let file = &self.tabs[index];
                            chrome::file_mark(ui, &file.path);
                            let label = file_label(&file.path);
                            if ui
                                .add(
                                    egui::Button::new(RichText::new(label).color(if focused {
                                        theme_text()
                                    } else {
                                        theme_muted()
                                    }))
                                    .frame(false)
                                    .truncate(),
                                )
                                .on_hover_text(file.path.display().to_string())
                                .clicked()
                            {
                                context.memory_mut(|memory| {
                                    memory.request_focus(editor_id(&file.path, position))
                                });
                                if !focused {
                                    focused_elsewhere = Some(index);
                                }
                            }
                            if file.is_dirty() {
                                ui.label(RichText::new("•").color(theme_accent()));
                            }
                        });
                    });
                    let (focus, jump) = self.draw_document(ui, context, index, position, focused);
                    if focus && !focused {
                        focused_elsewhere = Some(index);
                    }
                    if jump.is_some() {
                        requested_line = jump;
                    }
                },
            );
        }
        if let Some(index) = focused_elsewhere {
            self.set_active(index);
        }
        if requested_line.is_some() {
            self.pending_jump = requested_line;
        }
    }

    fn draw_document(
        &mut self,
        ui: &mut egui::Ui,
        context: &egui::Context,
        index: usize,
        right: bool,
        active: bool,
    ) -> (bool, Option<usize>) {
        let accepts_input = self.palette.mode.is_none()
            && self.language_preview.is_none()
            && self.pending_close.is_none();
        let file = &mut self.tabs[index];
        if let Some(preview) = conflicts::show(ui, &file.path, &file.text) {
            match preview {
                Ok(preview) => self.language_preview = Some(preview),
                Err(error) => self.status = Some((error.to_string(), true)),
            }
        }
        let outcome = if active {
            self.find.show(ui, file)
        } else {
            find::Outcome {
                selection: None,
                preview: None,
            }
        };
        let mut found = outcome.selection;
        if outcome.preview.is_some() && self.language_preview.is_none() {
            self.language_preview = outcome.preview;
        }
        let jump = if active {
            self.pending_jump.take()
        } else {
            None
        };
        if let Some(line) = jump {
            use forge_text::position::{Encoding, LineIndex, Position};
            let position = Position {
                line: line - 1,
                character: 0,
            };
            if let Ok(byte) = LineIndex::new(&file.text).to_byte(position, Encoding::Utf8) {
                found = Some(byte..byte);
            }
        }
        if file.external_change {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(Color32::YELLOW, "The file changed on disk.");
                if file.is_dirty() {
                    ui.label("Use Save Copy to keep your edits.");
                } else if ui.button("Reload from disk").clicked() {
                    match OpenFile::open(&file.path) {
                        Ok(reloaded) => {
                            if let Err(error) = file.clear_checkpoint() {
                                self.status = Some((error.to_string(), true));
                            }
                            *file = reloaded;
                            self.snippet = None;
                            self.multiple = None;
                            TextEdit::store_state(
                                context,
                                editor_id(&file.path, right),
                                Default::default(),
                            );
                        }
                        Err(error) => self.status = Some((error.to_string(), true)),
                    }
                }
            });
        }
        let path = file.path.clone();
        let original_text = context
            .input(|input| !input.events.is_empty())
            .then(|| file.text.clone());
        let folds = self.folds.entry(path.clone()).or_default();
        folds.sync(&path, &file.text);
        if let Some(range) = TextEdit::load_state(context, editor_id(&path, right))
            .and_then(|state| state.cursor.char_range())
        {
            let byte =
                multicursor::bytes(&file.text, range.primary.index..range.primary.index).start;
            let row = file.text[..byte]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count();
            folds.reveal(row);
        }
        let hidden = folds.hidden();
        let cache_slot = &mut self.layout_cache[usize::from(right)];
        let mut layout = |ui: &egui::Ui, text: &str, _: f32| {
            let cache = cache_slot.get_or_insert_with(|| (path.clone(), LayoutJob::default()));
            if cache.0 != path || cache.1.text != text {
                *cache = (
                    path.clone(),
                    syntax::layout(&path, text, ui.visuals().text_color()),
                );
            }
            let mut job = cache.1.clone();
            folding::apply(&mut job, &hidden);
            ui.fonts(|fonts| fonts.layout_job(job))
        };
        let minimap = !self.hide_minimap && ui.available_width() >= 530.0;
        let mut focused = false;
        let mut requested_line = None;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let width = ui.available_width() - if minimap { 89.0 } else { 0.0 };
            let height = ui.available_height();
            let state = ui
                .allocate_ui_with_layout(
                    egui::vec2(width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_max_width(width);
                        ScrollArea::both()
                            .auto_shrink([false, false])
                            .id_salt((&path, right))
                            .show(ui, |ui| {
                                ui.add_space(8.0);
                                ui.horizontal_top(|ui| {
                                    let (gutter, _) = ui.allocate_exact_size(
                                        egui::vec2(62.0, 0.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.add_space(6.0);
                                    let highlight = ui.painter().add(egui::Shape::Noop);
                                    let id = editor_id(&path, right);
                                    let selection = found.map(|range| {
                                        use egui::{text::CCursor, text_selection::CCursorRange};
                                        let start = file.text[..range.start].chars().count();
                                        let end = start + file.text[range].chars().count();
                                        let selection = CCursorRange::two(
                                            CCursor::new(start),
                                            CCursor::new(end),
                                        );
                                        let mut state =
                                            TextEdit::load_state(context, id).unwrap_or_default();
                                        state.cursor.set_char_range(Some(selection));
                                        TextEdit::store_state(context, id, state);
                                        selection
                                    });
                                    let modified = if accepts_input
                                        && context.memory(|memory| memory.has_focus(id))
                                    {
                                        self.lsp_panel.completion_keys(context, &path);
                                        let snippet_changed = if self.multiple.is_none() {
                                            snippets::keyboard(
                                                context,
                                                id,
                                                file,
                                                &mut self.snippet,
                                                &self.packages,
                                            )
                                        } else {
                                            false
                                        };
                                        edit_keyboard(
                                            context,
                                            id,
                                            file,
                                            &mut self.multiple,
                                            &mut self.undo_batch,
                                            &mut self.redo_batch,
                                            &mut self.status,
                                        ) || snippet_changed
                                    } else {
                                        if self
                                            .multiple
                                            .as_ref()
                                            .is_some_and(|entry| entry.editor == id)
                                        {
                                            self.multiple = None;
                                        }
                                        false
                                    };
                                    let editor = TextEdit::multiline(&mut file.text)
                                        .id(id)
                                        .code_editor()
                                        .font(FontId::monospace(chrome::font_size()))
                                        .frame(false)
                                        .desired_width(f32::INFINITY)
                                        .layouter(&mut layout);
                                    let output = editor.show(ui);
                                    if modified && !output.response.has_focus() {
                                        output.response.request_focus();
                                    }
                                    focused =
                                        output.response.has_focus() || output.response.clicked();
                                    let cursor = output.cursor_range.map(|range| {
                                        (
                                            range.primary.rcursor.row + 1,
                                            range.primary.rcursor.column + 1,
                                        )
                                    });
                                    if (active || focused) && self.cursor != cursor {
                                        self.cursor = cursor;
                                        context.request_repaint();
                                    }
                                    if let Some(line) = paint_gutter(
                                        ui,
                                        gutter,
                                        &output,
                                        highlight,
                                        self.debugger.breakpoints(&path),
                                        self.lsp_panel.diagnostics(&path),
                                    ) {
                                        self.debugger.toggle_breakpoint(&path, line);
                                    }
                                    if folding::gutter(ui, gutter, &output, folds) {
                                        if let Some(cursor) = output.cursor_range {
                                            if let Some(region) =
                                                folds.hidden().iter().find(|region| {
                                                    region.contains(&cursor.primary.rcursor.row)
                                                })
                                            {
                                                let byte = file
                                                    .text
                                                    .split_inclusive('\n')
                                                    .take(region.start - 1)
                                                    .map(str::len)
                                                    .sum::<usize>();
                                                set_caret(
                                                    context,
                                                    id,
                                                    file.text[..byte].chars().count(),
                                                );
                                            }
                                        }
                                        context.request_repaint();
                                    }
                                    output.response.context_menu(|ui| {
                                        if ui.button("Local file history…").clicked() {
                                            match history::versions(&path) {
                                                Ok(paths) => {
                                                    self.history_paths = paths;
                                                    self.history_open = true;
                                                }
                                                Err(error) => {
                                                    self.status = Some((error.to_string(), true))
                                                }
                                            }
                                            ui.close_menu();
                                        }
                                        if ui.button("Unfold all blocks").clicked() {
                                            folds.unfold_all();
                                            context.request_repaint();
                                            ui.close_menu();
                                        }
                                        for (label, action) in [
                                            ("Go to definition", lsp::Feature::Definition),
                                            ("Find references", lsp::Feature::References),
                                            ("Code actions", lsp::Feature::CodeActions),
                                            ("Format document", lsp::Feature::Format),
                                        ] {
                                            if ui.button(label).clicked() {
                                                self.lsp_panel.action = Some(action);
                                                ui.close_menu();
                                            }
                                        }
                                    });
                                    if let Some(entry) =
                                        self.multiple.as_ref().filter(|entry| entry.editor == id)
                                    {
                                        paint_selections(
                                            ui,
                                            &output,
                                            &file.text,
                                            &entry.selections.ranges,
                                        );
                                    }
                                    if let Some(action) =
                                        active.then(|| self.lsp_panel.action.take()).flatten()
                                    {
                                        if let Some(cursor) = output.cursor_range {
                                            let byte = file
                                                .text
                                                .char_indices()
                                                .nth(cursor.primary.ccursor.index)
                                                .map_or(file.text.len(), |(index, _)| index);
                                            self.lsp_panel
                                                .request_at(action, &path, &file.text, byte);
                                        }
                                    }
                                    if focused && self.multiple.is_none() && self.snippet.is_none()
                                    {
                                        if let Some(cursor) = output.cursor_range {
                                            let byte = multicursor::bytes(
                                                &file.text,
                                                cursor.primary.ccursor.index
                                                    ..cursor.primary.ccursor.index,
                                            )
                                            .start;
                                            let position = output
                                                .galley
                                                .pos_from_cursor(&cursor.primary)
                                                .translate(output.galley_pos.to_vec2())
                                                .left_bottom();
                                            self.lsp_panel.inline_completion(
                                                context, &path, position, id, byte,
                                            );
                                            if output.response.changed() {
                                                let byte = multicursor::bytes(
                                                    &file.text,
                                                    cursor.primary.ccursor.index
                                                        ..cursor.primary.ccursor.index,
                                                )
                                                .start;
                                                self.lsp_panel
                                                    .queue_completion(context, &path, byte);
                                            }
                                        }
                                    }
                                    if let Some(selection) = selection {
                                        if selection.primary.index == selection.secondary.index {
                                            output.response.request_focus();
                                        }
                                        let cursor = output.galley.from_ccursor(selection.primary);
                                        let caret = output
                                            .galley
                                            .pos_from_cursor(&cursor)
                                            .translate(output.galley_pos.to_vec2());
                                        ui.scroll_to_rect(caret, None);
                                    }
                                    if modified || output.response.changed() {
                                        snippets::sync(&mut self.snippet, id, &file.text);
                                        if let Err(error) = file.sync_edit() {
                                            self.status = Some((error.to_string(), true));
                                        } else {
                                            self.recovery_due.get_or_insert_with(|| {
                                                std::time::Instant::now()
                                                    + std::time::Duration::from_millis(750)
                                            });
                                            context.request_repaint();
                                        }
                                    }
                                    let visible = overview::visible_rows(&output, ui.clip_rect());
                                    (output.galley, visible)
                                })
                                .inner
                            })
                            .inner
                    },
                )
                .inner;
            if minimap {
                let (galley, visible) = state;
                if let Some(line) = overview::show(
                    ui,
                    &galley,
                    visible,
                    self.cursor
                        .filter(|_| active || focused)
                        .map(|(line, _)| line - 1),
                ) {
                    requested_line = Some(line);
                    focused = true;
                    context.request_repaint();
                }
            }
        });
        if let Some(original_text) = original_text.filter(|original| *original != file.text) {
            let group = self
                .multiple
                .as_ref()
                .filter(|entry| entry.editor == editor_id(&path, right))
                .map(|entry| entry.started);
            let change = language_edit::Preview {
                title: if group.is_some() {
                    "Multi-cursor"
                } else {
                    "Typing"
                },
                files: vec![language_edit::FileChange {
                    path: path.clone(),
                    before: original_text,
                    after: file.text.clone(),
                    snippets: vec![],
                    edits: None,
                }],
            };
            self.timeline.record(&change, group);
            self.undo_batch = self.timeline.current(false);
            self.redo_batch = None;
        }
        (focused, requested_line)
    }

    fn review_language_edit(&mut self, context: &egui::Context) {
        if self.language_preview.is_none() && self.root.is_some() {
            if let Some(preview) = self.lsp_panel.take_preview(&self.roots(), &self.tabs) {
                match preview {
                    Ok(preview)
                        if preview.title == "Review completion"
                            && preview.files.len() == 1
                            && preview.files[0].snippets.len() == 1 =>
                    {
                        match self.apply_language_edit(&preview) {
                            Ok(()) => {
                                let change = &preview.files[0];
                                let (range, inserted) =
                                    file::changed_range(&change.before, &change.after);
                                let caret =
                                    change.after[..range.start + inserted.len()].chars().count();
                                for right in [false, true] {
                                    set_caret(context, editor_id(&change.path, right), caret);
                                }
                                self.timeline.record(&preview, None);
                                self.undo_batch = self.timeline.current(false);
                                self.redo_batch = None;
                            }
                            Err(error) => self.status = Some((error.to_string(), true)),
                        }
                    }
                    Ok(preview) => self.language_preview = Some(preview),
                    Err(error) => self.status = Some((error.to_string(), true)),
                }
            }
        }
        let Some(preview) = &self.language_preview else {
            return;
        };
        use std::hash::{Hash, Hasher};
        let mut signature = std::collections::hash_map::DefaultHasher::new();
        for file in &preview.files {
            file.path.hash(&mut signature);
            file.before.hash(&mut signature);
            file.after.hash(&mut signature);
        }
        let signature = signature.finish();
        if self.review_signature != signature {
            self.review_signature = signature;
            self.review_selection = preview
                .files
                .iter()
                .map(|file| {
                    (
                        file.path.clone(),
                        vec![true; file.edits.as_ref().map_or(1, Vec::len)],
                    )
                })
                .collect();
        }
        let mut apply = false;
        let mut test_after = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("language-preview")).show(context, |ui| {
            ui.set_width((context.screen_rect().width() - 85.0).clamp(300.0, 760.0));
            ui.heading(preview.title);
            let edits: usize = preview.files.iter().map(|file| file.snippets.len()).sum();
            ui.label(chrome::small(format!(
                "{edits} changes in {} file(s) · Review before applying",
                preview.files.len()
            )));
            ui.columns(2, |columns| {
                columns[0].label(chrome::label("ORIGINAL"));
                columns[1].label(chrome::label("PROPOSED"));
            });
            ScrollArea::vertical()
                .max_height((context.screen_rect().height() * 0.55).clamp(170.0, 500.0))
                .show(ui, |ui| {
                    for change in &preview.files {
                        let path = self
                            .root
                            .as_deref()
                            .and_then(|root| change.path.strip_prefix(root).ok())
                            .unwrap_or(&change.path);
                        ui.add(
                            egui::Label::new(RichText::new(path.display().to_string()).strong())
                                .truncate(),
                        );
                        for (index, (old, new)) in change.snippets.iter().enumerate() {
                            if let Some(selected) = self
                                .review_selection
                                .get_mut(&change.path)
                                .and_then(|selected| selected.get_mut(index))
                            {
                                ui.checkbox(selected, "Apply this change");
                            }
                            ui.columns(2, |columns| {
                                diff_cell(
                                    &mut columns[0],
                                    old,
                                    theme_red(),
                                    Color32::from_rgb(53, 30, 40),
                                );
                                diff_cell(
                                    &mut columns[1],
                                    new,
                                    theme_green(),
                                    Color32::from_rgb(26, 46, 40),
                                );
                            });
                        }
                        ui.add_space(6.0);
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(chrome::small("Files on disk change when you save."));
                chrome::right(ui, |ui| {
                    apply = ui
                        .add(
                            egui::Button::new(
                                RichText::new("Apply to Editor")
                                    .strong()
                                    .color(theme_editor()),
                            )
                            .fill(theme_accent()),
                        )
                        .clicked();
                    if self.tests.has_tests() {
                        test_after = ui.button("Apply, save & test active folder").clicked();
                        apply |= test_after;
                    }
                    cancel = ui.button("Cancel").clicked();
                });
            });
        });
        if apply {
            let proposed = self.language_preview.take().expect("preview shown");
            let preview = match language_edit::selected(&proposed, &self.review_selection) {
                Ok(preview) => preview,
                Err(error) => {
                    self.status = Some((error.to_string(), true));
                    self.language_preview = Some(proposed);
                    return;
                }
            };
            match self.apply_language_edit(&preview) {
                Ok(()) => {
                    self.timeline.record(&preview, None);
                    self.undo_batch = self.timeline.current(false);
                    self.redo_batch = None;
                    if test_after {
                        let mut saved = true;
                        for change in &preview.files {
                            if let Some(index) =
                                self.tabs.iter().position(|file| file.path == change.path)
                            {
                                saved &= self.save_tab(index);
                            }
                        }
                        if saved {
                            self.tests.queue_all();
                            self.sidebar = Sidebar::Tests;
                        }
                    }
                }
                Err(error) => self.status = Some((error.to_string(), true)),
            }
        } else if cancel || modal.should_close() {
            self.language_preview = None;
        }
    }

    fn replay_batch(&mut self, redo: bool) {
        if let Some(preview) = self.timeline.current(redo) {
            match self.apply_language_edit(&preview) {
                Ok(()) => {
                    self.timeline.completed(redo);
                    self.undo_batch = self.timeline.current(false);
                    self.redo_batch = self.timeline.current(true);
                }
                Err(error) => self.status = Some((error.to_string(), true)),
            }
            return;
        }
        let change = if redo {
            self.redo_batch.take()
        } else {
            self.undo_batch.take()
        };
        let Some(preview) = change else { return };
        match self.apply_language_edit(&preview) {
            Ok(()) => {
                let inverse = reverse_preview(preview);
                if redo {
                    self.undo_batch = Some(inverse);
                } else {
                    self.redo_batch = Some(inverse);
                }
                self.status = Some((
                    if redo {
                        "Reviewed edit restored."
                    } else {
                        "Reviewed edit undone."
                    }
                    .into(),
                    false,
                ));
            }
            Err(error) => {
                if redo {
                    self.redo_batch = Some(preview);
                } else {
                    self.undo_batch = Some(preview);
                }
                self.status = Some((error.to_string(), true));
            }
        }
    }

    fn apply_language_edit(&mut self, preview: &language_edit::Preview) -> std::io::Result<()> {
        let mut staged = Vec::with_capacity(preview.files.len());
        for change in &preview.files {
            let index = self.tabs.iter().position(|file| file.path == change.path);
            let mut file = if let Some(index) = index {
                self.tabs[index].clone()
            } else {
                OpenFile::open(&change.path)?
            };
            if file.path != change.path || file.text != change.before {
                return Err(std::io::Error::other(
                    "A file changed since the edit was reviewed. Open a new preview.",
                ));
            }
            file.text.clone_from(&change.after);
            file.sync_edit().map_err(std::io::Error::other)?;
            staged.push((index, file));
        }
        for (index, file) in staged {
            if let Some(index) = index {
                self.tabs[index] = file;
            } else {
                self.tabs.push(file);
            }
        }
        self.multiple = None;
        self.snippet = None;
        self.find.reset_selection();
        self.recovery_due = Some(std::time::Instant::now());
        self.status = Some((
            "Changes applied. Save to update files on disk.".into(),
            false,
        ));
        Ok(())
    }

    fn confirm_close(&mut self, context: &egui::Context) {
        let Some(target) = self.pending_close else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("confirm-close")).show(context, |ui| {
            ui.heading("Unsaved edits");
            if let Some((status, true)) = &self.status {
                ui.colored_label(Color32::LIGHT_RED, status);
            }
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    self.pending_close = None;
                }
                if ui.button("Discard edits").clicked() {
                    if matches!(target, CloseTarget::Window) {
                        self.save_workspace();
                        while !self.tabs.is_empty() {
                            self.close_tab(self.tabs.len() - 1);
                        }
                    }
                    self.close(target, context);
                }
                if ui.button("Save and close").clicked() {
                    let saved = match target {
                        CloseTarget::Tab(index) => self.save_tab(index),
                        CloseTarget::Window => {
                            (0..self.tabs.len()).all(|index| self.save_tab(index))
                        }
                    };
                    if saved {
                        self.close(target, context);
                    }
                }
            });
        });
        if modal.should_close() {
            self.pending_close = None;
        }
    }
}

impl eframe::App for Editor {
    fn update(&mut self, context: &egui::Context, _: &mut eframe::Frame) {
        if self.history_open {
            let mut open = true;
            let mut selected = None;
            egui::Window::new("Local file history")
                .open(&mut open)
                .show(context, |ui| {
                    ui.label("Choose an earlier saved version to review its restoration.");
                    if self.history_paths.is_empty() {
                        ui.label("No saved versions for this file yet.");
                    }
                    ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        for (index, path) in self.history_paths.iter().enumerate() {
                            let timestamp = path
                                .metadata()
                                .and_then(|meta| meta.modified())
                                .ok()
                                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                                .map_or(0, |time| time.as_secs());
                            if ui
                                .push_id(path, |ui| {
                                    ui.button(format!("Saved version · {timestamp}"))
                                })
                                .inner
                                .clicked()
                            {
                                selected = Some(index);
                            }
                        }
                    });
                });
            self.history_open = open;
            if let Some(index) = selected {
                match recovery::read(&self.history_paths[index]) {
                    Ok(record) => {
                        if let Some(file) = self.tabs.iter().find(|file| file.path == record.path) {
                            self.language_preview = Some(language_edit::Preview {
                                title: "Review historical version",
                                files: vec![language_edit::FileChange {
                                    path: record.path,
                                    before: file.text.clone(),
                                    after: record.text.clone(),
                                    snippets: vec![(file.text.clone(), record.text)],
                                    edits: None,
                                }],
                            });
                            self.history_open = false;
                        } else {
                            self.status = Some((
                                "Open the original file before restoring its history.".into(),
                                true,
                            ));
                        }
                    }
                    Err(error) => self.status = Some((error.to_string(), true)),
                }
            }
        }
        if self.settings_open {
            let mut open = self.settings_open;
            egui::Window::new("Appearance")
                .open(&mut open)
                .resizable(false)
                .show(context, |ui| {
                    let mut theme_changed = false;
                    ui.menu_button("Installed themes", |ui| {
                        for (extension, theme) in self.packages.themes() {
                            if ui.button(format!("{} · {extension}", theme.name)).clicked() {
                                self.preferences.light = !theme.dark;
                                self.preferences.custom_theme = Some(theme.clone());
                                theme_changed = true;
                                ui.close_menu();
                            }
                        }
                    });
                    if self.preferences.show(ui) || theme_changed {
                        self.preferences.apply(context);
                        self.layout_cache = [None, None];
                        if let Err(error) = self.preferences.save() {
                            self.status = Some((error.to_string(), true));
                        }
                    }
                });
            self.settings_open = open;
        }
        self.packages
            .set_workspace(self.tool_root().map(Path::to_owned));
        self.packages.poll(context);
        self.accounts.tick(context);
        self.view_shortcuts(context);
        self.sync_tool_root();
        self.poll_workspace_search();
        if self.language_preview.is_none()
            && self.pending_close.is_none()
            && self.palette.mode.is_none()
        {
            self.language_preview = self.workspace_review.take();
        }
        if let Some(receiver) = &self.scan {
            match receiver.try_recv() {
                Ok(list) => {
                    self.listing_incomplete = !list.complete || list.limited || list.skipped;
                    self.entries = list.paths;
                    self.directories = list.directories;
                    self.palette.refresh();
                    if list.complete {
                        self.scan = None;
                    }
                    if let Some(error) = list.error {
                        self.status = Some((error, true));
                    } else if list.limited {
                        self.status = Some((
                            "Explorer reached its 10,000-file or directory limit.".to_owned(),
                            true,
                        ));
                    } else if list.skipped {
                        self.status = Some(("Some paths could not be listed.".to_owned(), true));
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scan = None;
                    self.listing_incomplete = true;
                    self.status = Some(("The file listing stopped unexpectedly.".to_owned(), true));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        if context.input(|input| input.viewport().close_requested()) {
            if self.tabs.iter().any(OpenFile::is_dirty) {
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.pending_close = Some(CloseTarget::Window);
            } else {
                self.save_workspace();
            }
        }
        let terminal_focused = self.terminal_open
            && !self.terminal.is_empty()
            && context.memory(|memory| memory.has_focus(terminal::input_id()));
        if self.pending_close.is_none()
            && self.language_preview.is_none()
            && !terminal_focused
            && !self.workspace_files.is_open()
        {
            let palettes = context.input_mut(|input| {
                input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::P)
            });
            if palettes {
                self.palette.open(workbench::Mode::Commands);
            } else if shortcut(context, Key::P) {
                self.show_quick_open();
            }
            if self.palette.mode.is_none() {
                if let Some(file) = self.active.map(|index| &self.tabs[index]).filter(|file| {
                    context.memory(|memory| {
                        [false, true]
                            .into_iter()
                            .any(|right| memory.has_focus(editor_id(&file.path, right)))
                    })
                }) {
                    let matches = |batch: &Option<language_edit::Preview>| {
                        batch.as_ref().is_some_and(|batch| {
                            batch.files.iter().any(|change| change.path == file.path)
                                && batch.files.iter().all(|change| {
                                    self.tabs.iter().any(|tab| {
                                        tab.path == change.path && tab.text == change.before
                                    })
                                })
                        })
                    };
                    let undo_ready = matches(&self.undo_batch);
                    let redo_ready = matches(&self.redo_batch);
                    if context.input_mut(|input| {
                        input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                    }) {
                        if redo_ready {
                            self.replay_batch(true);
                        }
                    } else if shortcut(context, Key::Z) && undo_ready {
                        self.replay_batch(false);
                    }
                }
                if shortcut(context, Key::Backslash) {
                    self.toggle_split(context);
                }
                if shortcut(context, Key::O) {
                    self.open_dialog();
                }
                if shortcut(context, Key::S) {
                    if let Some(index) = self.active {
                        self.save_tab(index);
                    }
                }
                if self.active.is_some() && shortcut(context, Key::F) {
                    self.find.open(false);
                }
                if shortcut(context, Key::G) {
                    self.find.next();
                }
                if shortcut(context, Key::W) {
                    self.request_tab_close(context);
                }
            }
        }

        self.top_bar(context);
        egui::TopBottomPanel::bottom("status")
            .frame(
                chrome::panel(theme_rail(), 12, 5).stroke(egui::Stroke::new(1.0, theme_border())),
            )
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    let problems = self.lsp_panel.problem_count();
                    if ui.small_button(format!("Problems {problems}")).clicked() {
                        self.problems_open = true;
                    }
                    if let Some(summary) = self.git_panel.branch_summary() {
                        if ui
                            .small_button(summary)
                            .on_hover_text("Open Source Control")
                            .clicked()
                        {
                            self.sidebar = Sidebar::Git;
                        }
                    }
                    if let Some((message, error)) = &self.status {
                        let width = (ui.available_width() - 230.0).max(90.0);
                        ui.scope(|ui| {
                            ui.set_max_width(width);
                            ui.add(
                                egui::Label::new(chrome::small(message).color(if *error {
                                    theme_red()
                                } else {
                                    theme_green()
                                }))
                                .truncate(),
                            )
                            .on_hover_text(message);
                        });
                    } else if let Some(root) = &self.root {
                        ui.label(chrome::small(file_label(root)))
                            .on_hover_text(root.display().to_string());
                    }
                    if let Some(file) = self.active.map(|index| &self.tabs[index]) {
                        chrome::right(ui, |ui| {
                            let (_, language, _) = chrome::file_kind(&file.path);
                            ui.label(chrome::small(language));
                            ui.separator();
                            ui.label(chrome::small("UTF-8"));
                            if let Some((line, column)) = self.cursor {
                                ui.separator();
                                ui.label(chrome::small(format!("Ln {line}, Col {column}")));
                            }
                        });
                    }
                });
            });
        self.activity_rail(context);
        self.explorer(context);
        self.sync_tool_root();
        self.packages
            .set_workspace(self.tool_root().map(Path::to_owned));
        if self.terminal_open {
            self.terminal_panel(context);
        }
        self.editor(context);
        self.sync_tool_root();
        if self.pending_close.is_none() && self.language_preview.is_none() {
            self.show_palette(context);
        }
        self.confirm_close(context);
        self.watch_open_files(context);
        self.flush_recovery(context);
        let active_in_workspace = self
            .active
            .is_some_and(|index| self.root_for(&self.tabs[index].path).is_some());
        let language_file = self.active.filter(|_| active_in_workspace).map(|index| {
            let file = &self.tabs[index];
            (file.path.as_path(), file.text.as_str())
        });
        for (root, tools) in &mut self.toolsets {
            tools.language.tick(context, None, &self.tabs);
            tools.debugger.tick(context);
            tools.git.tick(context);
            tools
                .tests
                .tick(context, testing::revision(&self.tabs, root));
            tools.extensions.sync_packages(&self.packages, Some(root));
        }
        self.lsp_panel.tick(context, language_file, &self.tabs);
        if let Some((path, line)) = self.lsp_panel.destination.take() {
            if self.root_for(&path).is_some() {
                self.open_file(&path);
                self.pending_jump = self
                    .active
                    .filter(|index| self.tabs[*index].path == path)
                    .map(|_| line);
                context.request_repaint();
            } else {
                self.status = Some(("Definition is outside the workspace.".into(), false));
            }
        }

        self.debugger.tick(context);
        self.git_panel.tick(context);
        let location = self.debugger.stopped_location();
        if self.pending_close.is_none()
            && self.language_preview.is_none()
            && location != self.debug_location
        {
            if let Some((path, line)) = &location {
                if self.root_for(path).is_some() {
                    self.open_file(path);
                    self.pending_jump = Some(*line);
                }
            }
            self.debug_location = location;
        }
        if let Some(root) = self.tools_root.as_deref() {
            self.tests
                .tick(context, testing::revision(&self.tabs, root));
        }
        self.extension_panel
            .sync_packages(&self.packages, self.tools_root.as_deref());
        if self.language_preview.is_none() {
            self.language_preview = self.extension_panel.poll(context);
        }
        if let Some(error) = self.extension_panel.take_error() {
            self.status = Some((error, true));
        }
        self.workspace_operations(context);
        self.show_problems(context);
        if self.accounts_open {
            let mut open = true;
            egui::Window::new("GitHub accounts")
                .id(egui::Id::new("accounts-window"))
                .default_width(440.0)
                .open(&mut open)
                .show(context, |ui| self.accounts.show(ui));
            self.accounts_open = open;
        }
        self.review_language_edit(context);
    }
}

enum ExplorerAction {
    Add(PathBuf),
    Remove(PathBuf),
    OpenSide(PathBuf),
    Jump(PathBuf, usize),
    Open(PathBuf),
    Refresh,
}

fn diff_cell(ui: &mut egui::Ui, text: &str, color: Color32, background: Color32) {
    let background = if ui.visuals().dark_mode {
        background
    } else {
        ui.visuals().faint_bg_color
    };
    chrome::panel(background, 9, 5)
        .corner_radius(5)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let contents = if text.is_empty() { "∅" } else { text };
            ui.add(egui::Label::new(RichText::new(contents).monospace().color(color)).wrap());
        });
}

fn reverse_preview(mut preview: language_edit::Preview) -> language_edit::Preview {
    for file in &mut preview.files {
        file.edits = None;
        std::mem::swap(&mut file.before, &mut file.after);
        for (before, after) in &mut file.snippets {
            std::mem::swap(before, after);
        }
    }
    preview
}

fn paint_gutter(
    ui: &egui::Ui,
    gutter: egui::Rect,
    output: &egui::text_edit::TextEditOutput,
    highlight: egui::layers::ShapeIdx,
    breakpoints: &[u32],
    diagnostics: &[(usize, String)],
) -> Option<u32> {
    let mut toggled = None;
    let selected = output.cursor_range.map(|range| range.primary.rcursor.row);
    let clip = ui.clip_rect();
    let origin = output.galley_pos;
    let rows = &output.galley.rows;
    let first = rows.partition_point(|row| origin.y + row.rect.max.y < clip.min.y);
    for (index, row) in rows.iter().enumerate().skip(first) {
        let rect = row.rect.translate(origin.to_vec2());
        if rect.min.y > clip.max.y {
            break;
        }
        if row.rect.height() < 1.0 {
            continue;
        }
        if Some(index) == selected {
            let line = egui::Rect::from_min_max(
                egui::pos2(gutter.min.x, rect.min.y),
                egui::pos2(clip.max.x, rect.max.y),
            );
            ui.painter().set(
                highlight,
                egui::Shape::rect_filled(line, 0.0, chrome::theme_elevated()),
            );
        }
        let hit = egui::Rect::from_min_max(
            egui::pos2(gutter.min.x, rect.min.y),
            egui::pos2(gutter.min.x + 16.0, rect.max.y),
        );
        let response = ui.interact(
            hit,
            output.response.id.with(("breakpoint", index)),
            egui::Sense::click(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                true,
                breakpoints.contains(&((index + 1) as u32)),
                format!("Breakpoint line {}", index + 1),
            )
        });
        if response.clicked() {
            toggled = Some((index + 1) as u32);
        }
        if let Some((_, message)) = diagnostics.iter().find(|(line, _)| *line == index + 1) {
            let marker = egui::Rect::from_min_size(
                egui::pos2(gutter.max.x + 3.0, rect.min.y),
                egui::vec2(3.0, rect.height()),
            );
            ui.painter().rect_filled(marker, 1.0, theme_red());
            ui.interact(
                marker.expand(3.0),
                output.response.id.with(("diagnostic", index)),
                egui::Sense::hover(),
            )
            .on_hover_text(message);
        }
        if breakpoints.contains(&((index + 1) as u32)) || response.hovered() {
            ui.painter().circle_filled(
                hit.center(),
                4.0,
                if response.hovered() {
                    theme_muted()
                } else {
                    theme_red()
                },
            );
        }
        ui.painter().text(
            egui::pos2(gutter.max.x - 2.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            (index + 1).to_string(),
            FontId::monospace(11.0),
            if Some(index) == selected {
                theme_accent()
            } else {
                chrome::theme_dim()
            },
        );
    }
    ui.painter().vline(
        gutter.max.x + 6.0,
        clip.y_range(),
        egui::Stroke::new(1.0, theme_border()),
    );
    toggled
}

struct ExplorerRow<'a> {
    path: &'a Path,
    name: String,
    depth: usize,
    folder: bool,
}

fn folder_open(
    path: &Path,
    folders: &std::collections::HashMap<PathBuf, bool>,
    active: Option<&Path>,
) -> bool {
    folders
        .get(path)
        .copied()
        .unwrap_or_else(|| active.is_some_and(|active| active.starts_with(path)))
}

fn explorer_rows<'a>(
    root: &Path,
    files: &'a [PathBuf],
    directories: &'a [PathBuf],
    filter: &str,
    folders: &std::collections::HashMap<PathBuf, bool>,
    active: Option<&Path>,
) -> Vec<ExplorerRow<'a>> {
    let mut nodes = std::collections::BTreeMap::<&'a Path, bool>::new();
    for (path, is_folder) in files
        .iter()
        .map(|path| (path.as_path(), false))
        .chain(directories.iter().map(|path| (path.as_path(), true)))
    {
        if path == root || path.strip_prefix(root).is_err() {
            continue;
        }
        nodes.insert(path, is_folder);
        for parent in path
            .ancestors()
            .skip(1)
            .take_while(|parent| *parent != root)
        {
            nodes.insert(parent, true);
        }
    }
    let filter = filter.trim().to_lowercase();
    if !filter.is_empty() {
        return nodes
            .into_iter()
            .filter_map(|(path, folder)| {
                let name = path.strip_prefix(root).ok()?.to_string_lossy();
                name.to_lowercase().contains(&filter).then(|| ExplorerRow {
                    path,
                    name: name.into_owned(),
                    depth: 0,
                    folder,
                })
            })
            .collect();
    }
    let mut children = std::collections::BTreeMap::<&Path, Vec<(&'a Path, bool)>>::new();
    for (path, folder) in nodes {
        if let Some(parent) = path.parent() {
            children.entry(parent).or_default().push((path, folder));
        }
    }
    for children in children.values_mut() {
        children.sort_unstable_by(|(left, left_folder), (right, right_folder)| {
            right_folder
                .cmp(left_folder)
                .then_with(|| left.file_name().cmp(&right.file_name()))
        });
    }
    let mut pending = children
        .get(root)
        .into_iter()
        .flatten()
        .rev()
        .map(|(path, folder)| (*path, *folder, 0))
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    while let Some((path, folder, depth)) = pending.pop() {
        rows.push(ExplorerRow {
            path,
            name: file_label(path),
            depth,
            folder,
        });
        if folder && folder_open(path, folders, active) {
            if let Some(children) = children.get(path) {
                pending.extend(
                    children
                        .iter()
                        .rev()
                        .map(|(path, folder)| (*path, *folder, depth + 1)),
                );
            }
        }
    }
    rows
}

fn list_files(root: &Path) -> FileListing {
    const MAX_ENTRIES: usize = 10_000;
    let mut paths = Vec::new();
    let mut directories = Vec::new();
    let exclusions = match project::Exclusions::load(root) {
        Ok(exclusions) => exclusions,
        Err(error) => {
            return FileListing {
                paths,
                directories,
                limited: false,
                skipped: true,
                error: Some(error.to_string()),
                complete: true,
            };
        }
    };
    let mut pending = vec![root.to_path_buf()];
    let mut visited = 0;
    let mut skipped = false;
    while let Some(directory) = pending.pop() {
        let Ok(entries) = directory.read_dir() else {
            skipped = true;
            continue;
        };
        for entry in entries {
            if visited == MAX_ENTRIES {
                paths.sort_unstable();
                return FileListing {
                    paths,
                    directories,
                    limited: true,
                    skipped,
                    error: None,
                    complete: true,
                };
            }
            visited += 1;
            let Ok(entry) = entry else {
                skipped = true;
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                skipped = true;
                continue;
            };
            if exclusions.matches(root, &entry.path()) {
                continue;
            }
            if kind.is_dir() {
                directories.push(entry.path());
                pending.push(entry.path());
            } else if kind.is_file() {
                paths.push(entry.path());
            }
        }
    }
    paths.sort_unstable();
    FileListing {
        paths,
        directories,
        limited: false,
        skipped,
        error: None,
        complete: true,
    }
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn edit_keyboard(
    context: &egui::Context,
    id: egui::Id,
    file: &mut OpenFile,
    multiple: &mut Option<MultiEdit>,
    undo: &mut Option<language_edit::Preview>,
    redo: &mut Option<language_edit::Preview>,
    status: &mut Option<(String, bool)>,
) -> bool {
    let selection_bytes = |text: &str| {
        let state = TextEdit::load_state(context, id)?;
        let [start, end] = state.cursor.char_range()?.sorted();
        Some(multicursor::bytes(text, start.index..end.index))
    };
    if context.input_mut(|input| input.consume_key(Modifiers::COMMAND | Modifiers::ALT, Key::D)) {
        if let Some(selections) = selection_bytes(&file.text)
            .and_then(|range| multicursor::Selections::rectangle(&file.text, range))
        {
            *status = Some((
                format!("{} rows selected · Esc to finish", selections.ranges.len()),
                false,
            ));
            *multiple = Some(MultiEdit {
                editor: id,
                selections,
                original: None,
                started: std::time::Instant::now(),
            });
        }
        return false;
    }
    let current_group = multiple.as_ref().is_some_and(|entry| entry.editor == id);
    if shortcut(context, Key::D) {
        if current_group {
            if let Some(entry) = multiple.as_mut().filter(|entry| entry.original.is_none()) {
                entry.selections.add_next(&file.text);
            }
        } else if let Some(selections) = selection_bytes(&file.text)
            .and_then(|selection| multicursor::Selections::new(&file.text, selection))
        {
            *multiple = Some(MultiEdit {
                editor: id,
                selections,
                original: None,
                started: std::time::Instant::now(),
            });
        }
        if let Some(entry) = multiple.as_ref().filter(|entry| entry.editor == id) {
            *status = Some((
                format!("{} cursors · Esc to finish", entry.selections.ranges.len()),
                false,
            ));
        }
        return false;
    }

    let mut canceled = false;
    enum MultipleInput {
        Text(String),
        Delete { backwards: bool },
    }
    let mut replacements = Vec::new();
    if current_group {
        context.input_mut(|input| {
            input.events.retain(|event| match event {
                egui::Event::Text(text) | egui::Event::Paste(text) => {
                    replacements.push(MultipleInput::Text(text.clone()));
                    false
                }
                egui::Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                } => {
                    canceled = true;
                    false
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if *modifiers == Modifiers::NONE
                    && matches!(key, Key::Backspace | Key::Delete) =>
                {
                    replacements.push(MultipleInput::Delete {
                        backwards: *key == Key::Backspace,
                    });
                    false
                }
                egui::Event::PointerButton { pressed: true, .. }
                | egui::Event::Ime(_)
                | egui::Event::Key {
                    key:
                        Key::ArrowUp
                        | Key::ArrowDown
                        | Key::ArrowLeft
                        | Key::ArrowRight
                        | Key::Home
                        | Key::End
                        | Key::PageUp
                        | Key::PageDown
                        | Key::Tab
                        | Key::Enter,
                    pressed: true,
                    ..
                } => {
                    canceled = true;
                    true
                }
                egui::Event::Key {
                    key,
                    modifiers,
                    pressed: true,
                    ..
                } if modifiers.command && *key != Key::V => {
                    canceled = true;
                    true
                }
                _ => true,
            });
        });
        if canceled {
            *multiple = None;
            *status = None;
            return false;
        }
    }
    let mut changed = false;
    if let Some(entry) = multiple.as_mut().filter(|entry| entry.editor == id) {
        for replacement in replacements {
            let text = match replacement {
                MultipleInput::Text(text) => entry.selections.replace(&file.text, &text),
                MultipleInput::Delete { backwards } => {
                    // After replacing ranges the cursors are distinct insertion points.
                    entry.selections.delete(&file.text, backwards)
                }
            };
            if let Some(text) = text {
                if text == file.text {
                    continue;
                }
                let original = entry.original.get_or_insert_with(|| file.text.clone());
                file.text = text;
                *undo = Some(language_edit::Preview {
                    title: "Undo multi-cursor edit",
                    files: vec![language_edit::FileChange {
                        path: file.path.clone(),
                        before: file.text.clone(),
                        after: original.clone(),
                        snippets: vec![],
                        edits: None,
                    }],
                });
                *redo = None;
                let offset = entry.selections.ranges[0].start;
                set_caret(context, id, file.text[..offset].chars().count());
                changed = true;
            } else {
                *status = Some(("The edit exceeds the file size limit.".into(), true));
            }
        }
    } else if let Some(range) = selection_bytes(&file.text)
        .filter(|_| context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter)))
    {
        let replacement = multicursor::indentation(&file.text, range.start);
        if file.text.len() - range.len() + replacement.len() <= 10 * 1024 * 1024 {
            let caret = file.text[..range.start].chars().count() + replacement.chars().count();
            let original = file.text.clone();
            file.text.replace_range(range, &replacement);
            set_caret(context, id, caret);
            *undo = Some(language_edit::Preview {
                title: "Undo auto-indent",
                files: vec![language_edit::FileChange {
                    path: file.path.clone(),
                    before: file.text.clone(),
                    after: original,
                    snippets: vec![],
                    edits: None,
                }],
            });
            *redo = None;
            changed = true;
        }
    }
    changed
}

fn set_caret(context: &egui::Context, id: egui::Id, index: usize) {
    let mut state = TextEdit::load_state(context, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text_selection::CCursorRange::one(
            egui::text::CCursor::new(index),
        )));
    TextEdit::store_state(context, id, state);
}

fn paint_selections(
    ui: &egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    text: &str,
    ranges: &[std::ops::Range<usize>],
) {
    let painter = ui
        .painter()
        .with_clip_rect(output.text_clip_rect.intersect(ui.clip_rect()));
    let entries = ranges.iter().flat_map(|range| [range.start, range.end]);
    let mut chars = text.char_indices().peekable();
    let mut index = 0;
    let mut coordinates = entries.map(|byte| {
        while chars.peek().is_some_and(|(offset, _)| *offset < byte) {
            chars.next();
            index += 1;
        }
        let cursor = output.galley.from_ccursor(egui::text::CCursor::new(index));
        output
            .galley
            .pos_from_cursor(&cursor)
            .translate(output.galley_pos.to_vec2())
    });
    while let (Some(start), Some(end)) = (coordinates.next(), coordinates.next()) {
        let left = start.left().min(end.left());
        let right = start.left().max(end.left()).max(left + 1.4);
        let rect = egui::Rect::from_min_max(
            egui::pos2(left, start.top()),
            egui::pos2(right, start.bottom()),
        );
        painter.rect_stroke(
            rect,
            1.0,
            egui::Stroke::new(1.0, theme_accent()),
            egui::StrokeKind::Inside,
        );
    }
}

fn shortcut(context: &egui::Context, key: Key) -> bool {
    context.input_mut(|input| input.consume_key(Modifiers::COMMAND, key))
}

#[cfg(test)]
mod editor_tests {
    use super::*;

    #[test]
    fn search_keeps_partial_marker_after_indexing_finishes() -> std::io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().canonicalize()?;
        let path = root.join("found.txt");
        std::fs::write(&path, "needle")?;
        let mut editor = Editor {
            root: Some(root),
            entries: vec![path],
            listing_incomplete: true,
            workspace_query: "needle".into(),
            ..Editor::default()
        };
        editor.start_workspace_search(&egui::Context::default());
        editor.listing_incomplete = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while editor.workspace_search.is_some() {
            editor.poll_workspace_search();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let results = editor.search_results.expect("search results");
        assert_eq!(results.matches.len(), 1);
        assert!(results.capped);
        Ok(())
    }

    #[test]
    fn multiple_roots_keep_tools_and_replacement_scope_separate() -> std::io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().canonicalize()?;
        let first = root.join("api");
        let second = root.join("web");
        let outside = root.join("private");
        for path in [&first, &second, &outside] {
            std::fs::create_dir(path)?;
            std::fs::write(path.join("main.rs"), "needle")?;
        }
        std::fs::create_dir(first.join(".forge"))?;
        std::fs::write(
            first.join(".forge/tasks.json"),
            r#"{"version":1,"tasks":[{"label":"test","kind":"test","program":"cargo","args":["test"]}]}"#,
        )?;
        let context = egui::Context::default();
        let mut editor = Editor::default();
        editor.open_folder(first.clone(), &context);
        editor.open_file(&first.join("main.rs"));
        editor.tests.load(&first)?;
        assert!(editor.tests.has_tests());
        editor.add_folder(second.clone(), &context);
        editor.open_file(&second.join("main.rs"));
        editor.sync_tool_root();
        assert_eq!(editor.tools_root.as_ref(), Some(&second));
        assert!(!editor.tests.has_tests());
        editor.tabs[1].text = "needle unsaved".into();
        editor.tabs[1].sync_edit().map_err(std::io::Error::other)?;
        let request = search::Request {
            query: "needle".into(),
            regex: false,
            ignore_case: false,
            path_filter: String::new(),
            replacement: Some("updated".into()),
        };
        let paths = vec![
            first.join("main.rs"),
            second.join("main.rs"),
            outside.join("main.rs"),
        ];
        let buffers = std::collections::HashMap::from([(
            second.join("main.rs"),
            Some("needle unsaved".to_owned()),
        )]);
        let result = search::run(
            &editor.roots(),
            &paths,
            &buffers,
            request,
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        let search::Outcome::Review { preview, .. } = result else {
            panic!("expected preview")
        };
        assert_eq!(preview.files.len(), 2);
        editor.apply_language_edit(&preview)?;
        assert_eq!(editor.tabs[1].text, "updated unsaved");
        assert_eq!(std::fs::read_to_string(outside.join("main.rs"))?, "needle");
        editor.set_active(0);
        editor.sync_tool_root();
        assert!(editor.tests.has_tests());
        editor.remove_folder(&second, &context);
        assert!(editor.root_for(&second.join("main.rs")).is_none());
        assert_eq!(editor.tabs.len(), 2);
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn terminal_panel_keeps_space_for_the_editor_across_frames()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let context = egui::Context::default();
        chrome::apply(&context);
        let mut editor = Editor {
            root: Some(directory.path().to_owned()),
            ..Editor::default()
        };
        editor
            .terminal
            .push(terminal::Terminal::start(directory.path(), &context)?);
        let mut available = Vec::new();
        for _ in 0..40 {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1180.0, 760.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    editor.terminal_panel(ctx);
                    available.push(ctx.available_rect().height());
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.label("Editor stays visible");
                    });
                },
            );
        }
        assert!(
            available.iter().all(|height| *height >= 400.0),
            "{available:?}"
        );
        assert!((available[0] - available[39]).abs() < 5.0, "{available:?}");
        Ok(())
    }

    #[test]
    fn save_all_saves_uncontested_files_and_keeps_conflicting_edits() -> std::io::Result<()> {
        let directory = tempfile::tempdir()?;
        let left = directory.path().join("left.rs");
        let right = directory.path().join("right.rs");
        std::fs::write(&left, "original left")?;
        std::fs::write(&right, "original right")?;
        let mut editor = Editor::default();
        editor.open_file(&left);
        editor.open_file(&right);
        for file in &mut editor.tabs {
            file.text = "editor edit".into();
            file.sync_edit().map_err(std::io::Error::other)?;
        }
        std::fs::write(&left, "external edit")?;
        editor.save_all();
        assert_eq!(std::fs::read_to_string(left)?, "external edit");
        assert_eq!(std::fs::read_to_string(right)?, "editor edit");
        assert!(editor.tabs[0].is_dirty());
        assert!(!editor.tabs[1].is_dirty());
        assert!(editor.status.is_some_and(|(_, error)| error));
        Ok(())
    }

    #[test]
    fn typing_in_the_right_pane_routes_save_to_that_document() -> std::io::Result<()> {
        let directory = tempfile::tempdir()?;
        let left = directory.path().join("left.rs");
        let right = directory.path().join("right.rs");
        std::fs::write(&left, "left")?;
        std::fs::write(&right, "right")?;
        let mut editor = Editor::default();
        editor.open_file(&left);
        editor.open_file(&right);
        editor.set_active(0);
        editor.split = Some(SplitEditor {
            companion: 1,
            active_right: false,
            ratio: 0.5,
        });
        let context = egui::Context::default();
        chrome::apply(&context);
        let screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1100.0, 760.0),
        ));
        let _ = context.run(
            egui::RawInput {
                screen_rect,
                ..Default::default()
            },
            |ctx| editor.editor(ctx),
        );
        let right_id = editor_id(&editor.tabs[1].path, true);
        assert!(TextEdit::load_state(&context, right_id).is_some());
        context.memory_mut(|memory| memory.request_focus(right_id));
        let _ = context.run(
            egui::RawInput {
                screen_rect,
                events: vec![egui::Event::Text("Δ".into())],
                ..Default::default()
            },
            |ctx| editor.editor(ctx),
        );
        assert_eq!(editor.active, Some(1));
        assert!(editor.tabs[1].text.contains('Δ'));
        assert!(editor.save_tab(editor.active.expect("focused tab")));
        assert_eq!(std::fs::read_to_string(left)?, "left");
        assert!(std::fs::read_to_string(right)?.contains('Δ'));
        Ok(())
    }

    #[test]
    fn keyboard_multicursor_replaces_occurrences_and_undo_keeps_disk_intact() -> std::io::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("multi.rs");
        std::fs::write(&path, "alpha + alpha")?;
        let mut editor = Editor::default();
        editor.open_file(&path);
        let context = egui::Context::default();
        chrome::apply(&context);
        let frame = |editor: &mut Editor, events| {
            let _ = context.run(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 760.0),
                    )),
                    ..Default::default()
                },
                |ctx| editor.editor(ctx),
            );
        };
        frame(&mut editor, vec![]);
        let id = editor_id(&editor.tabs[0].path, false);
        context.memory_mut(|memory| memory.request_focus(id));
        set_caret(&context, id, 1);
        let key = |key, modifiers| egui::Event::Key {
            key,
            modifiers,
            physical_key: None,
            pressed: true,
            repeat: false,
        };
        for _ in 0..2 {
            frame(&mut editor, vec![key(Key::D, Modifiers::COMMAND)]);
        }
        frame(
            &mut editor,
            vec![key(Key::B, Modifiers::NONE), egui::Event::Text("β".into())],
        );
        assert_eq!(editor.tabs[0].text, "β + β");
        editor.replay_batch(false);
        assert_eq!(editor.tabs[0].text, "alpha + alpha");
        assert_eq!(std::fs::read_to_string(path)?, "alpha + alpha");
        Ok(())
    }

    #[test]
    fn tab_expands_snippet_and_navigates_updated_fields() -> std::io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("snippet.rs");
        std::fs::write(&path, "fn")?;
        let mut editor = Editor::default();
        editor.open_file(&path);
        let context = egui::Context::default();
        chrome::apply(&context);
        let frame = |editor: &mut Editor, events| {
            let _ = context.run(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 760.0),
                    )),
                    ..Default::default()
                },
                |ctx| editor.editor(ctx),
            );
        };
        frame(&mut editor, vec![]);
        let id = editor_id(&editor.tabs[0].path, false);
        context.memory_mut(|memory| memory.request_focus(id));
        // Present the focused editor once so egui installs its Tab focus lock.
        frame(&mut editor, vec![]);
        set_caret(&context, id, 2);
        let tab = || egui::Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        frame(&mut editor, vec![tab()]);
        assert_eq!(editor.tabs[0].text, "fn name() {\n    \n}");
        assert!(
            context.memory(|memory| memory.has_focus(id)),
            "editor lost focus after expansion"
        );
        frame(&mut editor, vec![egui::Event::Text("build".into())]);
        assert!(
            editor.tabs[0].text.contains("build"),
            "typing failed, focus {:?}, state {:?}",
            context.memory(|memory| memory.focused()),
            TextEdit::load_state(&context, id).and_then(|state| state.cursor.char_range())
        );
        frame(&mut editor, vec![tab()]);
        frame(&mut editor, vec![egui::Event::Text("value: u32".into())]);
        assert_eq!(editor.tabs[0].text, "fn build(value: u32) {\n    \n}");
        assert!(editor.snippet.is_some());
        assert_eq!(std::fs::read_to_string(path)?, "fn");
        Ok(())
    }

    #[test]
    fn focus_switch_and_tab_removal_keep_the_correct_split_buffers() -> std::io::Result<()> {
        let directory = tempfile::tempdir()?;
        let paths = ["first.rs", "left.rs", "right.rs"].map(|name| directory.path().join(name));
        let mut editor = Editor::default();
        for path in &paths {
            std::fs::write(path, "sample")?;
            editor.open_file(path);
        }
        editor.set_active(1);
        editor.split = Some(SplitEditor {
            companion: 2,
            active_right: false,
            ratio: 0.6,
        });
        editor.set_active(2);
        let split = editor.split.expect("split retained");
        assert_eq!(editor.active, Some(2));
        assert_eq!(split.companion, 1);
        assert!(split.active_right);
        editor.close_tab(0);
        assert_eq!(editor.active, Some(1));
        assert_eq!(editor.split.expect("split reindexed").companion, 0);
        editor.close_tab(1);
        assert_eq!(editor.active, Some(0));
        assert!(editor.split.is_none());
        assert!(editor.tabs[0].path.ends_with("left.rs"));
        Ok(())
    }

    #[test]
    fn explorer_expands_active_ancestors_and_filters_collapsed_children() {
        let root = Path::new("workspace");
        let files = [
            root.join("docs/readme.md"),
            root.join("src/nested/code.rs"),
            root.join("src/main.rs"),
        ];
        let active = Some(files[1].as_path());
        let mut folders = std::collections::HashMap::new();
        let rows = explorer_rows(root, &files, &[], "", &folders, active);
        let displayed = rows
            .iter()
            .map(|row| (row.name.as_str(), row.depth))
            .collect::<Vec<_>>();
        assert_eq!(
            displayed,
            [
                ("docs", 0),
                ("src", 0),
                ("nested", 1),
                ("code.rs", 2),
                ("main.rs", 1)
            ]
        );
        folders.insert(root.join("src"), false);
        assert_eq!(
            explorer_rows(root, &files, &[], "", &folders, active).len(),
            2
        );
        let filtered = explorer_rows(root, &files, &[], "CODE.RS", &folders, active);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].path, files[1]);
    }

    #[test]
    fn undo_and_redo_reviewed_replacement_preserve_the_saved_file() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().canonicalize()?.join("main.rs");
        std::fs::write(&path, "old")?;
        let mut app = Editor::default();
        app.open_file(&path);
        let preview = language_edit::Preview {
            title: "Replace",
            files: vec![language_edit::replace(
                path.clone(),
                "old".into(),
                vec![(0..3, "new".into())],
            )?],
        };
        app.apply_language_edit(&preview)?;
        app.undo_batch = Some(reverse_preview(preview));
        app.replay_batch(false);
        assert_eq!(app.tabs[0].text, "old");
        assert!(!app.tabs[0].is_dirty());
        app.replay_batch(true);
        assert_eq!(app.tabs[0].text, "new");
        assert!(app.tabs[0].is_dirty());
        assert_eq!(std::fs::read_to_string(path)?, "old");
        Ok(())
    }

    #[test]
    fn undo_chain_preserves_typing_before_reviewed_change() -> std::io::Result<()> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().canonicalize()?.join("chain.rs");
        std::fs::write(&path, "original")?;
        let mut app = Editor::default();
        app.open_file(&path);
        let typed = language_edit::Preview {
            title: "Typing",
            files: vec![language_edit::replace(
                path.clone(),
                "original".into(),
                vec![(8..8, " typed".into())],
            )?],
        };
        app.apply_language_edit(&typed)?;
        app.timeline.record(&typed, None);
        let reviewed = language_edit::Preview {
            title: "Format",
            files: vec![language_edit::replace(
                path.clone(),
                "original typed".into(),
                vec![(0..8, "formatted".into())],
            )?],
        };
        app.apply_language_edit(&reviewed)?;
        app.timeline.record(&reviewed, None);
        app.replay_batch(false);
        assert_eq!(app.tabs[0].text, "original typed");
        app.replay_batch(false);
        assert_eq!(app.tabs[0].text, "original");
        app.replay_batch(true);
        app.replay_batch(true);
        assert_eq!(app.tabs[0].text, "formatted typed");
        assert_eq!(std::fs::read_to_string(path)?, "original");
        Ok(())
    }

    #[test]
    fn stale_second_file_does_not_apply_first_file() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let first = root.path().canonicalize()?.join("first.rs");
        let second = root.path().canonicalize()?.join("second.rs");
        std::fs::write(&first, "old")?;
        std::fs::write(&second, "different")?;
        let mut app = Editor::default();
        app.open_file(&first);
        let preview = language_edit::Preview {
            title: "Rename",
            files: vec![
                language_edit::FileChange {
                    path: first,
                    before: "old".into(),
                    after: "new".into(),
                    snippets: vec![],
                    edits: None,
                },
                language_edit::FileChange {
                    path: second,
                    before: "old".into(),
                    after: "new".into(),
                    snippets: vec![],
                    edits: None,
                },
            ],
        };
        assert!(app.apply_language_edit(&preview).is_err());
        assert_eq!(app.tabs[0].text, "old");
        Ok(())
    }
    #[test]
    fn search_shortcut_routes_typing_to_search_and_keeps_the_document_intact() -> std::io::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let root = directory.path().canonicalize()?;
        let path = root.join("file.rs");
        std::fs::write(&path, "original")?;
        let context = egui::Context::default();
        let mut app = Editor {
            root: Some(root),
            ..Editor::default()
        };
        app.open_file(&path);
        let input = |events| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100.0, 760.0),
            )),
            events,
            ..Default::default()
        };
        let frame = |app: &mut Editor, events| {
            let _ = context.run(input(events), |context| {
                app.view_shortcuts(context);
                app.activity_rail(context);
                app.explorer(context);
                app.editor(context);
            });
        };
        frame(&mut app, vec![]);
        frame(
            &mut app,
            vec![egui::Event::Key {
                key: Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND | Modifiers::SHIFT,
            }],
        );
        frame(&mut app, vec![egui::Event::Text("needle".into())]);
        assert!(app.sidebar == Sidebar::Search);
        assert_eq!(app.workspace_query, "needle");
        assert_eq!(app.tabs[0].text, "original");
        Ok(())
    }

    #[test]
    fn function_key_breakpoints_use_the_unicode_caret_line() -> std::io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().canonicalize()?;
        let path = root.join("file.rs");
        std::fs::write(&path, "a🦀\nsecond\n")?;
        let context = egui::Context::default();
        let mut app = Editor {
            root: Some(root),
            ..Editor::default()
        };
        app.open_file(&path);
        set_caret(&context, editor_id(&path, false), 3);
        let _ = context.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: Key::F9,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                ..Default::default()
            },
            |context| app.view_shortcuts(context),
        );
        assert_eq!(app.debugger.breakpoints(&path), &[2]);
        assert_eq!(app.tabs[0].text, "a🦀\nsecond\n");
        Ok(())
    }

    #[test]
    fn explorer_indexes_and_displays_empty_directories() -> std::io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().canonicalize()?;
        let empty = root.join("empty");
        std::fs::create_dir(&empty)?;
        let listing = list_files(&root);
        assert!(listing.paths.is_empty());
        assert_eq!(listing.directories, vec![empty.clone()]);
        let rows = explorer_rows(
            &root,
            &listing.paths,
            &listing.directories,
            "",
            &Default::default(),
            None,
        );
        assert_eq!(rows.len(), 1);
        assert!(rows[0].folder);
        assert_eq!(rows[0].path, empty);
        Ok(())
    }
}
