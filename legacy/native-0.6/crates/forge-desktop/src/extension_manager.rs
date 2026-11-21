use crate::{
    extension_registry::{self as registry, Remote},
    packages::{self, Action, Bundle, Library, Prepared},
};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    io,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[derive(Clone, Copy, Default, PartialEq)]
enum Tab {
    #[default]
    Installed,
    Discover,
    Updates,
}

enum Outcome {
    Search(registry::Search),
    Details(Box<Remote>, String),
    Prepared(Box<Prepared>, bool),
    Updates(Vec<Remote>, Vec<String>),
    Profile(Vec<Bundle>),
}

struct Job {
    receiver: mpsc::Receiver<io::Result<Outcome>>,
    cancelled: Arc<AtomicBool>,
    label: String,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Preferences {
    check_updates: bool,
}

#[derive(Default)]
pub struct Manager {
    tab: Tab,
    query: String,
    installed_filter: String,
    results: Vec<Remote>,
    total: usize,
    offset: usize,
    details: Option<(Remote, String)>,
    selected_version: String,
    selected: Option<String>,
    prepared: Option<Prepared>,
    profile: Option<Vec<Bundle>>,
    updates: BTreeMap<String, Remote>,
    queue: VecDeque<Remote>,
    job: Option<Job>,
    preferences: Preferences,
    checked: bool,
}

impl Manager {
    pub(crate) fn has_update(&self, id: &str) -> bool {
        self.updates.contains_key(id)
    }

    pub fn load_preferences(&mut self) {
        if let Ok(directory) = packages::directory() {
            self.preferences = packages::read_bounded(&directory.join("preferences.state"), 4096)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
        }
    }

    fn start(
        &mut self,
        context: &egui::Context,
        label: &str,
        work: impl FnOnce(Arc<AtomicBool>) -> io::Result<Outcome> + Send + 'static,
    ) {
        if self.job.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        let context = context.clone();
        self.job = Some(Job {
            receiver,
            cancelled,
            label: label.into(),
        });
        std::thread::spawn(move || {
            let result = work(stop.clone());
            if !stop.load(Ordering::Relaxed) {
                let _ = sender.send(result);
                context.request_repaint();
            }
        });
    }

    fn search(&mut self, context: &egui::Context, offset: usize) {
        let query = self.query.trim().chars().take(200).collect::<String>();
        self.start(context, "Searching Open VSX…", move |_| {
            registry::search(&query, offset).map(Outcome::Search)
        });
    }

    fn details(&mut self, context: &egui::Context, id: String, version: Option<String>) {
        self.start(context, "Loading extension details…", move |_| {
            let remote = registry::details(&id, version.as_deref())?;
            let readme = remote
                .readme_url
                .as_ref()
                .and_then(|url| registry::fetch(url, 256 * 1024).ok())
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_default();
            Ok(Outcome::Details(Box::new(remote), readme))
        });
    }

    fn prepare_remote(&mut self, context: &egui::Context, remote: Remote, update: bool) {
        self.start(
            context,
            "Downloading and checking compatibility…",
            move |stop| {
                let bytes = registry::download(&remote)?;
                if stop.load(Ordering::Relaxed) {
                    return Err(io::Error::other("Download cancelled."));
                }
                let mut prepared = crate::extension_vsix::prepare(bytes)?;
                if prepared.bundle.manifest.id != remote.id
                    || prepared.bundle.manifest.version != remote.version
                {
                    return Err(io::Error::other(
                        "Downloaded package identifier or version does not match the catalog.",
                    ));
                }
                prepared.bundle.metadata.source = Some(remote.id);
                Ok(Outcome::Prepared(Box::new(prepared), update))
            },
        );
    }

    fn check_updates(&mut self, library: &Library, context: &egui::Context) {
        let installed: Vec<_> = library
            .installed
            .iter()
            .filter(|item| !item.pinned)
            .filter_map(|item| {
                item.metadata
                    .source
                    .as_ref()
                    .map(|id| (id.clone(), item.manifest.version.clone()))
            })
            .collect();
        self.checked = true;
        self.start(context, "Checking extension updates…", move |stop| {
            let mut updates = Vec::new();
            let mut errors = Vec::new();
            for (id, current) in installed {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let remote = match registry::details(&id, None) {
                    Ok(remote) => remote,
                    Err(error) => {
                        errors.push(format!("{id} — {error}"));
                        continue;
                    }
                };
                if semver::Version::parse(&remote.version).map_err(io::Error::other)?
                    > semver::Version::parse(&current).map_err(io::Error::other)?
                {
                    updates.push(remote);
                }
            }
            Ok(Outcome::Updates(updates, errors))
        });
    }

    pub fn poll(&mut self, library: &mut Library, context: &egui::Context) {
        if self.preferences.check_updates && !self.checked && self.job.is_none() {
            self.check_updates(library, context);
        }
        let result = self
            .job
            .as_ref()
            .and_then(|job| match job.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(io::Error::other(
                    "Extension operation ended unexpectedly.",
                ))),
                Err(mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result {
            self.job = None;
            match result {
                Ok(Outcome::Search(search)) => {
                    self.results = search.items;
                    self.total = search.total;
                    self.offset = search.offset;
                    self.tab = Tab::Discover;
                }
                Ok(Outcome::Details(remote, readme)) => {
                    self.selected_version = remote.version.clone();
                    self.details = Some((*remote, readme));
                }
                Ok(Outcome::Prepared(prepared, update)) => {
                    let prepared = *prepared;
                    if update && !prepared.bundle.metadata.javascript {
                        let id = prepared.bundle.manifest.id.clone();
                        match packages::directory()
                            .and_then(|directory| library.install_prepared_at(prepared, &directory))
                        {
                            Ok(()) => {
                                self.updates.remove(&id);
                                library.notice = format!("{id} updated.");
                            }
                            Err(error) => library.notice = error.to_string(),
                        }
                    } else {
                        self.prepared = Some(prepared);
                    }
                }
                Ok(Outcome::Updates(updates, errors)) => {
                    self.updates = updates
                        .into_iter()
                        .map(|item| (item.id.clone(), item))
                        .collect();
                    library.notice = format!(
                        "{} updates available. {}",
                        self.updates.len(),
                        errors.join("\n")
                    );
                }
                Ok(Outcome::Profile(bundles)) => self.profile = Some(bundles),
                Err(error) => library.notice = error.to_string(),
            }
        }
        if self.job.is_none() && self.prepared.is_none() && self.profile.is_none() {
            if let Some(remote) = self.queue.pop_front() {
                self.prepare_remote(context, remote, true);
            }
        }
        if self.job.is_some() {
            context.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    pub fn show(&mut self, library: &mut Library, ui: &mut egui::Ui) -> Option<Action> {
        self.poll(library, ui.ctx());
        let busy = self.job.is_some();
        ui.horizontal_wrapped(|ui| {
            for (tab, label) in [
                (Tab::Installed, "Installed"),
                (Tab::Discover, "Discover"),
                (Tab::Updates, "Updates"),
            ] {
                ui.selectable_value(&mut self.tab, tab, label);
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Install file…"))
                .on_hover_text("VSIX, .forge-extension or forge-extension.json")
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Extension packages", &["vsix", "forge-extension", "json"])
                    .pick_file()
                {
                    self.start(ui.ctx(), "Inspecting extension package…", move |_| {
                        let prepared = packages::prepare_file(&path)?;
                        Ok(Outcome::Prepared(Box::new(prepared), false))
                    });
                }
            }
            ui.menu_button("…", |ui| {
                if ui
                    .add_enabled(!busy, egui::Button::new("Reload installed"))
                    .clicked()
                {
                    if let Err(error) = library.reload() {
                        library.notice = error.to_string();
                    }
                    ui.close_menu();
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Export extension profile…"))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("forge-extensions.json")
                        .save_file()
                    {
                        let result = export_profile(&path, &library.installed);
                        library.notice = result.map_or_else(
                            |error| error.to_string(),
                            |()| "Extension profile exported.".into(),
                        );
                    }
                    ui.close_menu();
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Import extension profile…"))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Extension profile", &["json"])
                        .pick_file()
                    {
                        self.start(ui.ctx(), "Importing extension profile…", move |_| {
                            import_profile(&path).map(Outcome::Profile)
                        });
                    }
                    ui.close_menu();
                }
                ui.menu_button("Restore uninstalled", |ui| match library.removed() {
                    Ok(ids) if ids.is_empty() => {
                        ui.label("No removed extensions.");
                    }
                    Ok(ids) => {
                        for id in ids {
                            if ui.add_enabled(!busy, egui::Button::new(&id)).clicked() {
                                library.notice = library.restore_removed(&id).map_or_else(
                                    |error| error.to_string(),
                                    |()| format!("{id} restored."),
                                );
                                ui.close_menu();
                            }
                        }
                    }
                    Err(error) => {
                        ui.label(error.to_string());
                    }
                });
                if ui
                    .checkbox(
                        &mut self.preferences.check_updates,
                        "Check updates on startup",
                    )
                    .changed()
                {
                    match packages::directory().and_then(|directory| {
                        let bytes = serde_json::to_vec(&self.preferences)?;
                        crate::packages::atomic_write(&directory.join("preferences.state"), &bytes)
                    }) {
                        Ok(()) => {}
                        Err(error) => library.notice = error.to_string(),
                    }
                }
            });
        });
        if let Some(job) = &self.job {
            let label = job.label.clone();
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(label);
                if ui.button("Cancel").clicked() {
                    self.job = None;
                    self.queue.clear();
                }
            });
        }
        if !library.notice.is_empty() {
            ui.label(&library.notice);
        }
        if let Some(profile) = &self.profile {
            ui.separator();
            ui.strong(format!("Review profile — {} extensions", profile.len()));
            for bundle in profile {
                ui.label(format!(
                    "{} · {} · {} commands",
                    bundle.manifest.id,
                    bundle.manifest.version,
                    bundle.manifest.commands.len()
                ));
            }
            ui.label("This restores packaged contributions and enabled/pinned settings. Native commands run only when requested and receive the active document.");
            let mut install = false;
            let mut discard = false;
            ui.horizontal(|ui| {
                install = ui.button("Restore reviewed profile").clicked();
                discard = ui.button("Discard profile").clicked();
            });
            if install {
                let bundles = self.profile.take().expect("reviewed profile");
                let mut count = 0;
                let mut errors = Vec::new();
                for bundle in bundles {
                    let id = bundle.manifest.id.clone();
                    let enabled = bundle.enabled;
                    let pinned = bundle.pinned;
                    let result = packages::directory()
                        .and_then(|directory| {
                            library.install_prepared_at(Prepared { bundle }, &directory)
                        })
                        .and_then(|()| library.set_enabled(&id, enabled, None))
                        .and_then(|()| library.pin(&id, pinned));
                    match result {
                        Ok(()) => count += 1,
                        Err(error) => errors.push(format!("{id} — {error}")),
                    }
                }
                library.notice = format!("{count} extensions restored. {}", errors.join("\n"));
            } else if discard {
                self.profile = None;
            }
        }
        if let Some(prepared) = &self.prepared {
            let bundle = &prepared.bundle;
            ui.separator();
            ui.heading(&bundle.manifest.name);
            ui.label(format!(
                "{} · {}",
                bundle.manifest.id, bundle.manifest.version
            ));
            ui.label(format!(
                "{} themes · {} snippets · {} commands",
                bundle.manifest.themes.len(),
                bundle.manifest.snippets.len(),
                bundle.manifest.commands.len()
            ));
            if bundle.module.is_some() || !bundle.command_modules.is_empty() {
                ui.label("Permissions — active document text when you run a command. WebAssembly has no filesystem or network imports.");
            }
            for note in &bundle.metadata.notes {
                ui.label(note);
            }
            ui.label(format!("License — {}", bundle.metadata.license));
            let mut install = false;
            let mut dismiss = false;
            ui.horizontal(|ui| {
                install = ui.button("Install reviewed contributions").clicked();
                dismiss = ui.button("Discard").clicked();
            });
            if install {
                let prepared = self.prepared.take().expect("prepared package");
                let id = prepared.bundle.manifest.id.clone();
                match packages::directory()
                    .and_then(|directory| library.install_prepared_at(prepared, &directory))
                {
                    Ok(()) => {
                        library.notice = format!("{id} installed.");
                        self.updates.remove(&id);
                        self.selected = Some(id);
                        self.tab = Tab::Installed;
                    }
                    Err(error) => library.notice = error.to_string(),
                }
            } else if dismiss {
                self.prepared = None;
                self.queue.clear();
            }
            ui.separator();
        }

        let mut action = None;
        match self.tab {
            Tab::Discover => {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Search extensions or publisher.name")
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(!busy, egui::Button::new("Search Open VSX"))
                        .clicked()
                        || (!busy
                            && response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                    {
                        self.search(ui.ctx(), 0);
                    }
                    if ui
                        .add_enabled(!busy, egui::Button::new("Find by ID"))
                        .clicked()
                    {
                        let query = self.query.trim();
                        let (id, version) = query
                            .split_once('@')
                            .map_or((query, None), |(id, version)| {
                                (id, Some(version.to_owned()))
                            });
                        self.details(ui.ctx(), id.to_lowercase(), version);
                    }
                });
                ui.label(crate::chrome::small(
                    "Catalog — Open VSX. Compatibility is inspected before installation.",
                ));
                let mut inspect = None;
                for remote in &self.results {
                    ui.push_id(&remote.id, |ui| {
                        ui.separator();
                        if ui
                            .add_enabled(
                                !busy,
                                egui::Button::new(egui::RichText::new(&remote.name).strong()),
                            )
                            .clicked()
                        {
                            inspect = Some(remote.id.clone());
                        }
                        ui.label(crate::chrome::small(format!(
                            "{} · {} · {} downloads",
                            remote.id, remote.version, remote.downloads
                        )));
                        ui.label(&remote.description);
                    });
                }
                if let Some(id) = inspect {
                    self.details(ui.ctx(), id, None);
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!busy && self.offset > 0, egui::Button::new("Previous"))
                        .clicked()
                    {
                        self.search(ui.ctx(), self.offset.saturating_sub(30));
                    }
                    ui.label(format!(
                        "{} / {}",
                        self.offset + self.results.len(),
                        self.total
                    ));
                    if ui
                        .add_enabled(
                            !busy && self.offset + self.results.len() < self.total,
                            egui::Button::new("Next"),
                        )
                        .clicked()
                    {
                        self.search(ui.ctx(), self.offset + 30);
                    }
                });
                if let Some((remote, readme)) = self.details.clone() {
                    ui.separator();
                    ui.heading(&remote.name);
                    ui.label(format!("{} · {}", remote.id, remote.version));
                    ui.label(&remote.description);
                    ui.label(format!(
                        "Publisher — {}{}",
                        remote.publisher,
                        if remote.verified_namespace {
                            " · namespace verified by registry"
                        } else {
                            ""
                        }
                    ));
                    ui.label(format!("License — {}", remote.license));
                    ui.label(remote.categories.join(" · "));
                    egui::ComboBox::from_id_salt("extension-version")
                        .selected_text(&self.selected_version)
                        .show_ui(ui, |ui| {
                            for version in &remote.versions {
                                ui.selectable_value(
                                    &mut self.selected_version,
                                    version.clone(),
                                    version,
                                );
                            }
                        });
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(
                                !busy && self.selected_version != remote.version,
                                egui::Button::new("Load selected version"),
                            )
                            .clicked()
                        {
                            self.details(
                                ui.ctx(),
                                remote.id.clone(),
                                Some(self.selected_version.clone()),
                            );
                        }
                        if ui
                            .add_enabled(
                                !busy && self.selected_version == remote.version,
                                egui::Button::new("Inspect & install"),
                            )
                            .clicked()
                        {
                            self.prepare_remote(ui.ctx(), remote.clone(), false);
                        }
                    });
                    ui.collapsing("README", |ui| {
                        ui.add(egui::Label::new(readme).selectable(true));
                    });
                }
            }
            Tab::Installed => {
                ui.add(
                    egui::TextEdit::singleline(&mut self.installed_filter)
                        .hint_text("Filter installed extensions")
                        .desired_width(f32::INFINITY),
                );
                let filter = self.installed_filter.to_lowercase();
                let mut selection = None;
                for bundle in &library.installed {
                    if !format!("{} {}", bundle.manifest.name, bundle.manifest.id)
                        .to_lowercase()
                        .contains(&filter)
                    {
                        continue;
                    }
                    if ui
                        .selectable_label(
                            self.selected.as_ref() == Some(&bundle.manifest.id),
                            format!("{} · {}", bundle.manifest.name, bundle.manifest.version),
                        )
                        .clicked()
                    {
                        selection = Some(bundle.manifest.id.clone());
                    }
                    ui.label(crate::chrome::small(
                        if library.is_available_at(&bundle.manifest.id, library.workspace()) {
                            if bundle.pinned {
                                "Enabled · pinned version"
                            } else {
                                "Enabled"
                            }
                        } else {
                            "Disabled"
                        },
                    ));
                }
                if let Some(id) = selection {
                    self.selected = Some(id);
                }
                if let Some(bundle) = self
                    .selected
                    .as_ref()
                    .and_then(|id| {
                        library
                            .installed
                            .iter()
                            .find(|bundle| bundle.manifest.id == *id)
                    })
                    .cloned()
                {
                    ui.separator();
                    ui.heading(&bundle.manifest.name);
                    ui.label(&bundle.metadata.description);
                    ui.label(format!(
                        "{} · {} · {}",
                        bundle.manifest.id, bundle.manifest.version, bundle.metadata.license
                    ));
                    let mut enabled = bundle.enabled;
                    if ui
                        .add_enabled(!busy, egui::Checkbox::new(&mut enabled, "Enabled globally"))
                        .changed()
                    {
                        if let Err(error) = library.set_enabled(&bundle.manifest.id, enabled, None)
                        {
                            library.notice = error.to_string();
                        }
                    }
                    if let Some(root) = library.workspace().map(Path::to_owned) {
                        let mut enabled = !bundle.disabled_workspaces.contains(&root);
                        if ui
                            .add_enabled(
                                !busy,
                                egui::Checkbox::new(
                                    &mut enabled,
                                    "Enabled in this workspace folder",
                                ),
                            )
                            .changed()
                        {
                            if let Err(error) =
                                library.set_enabled(&bundle.manifest.id, enabled, Some(&root))
                            {
                                library.notice = error.to_string();
                            }
                        }
                    }
                    let mut pinned = bundle.pinned;
                    if ui
                        .add_enabled(!busy, egui::Checkbox::new(&mut pinned, "Pin this version"))
                        .changed()
                    {
                        if let Err(error) = library.pin(&bundle.manifest.id, pinned) {
                            library.notice = error.to_string();
                        }
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(!busy, egui::Button::new("Restore previous version"))
                            .clicked()
                        {
                            if let Err(error) = library.rollback(&bundle.manifest.id) {
                                library.notice = error.to_string();
                            }
                        }
                        if ui
                            .add_enabled(!busy, egui::Button::new("Uninstall"))
                            .clicked()
                        {
                            if let Err(error) = library.uninstall(&bundle.manifest.id) {
                                library.notice = error.to_string();
                            } else {
                                self.selected = None;
                            }
                        }
                    });
                    if let Some(update) = self.updates.get(&bundle.manifest.id).cloned() {
                        if ui
                            .add_enabled(
                                !busy && !bundle.pinned,
                                egui::Button::new(format!("Update to {}", update.version)),
                            )
                            .clicked()
                        {
                            self.prepare_remote(ui.ctx(), update, true);
                        }
                    }
                    for dependency in &bundle.manifest.dependencies {
                        if !library.is_enabled(dependency) {
                            ui.label(format!("Missing or disabled dependency — {dependency}"));
                        }
                    }
                    ui.add_enabled_ui(
                        library.is_available_at(&bundle.manifest.id, library.workspace()),
                        |ui| {
                            for theme in &bundle.manifest.themes {
                                if ui.button(format!("Apply {}", theme.name)).clicked() {
                                    action = Some(Action::Theme(theme.clone()));
                                }
                            }
                            for command in &bundle.manifest.commands {
                                if ui.button(&command.title).clicked() {
                                    action = Some(Action::RunCommand(format!(
                                        "{}/{}",
                                        bundle.manifest.id, command.id
                                    )));
                                }
                            }
                            if let Some(module) = &bundle.module {
                                if ui.button("Use transform").clicked() {
                                    action = Some(Action::Transform(
                                        bundle.manifest.id.clone(),
                                        module.clone(),
                                    ));
                                }
                            }
                        },
                    );
                    if !bundle.manifest.extension_pack.is_empty() {
                        ui.collapsing("Extension pack", |ui| {
                            for member in &bundle.manifest.extension_pack {
                                if ui
                                    .add_enabled(
                                        !busy,
                                        egui::Button::new(format!("Inspect {member}")),
                                    )
                                    .clicked()
                                {
                                    self.tab = Tab::Discover;
                                    self.details(ui.ctx(), member.clone(), None);
                                }
                            }
                        });
                    }
                    ui.label(format!(
                        "{} themes · {} snippets · {} commands",
                        bundle.manifest.themes.len(),
                        bundle.manifest.snippets.len(),
                        bundle.manifest.commands.len()
                    ));
                    for note in &bundle.metadata.notes {
                        ui.label(note);
                    }
                    ui.collapsing("README", |ui| {
                        ui.add(egui::Label::new(&bundle.metadata.readme).selectable(true));
                    });
                    ui.collapsing("Changelog", |ui| {
                        ui.add(egui::Label::new(&bundle.metadata.changelog).selectable(true));
                    });
                    ui.collapsing("Integrity", |ui| {
                        ui.label(format!("Source SHA-256 — {}", bundle.metadata.sha256));
                    });
                }
                if library.installed.is_empty() {
                    ui.label("Install a VSIX or local package, or search Open VSX.");
                }
            }
            Tab::Updates => {
                if ui
                    .add_enabled(!busy, egui::Button::new("Check updates"))
                    .clicked()
                {
                    self.check_updates(library, ui.ctx());
                }
                if ui
                    .add_enabled(
                        !busy && !self.updates.is_empty(),
                        egui::Button::new("Update all unpinned extensions"),
                    )
                    .clicked()
                {
                    self.queue = self
                        .updates
                        .values()
                        .filter(|remote| {
                            library
                                .installed
                                .iter()
                                .any(|bundle| bundle.manifest.id == remote.id && !bundle.pinned)
                        })
                        .cloned()
                        .collect();
                }
                let mut update = None;
                for remote in self.updates.values() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} → {}", remote.id, remote.version));
                        if ui
                            .add_enabled(
                                !busy
                                    && !library.installed.iter().any(|bundle| {
                                        bundle.manifest.id == remote.id && bundle.pinned
                                    }),
                                egui::Button::new("Update"),
                            )
                            .clicked()
                        {
                            update = Some(remote.clone());
                        }
                    });
                }
                if let Some(remote) = update {
                    self.prepare_remote(ui.ctx(), remote, true);
                }
                ui.label(crate::chrome::small("Updates install atomically per extension. The previous version remains available for rollback."));
            }
        }
        action
    }
}

#[derive(Serialize, Deserialize)]
struct Profile {
    schema_version: u32,
    extensions: Vec<Bundle>,
}

fn export_profile(path: &Path, bundles: &[Bundle]) -> io::Result<()> {
    let mut extensions = bundles.to_vec();
    for bundle in &mut extensions {
        bundle.disabled_workspaces.clear();
    }
    let bytes = serde_json::to_vec_pretty(&Profile {
        schema_version: 1,
        extensions,
    })?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(io::Error::other("Extension profile exceeds 64 MiB."));
    }
    packages::atomic_write(path, &bytes)
}

fn import_profile(path: &Path) -> io::Result<Vec<Bundle>> {
    let profile: Profile =
        serde_json::from_slice(&packages::read_bounded(path, 64 * 1024 * 1024)?)?;
    if profile.schema_version != 1 || profile.extensions.len() > 100 {
        return Err(io::Error::other(
            "Unsupported extension profile version or too many entries.",
        ));
    }
    let mut identifiers = std::collections::HashSet::new();
    for bundle in &profile.extensions {
        packages::validate_bundle(bundle)?;
        if !identifiers.insert(&bundle.manifest.id) {
            return Err(io::Error::other(
                "Profile contains duplicate extension identifiers.",
            ));
        }
    }
    let mut extensions = profile.extensions;
    for bundle in &mut extensions {
        bundle.disabled_workspaces.clear();
    }
    Ok(extensions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bundle() -> Bundle {
        serde_json::from_value(json!({
            "manifest":{"schema_version":1,"id":"studio.profile","name":"Profile","version":"1.0.0"},
            "enabled":false,"module":null,"pinned":true,"disabled_workspaces":["/private/project"]
        })).unwrap()
    }

    #[test]
    fn profile_round_trip_preserves_versions_settings_and_removes_workspace_paths() -> io::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("profile.json");
        export_profile(&path, &[bundle()])?;
        let restored = import_profile(&path)?;
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].manifest.version, "1.0.0");
        assert!(!restored[0].enabled);
        assert!(restored[0].pinned);
        assert!(restored[0].disabled_workspaces.is_empty());
        Ok(())
    }

    #[test]
    fn profile_rejects_duplicates_unknown_schema_and_unsafe_modules() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("profile.json");
        export_profile(&path, &[bundle(), bundle()])?;
        assert!(import_profile(&path).is_err());
        let profile = json!({"schema_version":2,"extensions":[]});
        std::fs::write(&path, serde_json::to_vec(&profile)?)?;
        assert!(import_profile(&path).is_err());
        let mut item = bundle();
        item.manifest.transform = Some("invalid.wasm".into());
        item.module = Some(vec![1, 2, 3]);
        export_profile(&path, &[item])?;
        assert!(import_profile(&path).is_err());
        Ok(())
    }

    #[test]
    fn cancelled_preparation_never_changes_the_library() {
        let context = egui::Context::default();
        let mut manager = Manager::default();
        manager.start(&context, "test", |stop| {
            while !stop.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            Ok(Outcome::Profile(vec![bundle()]))
        });
        manager.job = None;
        let mut library = Library::default();
        for _ in 0..10 {
            manager.poll(&mut library, &context);
        }
        assert!(library.installed.is_empty() && manager.profile.is_none());
    }
}
