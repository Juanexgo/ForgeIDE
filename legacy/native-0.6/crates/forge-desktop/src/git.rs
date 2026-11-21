use eframe::egui::{self, ScrollArea, TextEdit};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const LIMIT: u64 = 512 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Change {
    index: u8,
    worktree: u8,
    path: String,
}

#[derive(Clone)]
enum Action {
    CheckedBranch {
        branch: String,
        action: Box<Action>,
    },
    Init,
    Clone {
        url: String,
        destination: PathBuf,
    },
    PublishRepository {
        name: String,
        private: bool,
        branch: String,
        head: String,
    },
    SetIdentity {
        name: String,
        email: String,
    },
    Status,
    StageAll(Vec<String>),
    UnstageAll,
    ReviewDiscard(String),
    Discard(DiscardReview),
    AddRemote {
        name: String,
        url: String,
    },
    Fetch(String),
    Pull {
        remote: String,
        branch: String,
        head: String,
    },
    Push {
        remote: String,
        branch: String,
        head: String,
        publish: bool,
    },
    Sync {
        remote: String,
        branch: String,
        head: String,
    },
    StashSave(String),
    Stashes,
    StashApply(String),
    RenameBranch {
        old: String,
        new: String,
    },
    DeleteBranch(String),
    Branches,
    Switch(String),
    CreateBranch(String),
    History,
    CommitDiff(String),
    Compare {
        base: String,
        head: String,
    },
    StageLines {
        path: String,
        expected: String,
        hunk: usize,
        lines: BTreeSet<usize>,
    },
    Worktrees,
    AddWorktree {
        path: PathBuf,
        branch: String,
    },
    StageHunk {
        path: String,
        expected: String,
        patch: String,
    },
    Diff {
        path: String,
        staged: bool,
    },
    Stage(String),
    Unstage(String),
    Commit {
        message: String,
        staged: Vec<u8>,
    },
}

enum Outcome {
    Status(Vec<Change>, usize, Vec<u8>, Box<Repository>),
    Diff(String),
    Info(String),
    Branches(Vec<String>),
    Stashes(Vec<Stash>),
    History(Vec<Commit>),
    Discard(DiscardReview),
    Cloned(PathBuf),
}

#[derive(Clone, Debug)]
struct DiscardReview {
    path: String,
    before: Option<String>,
    index: Vec<u8>,
    head: String,
}

#[derive(Clone, Default, Debug)]
struct Repository {
    root: PathBuf,
    branch: String,
    head: String,
    upstream: String,
    ahead: usize,
    behind: usize,
    remotes: Vec<Remote>,
    author_name: String,
    author_email: String,
}

#[derive(Clone, Debug)]
struct Remote {
    name: String,
    url: String,
    push_urls: Vec<String>,
}

#[derive(Clone, Debug)]
struct Commit {
    oid: String,
    author: String,
    subject: String,
}

#[derive(Clone, Debug)]
struct Stash {
    reference: String,
    oid: String,
    subject: String,
}

thread_local! {
    static CANCELLATION: std::cell::RefCell<Option<Arc<AtomicBool>>> = const { std::cell::RefCell::new(None) };
}
struct GitJob {
    receiver: mpsc::Receiver<io::Result<Outcome>>,
    cancel: Arc<AtomicBool>,
}
impl Drop for GitJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub struct Panel {
    enabled: bool,
    process: Option<GitJob>,
    changes: Vec<Change>,
    selected: Option<String>,
    diff: Option<String>,
    message: String,
    notice: String,
    staged: Vec<u8>,
    skipped_paths: usize,
    diff_path: Option<String>,
    branches: Vec<String>,
    branch_name: String,
    revision: String,
    base_revision: String,
    selected_lines: BTreeMap<usize, BTreeSet<usize>>,
    info: String,
    repository: Repository,
    last_refresh: Option<Instant>,
    selected_remote: String,
    remote_branch: String,
    remote_name: String,
    remote_url: String,
    stashes: Vec<Stash>,
    stash_message: String,
    pending_remote: Option<Action>,
    open_path: Option<PathBuf>,
    committing: bool,
    commits: Vec<Commit>,
    unified: bool,
    discard: Option<DiscardReview>,
    clone_url: String,
    clone_name: String,
    cloned: Option<PathBuf>,
    publish_name: String,
    publish_public: bool,
    author_name: String,
    author_email: String,
    failed: bool,
}

impl Panel {
    pub fn tick(&mut self, context: &egui::Context) {
        self.update();
        if self.process.is_some() {
            context.request_repaint_after(Duration::from_millis(120));
        }
    }

    pub fn changed_count(&self) -> usize {
        self.changes.len()
    }
    pub fn branch_summary(&self) -> Option<String> {
        (!self.repository.branch.is_empty()).then(|| {
            format!(
                "{}  ↑{} ↓{}",
                self.repository.branch, self.repository.ahead, self.repository.behind
            )
        })
    }

    pub fn take_cloned(&mut self) -> Option<PathBuf> {
        self.cloned.take()
    }

    pub fn take_open(&mut self) -> Option<PathBuf> {
        self.open_path.take()
    }

    pub fn show_without_folder(&mut self, ui: &mut egui::Ui) {
        self.tick(ui.ctx());
        if !self.notice.is_empty() {
            ui.label(&self.notice);
        }
        if self.process.is_some() {
            ui.spinner();
        }
        self.clone_controls(ui);
    }

    fn clone_controls(&mut self, ui: &mut egui::Ui) {
        ui.label("Clone and add a repository to this workspace");
        ui.add(TextEdit::singleline(&mut self.clone_url).hint_text("HTTPS or SSH clone URL"));
        ui.add(TextEdit::singleline(&mut self.clone_name).hint_text("New folder name"));
        if ui
            .add_enabled(
                self.process.is_none()
                    && !self.clone_url.trim().is_empty()
                    && !self.clone_name.trim().is_empty(),
                egui::Button::new("Choose parent folder & clone…"),
            )
            .clicked()
        {
            let name = self.clone_name.trim();
            if name == "."
                || name == ".."
                || name.contains(['/', '\\'])
                || name.chars().any(char::is_control)
            {
                self.notice = "Choose a simple new folder name.".into();
            } else if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                let destination = parent.join(name);
                self.run(
                    ui.ctx(),
                    &parent,
                    Action::Clone {
                        url: self.clone_url.trim().into(),
                        destination,
                    },
                );
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, root: &Path) {
        self.tick(ui.ctx());
        if !self.enabled {
            self.enabled = true;
            self.run(ui.ctx(), root, Action::Status);
        }
        self.diff_window(ui.ctx(), root);
        if self.process.is_none()
            && !self.failed
            && self.pending_remote.is_none()
            && self
                .last_refresh
                .is_some_and(|time| time.elapsed() > Duration::from_secs(4))
        {
            self.run(ui.ctx(), root, Action::Status);
        }
        let busy = self.process.is_some();
        ScrollArea::vertical().id_salt("source-control-content").show(ui,|ui| {
            ui.horizontal(|ui| {
                ui.strong(if self.repository.branch.is_empty() { "Source Control" } else { &self.repository.branch });
                if busy { ui.spinner(); }
                if ui.add_enabled(!busy,egui::Button::new("↻")).on_hover_text("Refresh saved files").clicked() { self.run(ui.ctx(),root,Action::Status); }
            });
            if !self.repository.root.as_os_str().is_empty() { ui.label(crate::chrome::small(self.repository.root.display().to_string())); }
            if !self.repository.upstream.is_empty() {
                ui.label(crate::chrome::small(format!("{} · ↑ {}  ↓ {}",self.repository.upstream,self.repository.ahead,self.repository.behind)));
            }
            if let Some(job) = &self.process {
                if ui.button(if job.cancel.load(Ordering::Relaxed) { "Cancelling…" } else { "Cancel Git operation" }).clicked() { job.cancel.store(true,Ordering::Relaxed); }
            }
            if !self.notice.is_empty() { ui.label(&self.notice); }
            if self.repository.root.as_os_str().is_empty() {
                ui.label("Open an existing Git repository or initialize this folder.");
                if ui.add_enabled(!busy,egui::Button::new("Initialize repository")).clicked() { self.run(ui.ctx(),root,Action::Init); }
                self.clone_controls(ui);
                return;
            }
            ui.add(TextEdit::multiline(&mut self.message).hint_text("Commit message").desired_rows(3).desired_width(f32::INFINITY));
            let staged_count = self.changes.iter().filter(|change| is_staged(change) && !is_conflict(change)).count();
            let conflicts = self.changes.iter().filter(|change| is_conflict(change)).count();
            if ui.add_enabled(!busy && staged_count > 0 && conflicts == 0 && self.skipped_paths == 0 && !self.staged.is_empty() && !self.message.trim().is_empty(),
                egui::Button::new(format!("✓ Commit staged ({staged_count})")).min_size(egui::vec2(ui.available_width(),30.0))).clicked() {
                self.run(ui.ctx(),root,Action::Commit {message:self.message.trim().into(),staged:self.staged.clone()});
            }
            ui.label(crate::chrome::small("Commits include saved, staged content. Unsaved editor changes stay in the editor."));
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!busy && !self.changes.is_empty() && conflicts == 0,egui::Button::new("Stage all")).clicked() {
                    self.run(ui.ctx(),root,Action::StageAll(self.changes.iter().filter(|change| change.worktree != b' ').map(|change| change.path.clone()).collect()));
                }
                if ui.add_enabled(!busy && staged_count > 0,egui::Button::new("Unstage all")).clicked() { self.run(ui.ctx(),root,Action::UnstageAll); }
            });
            let mut action = None;
            for (kind,label) in [(0,"Merge changes"),(1,"Staged changes"),(2,"Changes")] {
                let changes: Vec<_> = self.changes.iter().filter(|change| match kind {
                    0 => is_conflict(change),
                    1 => is_staged(change) && !is_conflict(change),
                    _ => change.worktree != b' ' && !is_conflict(change),
                }).cloned().collect();
                if changes.is_empty() && kind == 0 { continue; }
                egui::CollapsingHeader::new(format!("{label} ({})",changes.len())).id_salt(("git-group",kind)).default_open(true).show(ui,|ui| {
                    if changes.is_empty() { ui.label(crate::chrome::small("No changes")); }
                    for change in changes {
                        ui.push_id((kind,&change.path),|ui| {
                            ui.horizontal(|ui| {
                                let status = if kind == 1 { change.index } else { change.worktree };
                                ui.colored_label(if kind == 0 || status == b'D' { crate::chrome::theme_red() } else { crate::chrome::theme_green() },format!("{}",char::from(status)));
                                let display = change.path.escape_debug().to_string();
                                if ui.selectable_label(self.selected.as_ref() == Some(&change.path),&display).on_hover_text(&display).clicked() {
                                    self.selected = Some(change.path.clone());
                                    action = Some(Action::Diff {path:change.path.clone(),staged:kind == 1});
                                }
                            });
                            ui.horizontal_wrapped(|ui| {
                                if ui.small_button("Open").clicked() { self.open_path = Some(self.repository.root.join(&change.path)); }
                                if kind == 1 {
                                    if ui.add_enabled(!busy,egui::Button::new("− Unstage").small()).clicked() { action = Some(Action::Unstage(change.path.clone())); }
                                } else if kind != 0 && ui.add_enabled(!busy,egui::Button::new("+ Stage").small()).clicked() {
                                    action = Some(Action::Stage(change.path.clone()));
                                }
                                if kind == 2 && change.worktree != b'?' && ui.add_enabled(!busy,egui::Button::new("Discard saved…").small()).clicked() {
                                    action = Some(Action::ReviewDiscard(change.path.clone()));
                                }
                                if kind == 0 { ui.label(crate::chrome::small("Resolve in editor, save, then stage.")); }
                                if kind == 0 && ui.add_enabled(!busy,egui::Button::new("Stage resolution").small()).clicked() { action = Some(Action::Stage(change.path.clone())); }
                            });
                        });
                    }
                });
            }
            if let Some(action) = action { self.run(ui.ctx(),root,action); }
            ui.separator();
            ui.collapsing("Commit author",|ui| {
                ui.label(crate::chrome::small(format!("Current: {} <{}>",self.repository.author_name,self.repository.author_email)));
                ui.add(TextEdit::singleline(&mut self.author_name).hint_text("Author name"));
                ui.add(TextEdit::singleline(&mut self.author_email).hint_text("Author email"));
                if ui.add_enabled(!busy && !self.author_name.trim().is_empty() && !self.author_email.trim().is_empty(),egui::Button::new("Save for this repository")).clicked() {
                    self.run(ui.ctx(),root,Action::SetIdentity {name:self.author_name.trim().into(),email:self.author_email.trim().into()});
                }
            });
            ui.collapsing("Clone repository",|ui| self.clone_controls(ui));
            ui.collapsing("Remotes & synchronization",|ui| {
                egui::ComboBox::from_id_salt("git-remote").selected_text(if self.selected_remote.is_empty() { "Choose remote" } else { &self.selected_remote }).show_ui(ui,|ui| {
                    for remote in &self.repository.remotes { ui.selectable_value(&mut self.selected_remote,remote.name.clone(),&remote.name); }
                });
                if let Some(remote) = self.repository.remotes.iter().find(|remote| remote.name == self.selected_remote) {
                    ui.label(crate::chrome::small(format!("Fetch — {}",remote.url)));
                    for url in &remote.push_urls { ui.label(crate::chrome::small(format!("Push — {url}"))); }
                }
                ui.add(TextEdit::singleline(&mut self.remote_branch).hint_text("Remote branch").desired_width(f32::INFINITY));
                let available = !busy && !self.selected_remote.is_empty();
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(available,egui::Button::new("Fetch")).clicked() { self.run(ui.ctx(),root,Action::Fetch(self.selected_remote.clone())); }
                    if ui.add_enabled(available && !self.repository.head.is_empty(),egui::Button::new("Pull (fast-forward)")).clicked() {
                        self.pending_remote = Some(Action::Pull {remote:self.selected_remote.clone(),branch:self.remote_branch.clone(),head:self.repository.head.clone()});
                    }
                    if ui.add_enabled(available && !self.repository.head.is_empty(),egui::Button::new(if self.repository.upstream.is_empty() { "Publish branch…" } else { "Push…" })).clicked() {
                        self.pending_remote = Some(Action::Push {remote:self.selected_remote.clone(),branch:self.remote_branch.clone(),head:self.repository.head.clone(),publish:self.repository.upstream.is_empty()});
                    }
                    if ui.add_enabled(available && !self.repository.head.is_empty(),egui::Button::new("Sync…")).clicked() {
                        self.pending_remote = Some(Action::Sync {remote:self.selected_remote.clone(),branch:self.remote_branch.clone(),head:self.repository.head.clone()});
                    }
                });
                ui.collapsing("Add remote",|ui| {
                    ui.add(TextEdit::singleline(&mut self.remote_name).hint_text("Name, e.g. origin"));
                    ui.add(TextEdit::singleline(&mut self.remote_url).hint_text("HTTPS or SSH repository URL"));
                    if ui.add_enabled(!busy && !self.remote_name.is_empty() && !self.remote_url.is_empty(),egui::Button::new("Add remote")).clicked() {
                        self.run(ui.ctx(),root,Action::AddRemote {name:self.remote_name.trim().into(),url:self.remote_url.trim().into()});
                    }
                });
                if self.repository.remotes.is_empty() {
                    ui.collapsing("Publish new GitHub repository",|ui| {
                        ui.add(TextEdit::singleline(&mut self.publish_name).hint_text("owner/repository"));
                        ui.checkbox(&mut self.publish_public,"Make repository public");
                        ui.label(if self.publish_public { "Public: uploaded code will be visible to anyone." } else { "Private: only GitHub users with access can read uploaded code." });
                        if ui.add_enabled(!busy && !self.publish_name.trim().is_empty() && !self.repository.head.is_empty(),egui::Button::new("Review publication…")).clicked() {
                            self.pending_remote = Some(Action::PublishRepository {name:self.publish_name.trim().into(),private:!self.publish_public,branch:self.repository.branch.clone(),head:self.repository.head.clone()});
                        }
                    });
                }
                ui.label(crate::chrome::small("GitHub HTTPS uses the connected GitHub CLI account. SSH uses your configured SSH agent. Sync pulls fast-forward, then pushes; no forced push."));
            });
            ui.collapsing("Branches & worktrees",|ui| {
                if ui.add_enabled(!busy,egui::Button::new("Refresh branches")).clicked() { self.run(ui.ctx(),root,Action::Branches); }
                for branch in self.branches.clone() {
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(!busy && branch != self.repository.branch,egui::Button::new(&branch)).clicked() { self.run(ui.ctx(),root,Action::Switch(branch.clone())); }
                        if ui.add_enabled(!busy && branch != self.repository.branch,egui::Button::new("Delete merged").small()).clicked() {
                            self.pending_remote = Some(Action::DeleteBranch(branch));
                        }
                    });
                }
                ui.add(TextEdit::singleline(&mut self.branch_name).hint_text("New branch name"));
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!busy && !self.branch_name.trim().is_empty(),egui::Button::new("Create & switch")).clicked() { self.run(ui.ctx(),root,Action::CreateBranch(self.branch_name.trim().into())); }
                    if ui.add_enabled(!busy && !self.branch_name.trim().is_empty() && !self.repository.head.is_empty(),egui::Button::new("Rename current")).clicked() {
                        self.pending_remote = Some(Action::RenameBranch {old:self.repository.branch.clone(),new:self.branch_name.trim().into()});
                    }
                });
                if ui.add_enabled(!busy,egui::Button::new("List worktrees")).clicked() { self.run(ui.ctx(),root,Action::Worktrees); }
                if ui.add_enabled(!busy && !self.branch_name.trim().is_empty(),egui::Button::new("Create worktree with new branch…")).clicked() {
                    if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                        self.run(ui.ctx(),root,Action::AddWorktree {path:parent.join(self.branch_name.replace('/',"-")),branch:self.branch_name.trim().into()});
                    }
                }
            });
            ui.collapsing("History & compare",|ui| {
                if ui.add_enabled(!busy,egui::Button::new("Refresh history")).clicked() { self.run(ui.ctx(),root,Action::History); }
                for commit in self.commits.clone() {
                    if ui.add_enabled(!busy,egui::Button::new(format!("{}  {}",&commit.oid[..8],commit.subject))).on_hover_text(format!("{} · {}",commit.author,commit.oid)).clicked() {
                        self.diff_path = None;
                        self.revision = commit.oid.clone();
                        self.run(ui.ctx(),root,Action::CommitDiff(commit.oid));
                    }
                }
                ui.add(TextEdit::singleline(&mut self.base_revision).hint_text("Base commit hash"));
                ui.add(TextEdit::singleline(&mut self.revision).hint_text("Target commit hash"));
                if ui.add_enabled(!busy,egui::Button::new("Compare commits")).clicked() {
                    self.diff_path = None;
                    self.run(ui.ctx(),root,Action::Compare {base:self.base_revision.trim().into(),head:self.revision.trim().into()});
                }
            });
            ui.collapsing("Stashes",|ui| {
                ui.add(TextEdit::singleline(&mut self.stash_message).hint_text("Describe changes to stash"));
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!busy && !self.changes.is_empty() && !self.stash_message.trim().is_empty() && conflicts == 0,egui::Button::new("Stash saved changes")).clicked() {
                        self.run(ui.ctx(),root,Action::StashSave(self.stash_message.trim().into()));
                    }
                    if ui.add_enabled(!busy,egui::Button::new("Refresh stashes")).clicked() { self.run(ui.ctx(),root,Action::Stashes); }
                });
                for stash in self.stashes.clone() {
                    ui.label(format!("{} · {}",stash.reference,stash.subject));
                    if ui.add_enabled(!busy,egui::Button::new("Apply, keep stash")).clicked() { self.run(ui.ctx(),root,Action::StashApply(stash.oid)); }
                }
                ui.label(crate::chrome::small("Includes saved tracked/untracked files. Applying keeps the stash for recovery."));
            });
            if let Some(review) = self.discard.clone() {
                egui::Frame::group(ui.style()).show(ui,|ui| {
                    ui.strong(format!("Discard saved changes in {}?",review.path));
                    ui.label("The working file will match the staged version. Its current saved text is archived in Local history first. Unsaved editor buffers stay open.");
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!busy,egui::Button::new("Archive and discard")).clicked() { self.discard = None; self.run(ui.ctx(),root,Action::Discard(review)); }
                        if ui.button("Keep saved changes").clicked() { self.discard = None; }
                    });
                });
            }
            if let Some(pending) = self.pending_remote.clone() {
                egui::Frame::group(ui.style()).show(ui,|ui| {
                    ui.strong("Review Git operation");
                    match &pending {
                        Action::Push {remote,branch,..} | Action::Pull {remote,branch,..} | Action::Sync {remote,branch,..} => {
                            let verb = match &pending { Action::Push {..} => "Push",Action::Pull {..} => "Pull fast-forward",_ => "Pull fast-forward, then push" };
                            ui.label(format!("{verb}: {} → {remote}/{branch}",self.repository.branch));
                            if let Some(remote) = self.repository.remotes.iter().find(|item| &item.name == remote) {
                                ui.label(format!("Fetch — {}",remote.url));
                                for url in &remote.push_urls { ui.label(format!("Push — {url}")); }
                            }
                            ui.label(format!("Local upstream comparison: ↑ {}  ↓ {}",self.repository.ahead,self.repository.behind));
                        }
                        Action::PublishRepository {name,private,branch,..} => {
                            ui.label(format!("Create {} GitHub repository {name}, add origin, and upload branch {branch}.",if *private { "private" } else { "public" }));
                            ui.label("Publication uses the active GitHub CLI account. If uploading fails after creation, the repository and origin remain available for retry.");
                        }
                        Action::DeleteBranch(branch) => { ui.label(format!("Delete merged local branch {branch}. Git refuses unmerged branches.")); }
                        Action::RenameBranch {old,new} => { ui.label(format!("Rename local branch {old} → {new}")); }
                        _ => {}
                    }
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!busy,egui::Button::new("Continue")).clicked() { self.pending_remote = None; self.run(ui.ctx(),root,pending); }
                        if ui.button("Cancel").clicked() { self.pending_remote = None; }
                    });
                });
            }
            if !self.info.is_empty() {
                ui.collapsing("Git output",|ui| { ui.add(egui::Label::new(egui::RichText::new(&self.info).monospace()).selectable(true)); });
            }
        });
    }

    fn diff_window(&mut self, context: &egui::Context, root: &Path) {
        let Some(diff) = self.diff.clone() else {
            return;
        };
        let mut open = true;
        let mut action = None;
        egui::Window::new("Git changes")
            .id(egui::Id::new("git-diff-window"))
            .default_size([920.0, 620.0])
            .open(&mut open)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.unified, false, "Side by side");
                    ui.selectable_value(&mut self.unified, true, "Unified");
                    if let Some(path) = &self.selected {
                        ui.label(path);
                    }
                });
                ScrollArea::both()
                    .id_salt("git-diff-body")
                    .max_height(520.0)
                    .show(ui, |ui| {
                        if self.unified {
                            colored_diff(ui, &diff);
                        } else {
                            side_by_side(ui, &diff);
                        }
                        if let Some(path) = &self.diff_path {
                            egui::CollapsingHeader::new("Stage individual hunks or lines")
                                .id_salt("git-partial")
                                .show(ui, |ui| {
                                    for (index, patch) in split_hunks(&diff).iter().enumerate() {
                                        if ui
                                            .add_enabled(
                                                self.process.is_none(),
                                                egui::Button::new(format!(
                                                    "Stage hunk {}",
                                                    index + 1
                                                )),
                                            )
                                            .clicked()
                                        {
                                            action = Some(Action::StageHunk {
                                                path: path.clone(),
                                                expected: diff.clone(),
                                                patch: patch.clone(),
                                            });
                                        }
                                        let body = patch
                                            .find("@@ ")
                                            .map_or(patch.as_str(), |start| &patch[start..]);
                                        let selected =
                                            self.selected_lines.entry(index).or_default();
                                        for (line, text) in body.lines().enumerate() {
                                            if text.starts_with(['+', '-']) {
                                                let mut checked = selected.contains(&line);
                                                if ui
                                                    .checkbox(
                                                        &mut checked,
                                                        egui::RichText::new(text).monospace(),
                                                    )
                                                    .changed()
                                                {
                                                    if checked {
                                                        selected.insert(line);
                                                    } else {
                                                        selected.remove(&line);
                                                    }
                                                }
                                            }
                                        }
                                        if ui
                                            .add_enabled(
                                                self.process.is_none() && !selected.is_empty(),
                                                egui::Button::new("Stage selected lines"),
                                            )
                                            .clicked()
                                        {
                                            action = Some(Action::StageLines {
                                                path: path.clone(),
                                                expected: diff.clone(),
                                                hunk: index,
                                                lines: selected.clone(),
                                            });
                                        }
                                    }
                                });
                        }
                    });
            });
        if !open {
            self.diff = None;
        }
        if let Some(action) = action {
            self.diff = None;
            self.run(context, root, action);
        }
    }

    fn run(&mut self, context: &egui::Context, root: &Path, action: Action) {
        if self.process.is_some() {
            return;
        }
        self.pending_remote = None;
        let action = if matches!(
            &action,
            Action::Pull { .. } | Action::Push { .. } | Action::Sync { .. } | Action::Commit { .. }
        ) {
            Action::CheckedBranch {
                branch: self.repository.branch.clone(),
                action: Box::new(action),
            }
        } else {
            action
        };
        let (send, receive) = mpsc::sync_channel(1);
        let root = root.to_path_buf();
        let context = context.clone();
        if matches!(&action, Action::CommitDiff(_) | Action::Compare { .. }) {
            self.selected = None;
        }
        if let Action::Diff { path, staged } = &action {
            self.diff_path = (!staged).then(|| path.clone());
        }
        self.committing = matches!(&action, Action::Commit { .. });
        self.last_refresh = Some(Instant::now());
        let cancel = Arc::new(AtomicBool::new(false));
        self.process = Some(GitJob {
            receiver: receive,
            cancel: cancel.clone(),
        });
        self.notice.clear();
        self.failed = false;
        std::thread::spawn(move || {
            CANCELLATION.with(|flag| *flag.borrow_mut() = Some(cancel));
            let _ = send.send(execute(&root, action));
            context.request_repaint();
        });
    }

    fn update(&mut self) {
        let Some(process) = self.process.as_ref() else {
            return;
        };
        let result = match process.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(io::Error::other("Git worker ended unexpectedly."))
            }
        };
        self.process = None;
        match result {
            Ok(Outcome::Status(changes, skipped, staged, repository)) => {
                let changed_branch = self.repository.branch != repository.branch;
                self.repository = *repository;
                if self.author_name.is_empty() {
                    self.author_name = self.repository.author_name.clone();
                }
                if self.author_email.is_empty() {
                    self.author_email = self.repository.author_email.clone();
                }
                if changed_branch {
                    self.remote_branch = self.repository.upstream.split_once('/').map_or_else(
                        || self.repository.branch.clone(),
                        |(_, branch)| branch.into(),
                    );
                    self.selected_remote = self
                        .repository
                        .upstream
                        .split_once('/')
                        .map(|(remote, _)| remote.to_owned())
                        .unwrap_or_default();
                }
                if self.selected_remote.is_empty() {
                    self.selected_remote = self
                        .repository
                        .remotes
                        .iter()
                        .find(|item| item.name == "origin")
                        .or(self.repository.remotes.first())
                        .map(|item| item.name.clone())
                        .unwrap_or_default();
                }
                if self.remote_branch.is_empty() {
                    self.remote_branch = self.repository.branch.clone();
                }
                if self.committing {
                    self.message.clear();
                }
                self.staged = staged;
                self.skipped_paths = skipped;
                self.changes = changes;
                self.notice = if skipped == 0 {
                    format!("{} changed files", self.changes.len())
                } else {
                    format!(
                        "{} changed files. {skipped} paths could not be displayed.",
                        self.changes.len()
                    )
                };
            }
            Ok(Outcome::Diff(diff)) => {
                self.selected_lines.clear();
                self.diff = Some(if diff.is_empty() {
                    "No diff".to_owned()
                } else {
                    diff
                });
            }
            Ok(Outcome::Info(info)) => self.info = info,
            Ok(Outcome::Branches(branches)) => self.branches = branches,
            Ok(Outcome::Stashes(stashes)) => self.stashes = stashes,
            Ok(Outcome::History(commits)) => self.commits = commits,
            Ok(Outcome::Discard(review)) => self.discard = Some(review),
            Ok(Outcome::Cloned(path)) => {
                self.cloned = Some(path);
                self.notice = "Repository cloned.".into();
            }
            Err(error) => {
                self.failed = true;
                self.info = error.to_string();
                self.staged.clear();
                self.notice = error.to_string();
            }
        }
    }
}

fn execute(root: &Path, action: Action) -> io::Result<Outcome> {
    let git = locate_git(root)?;
    if let Action::Clone { url, destination } = &action {
        validate_remote_url(url)?;
        if destination.exists() {
            return Err(io::Error::other("The clone destination already exists."));
        }
        let destination_text = destination
            .to_str()
            .ok_or_else(|| io::Error::other("Clone paths must be Unicode."))?;
        call_network(
            &git,
            root,
            &["clone", "--template=", "--", url, destination_text],
        )?;
        return Ok(Outcome::Cloned(destination.canonicalize()?));
    }
    if matches!(&action, Action::Init) {
        call(&git, root, &["init", "--template="])?;
    }
    let top = call(&git, root, &["rev-parse", "--show-toplevel"])?;
    let top = std::str::from_utf8(&top)
        .map_err(io::Error::other)?
        .trim_end_matches(['\r', '\n']);
    let repository_root = PathBuf::from(top).canonicalize()?;
    let root = repository_root.as_path();
    let action = if let Action::CheckedBranch { branch, action } = action {
        let current = call(&git, root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        if String::from_utf8_lossy(&current).trim() != branch {
            return Err(io::Error::other(
                "The active branch changed. Refresh and review the operation again.",
            ));
        }
        *action
    } else {
        action
    };

    match action {
        Action::CheckedBranch { .. } => {
            return Err(io::Error::other("Nested checked operations are invalid."));
        }

        Action::Init => {}
        Action::Clone { .. } => unreachable!("clone handled before repository discovery"),
        Action::SetIdentity { name, email } => {
            if name.is_empty()
                || name.len() > 200
                || name.chars().any(char::is_control)
                || email.len() > 254
                || !email.contains('@')
                || email
                    .chars()
                    .any(|character| character.is_whitespace() || character.is_control())
            {
                return Err(io::Error::other(
                    "Enter an author name and a valid email address.",
                ));
            }
            call(&git, root, &["config", "--local", "user.name", &name])?;
            call(&git, root, &["config", "--local", "user.email", &email])?;
        }
        Action::PublishRepository {
            name,
            private,
            branch,
            head,
        } => {
            crate::accounts::validate_repository_name(&name)?;
            call(&git, root, &["check-ref-format", "--branch", &branch])?;
            let current = repository(&git, root)?;
            if current.head != head || !valid_hash(&head) || current.branch != branch {
                return Err(io::Error::other(
                    "HEAD or the branch changed. Review publication again.",
                ));
            }
            if !current.remotes.is_empty() {
                return Err(io::Error::other(
                    "A remote is already configured. Use Push for an existing repository.",
                ));
            }
            crate::accounts::create_repository(root, &name, private)?;
            let reference = format!("HEAD:refs/heads/{branch}");
            call_network(&git,root,&["push","--porcelain","--no-follow-tags","--recurse-submodules=no","--set-upstream","--","origin",&reference]).map_err(|error| io::Error::other(format!("GitHub repository was created; uploading did not finish: {error}. Refresh remotes and retry Push.")))?;
        }

        Action::ReviewDiscard(path) => {
            return Ok(Outcome::Discard(review_discard(&git, root, path)?));
        }
        Action::Discard(review) => {
            discard_saved(&git, root, &review, crate::history::archive)?;
        }

        Action::StageAll(paths) => {
            if paths.is_empty() {
                return Err(io::Error::other("No saved changes to stage."));
            }
            for path in &paths {
                validate_path(path)?;
            }
            let mut arguments = vec!["add", "--"];
            arguments.extend(paths.iter().map(String::as_str));
            call(&git, root, &arguments)?;
        }
        Action::UnstageAll => {
            if call(&git, root, &["rev-parse", "--verify", "HEAD"]).is_ok() {
                call(&git, root, &["restore", "--staged", "--", "."])?;
            } else {
                call(&git, root, &["rm", "--cached", "-r", "--", "."])?;
            }
        }
        Action::AddRemote { name, url } => {
            validate_remote_name(&name)?;
            validate_remote_url(&url)?;
            call(&git, root, &["remote", "add", "--", &name, &url])?;
        }
        Action::Fetch(remote) => {
            check_remote(&git, root, &remote)?;
            call_network(&git, root, &["fetch", "--prune", "--", &remote])?;
        }
        Action::Pull {
            remote,
            branch,
            head,
        } => {
            check_network_target(&git, root, &remote, &branch, &head)?;
            call_network(
                &git,
                root,
                &["pull", "--ff-only", "--no-rebase", "--", &remote, &branch],
            )?;
        }
        Action::Push {
            remote,
            branch,
            head,
            publish,
        } => {
            check_network_target(&git, root, &remote, &branch, &head)?;
            let reference = format!("HEAD:refs/heads/{branch}");
            let mut args = vec![
                "push",
                "--porcelain",
                "--no-follow-tags",
                "--recurse-submodules=no",
            ];
            if publish {
                args.push("--set-upstream");
            }
            args.extend(["--", &remote, &reference]);
            call_network(&git, root, &args)?;
        }
        Action::Sync {
            remote,
            branch,
            head,
        } => {
            check_network_target(&git, root, &remote, &branch, &head)?;
            call_network(
                &git,
                root,
                &["pull", "--ff-only", "--no-rebase", "--", &remote, &branch],
            )?;
            let reference = format!("HEAD:refs/heads/{branch}");
            call_network(
                &git,
                root,
                &[
                    "push",
                    "--porcelain",
                    "--no-follow-tags",
                    "--recurse-submodules=no",
                    "--",
                    &remote,
                    &reference,
                ],
            )?;
        }
        Action::StashSave(message) => {
            if message.trim().is_empty() || message.len() > 4096 {
                return Err(io::Error::other("Enter a stash description."));
            }
            call(
                &git,
                root,
                &["stash", "push", "--include-untracked", "-m", &message],
            )?;
        }
        Action::Stashes => {
            let bytes = call(
                &git,
                root,
                &["stash", "list", "-50", "--format=%gd%x00%H%x00%gs"],
            )?;
            let mut stashes = Vec::new();
            for line in String::from_utf8_lossy(&bytes).lines() {
                let fields: Vec<_> = line.split('\0').collect();
                if fields.len() == 3 && valid_hash(fields[1]) {
                    stashes.push(Stash {
                        reference: fields[0].into(),
                        oid: fields[1].into(),
                        subject: fields[2].into(),
                    });
                }
            }
            return Ok(Outcome::Stashes(stashes));
        }
        Action::StashApply(oid) => {
            if !valid_hash(&oid) {
                return Err(io::Error::other("Select a stash from the list."));
            }
            call(&git, root, &["stash", "apply", "--index", &oid])?;
        }
        Action::RenameBranch { old, new } => {
            call(&git, root, &["check-ref-format", "--branch", &old])?;
            call(&git, root, &["check-ref-format", "--branch", &new])?;
            call(&git, root, &["branch", "--move", "--", &old, &new])?;
        }
        Action::DeleteBranch(branch) => {
            call(&git, root, &["check-ref-format", "--branch", &branch])?;
            call(&git, root, &["branch", "--delete", "--", &branch])?;
        }

        Action::Branches => {
            let raw = call(
                &git,
                root,
                &["for-each-ref", "--format=%(refname:short)", "refs/heads/"],
            )?;
            let branches = String::from_utf8_lossy(&raw)
                .lines()
                .map(str::to_owned)
                .collect();
            return Ok(Outcome::Branches(branches));
        }
        Action::Switch(branch) => {
            call(&git, root, &["check-ref-format", "--branch", &branch])?;
            call(&git, root, &["switch", &branch])?;
        }
        Action::CreateBranch(branch) => {
            call(&git, root, &["check-ref-format", "--branch", &branch])?;
            call(&git, root, &["switch", "-c", &branch])?;
        }
        Action::History => {
            let bytes = call(&git, root, &["log", "-50", "--format=%H%x00%an%x00%s"])?;
            let mut commits = Vec::new();
            for line in String::from_utf8_lossy(&bytes).lines() {
                let fields: Vec<_> = line.split('\0').collect();
                if fields.len() == 3 && valid_hash(fields[0]) {
                    commits.push(Commit {
                        oid: fields[0].into(),
                        author: fields[1].into(),
                        subject: fields[2].into(),
                    });
                }
            }
            return Ok(Outcome::History(commits));
        }
        Action::CommitDiff(hash) => {
            if !matches!(hash.len(), 40 | 64) || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(io::Error::other("Paste the full commit hash from History."));
            }
            let bytes = call(
                &git,
                root,
                &[
                    "show",
                    "--format=fuller",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-color",
                    &hash,
                    "--",
                ],
            )?;
            return Ok(Outcome::Diff(String::from_utf8_lossy(&bytes).into_owned()));
        }
        Action::Compare { base, head } => {
            if !valid_hash(&base) || !valid_hash(&head) {
                return Err(io::Error::other(
                    "Paste two full commit hashes from History.",
                ));
            }
            let bytes = call(
                &git,
                root,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-color",
                    &base,
                    &head,
                    "--",
                ],
            )?;
            return Ok(Outcome::Diff(String::from_utf8_lossy(&bytes).into_owned()));
        }
        Action::StageLines {
            path,
            expected,
            hunk,
            lines,
        } => {
            let hunks = split_hunks(&expected);
            let original = hunks
                .get(hunk)
                .ok_or_else(|| io::Error::other("The selected hunk is missing."))?;
            let patch = selected_line_patch(original, &lines)?;
            stage_patch(&git, root, &path, &expected, &patch)?;
        }
        Action::Worktrees => {
            let bytes = call(&git, root, &["worktree", "list", "--porcelain"])?;
            return Ok(Outcome::Info(String::from_utf8_lossy(&bytes).into_owned()));
        }
        Action::AddWorktree { path, branch } => {
            call(&git, root, &["check-ref-format", "--branch", &branch])?;
            if path.exists() {
                return Err(io::Error::other("The worktree destination already exists."));
            }
            let destination = path
                .to_str()
                .ok_or_else(|| io::Error::other("Worktree path must be Unicode."))?;
            call(
                &git,
                root,
                &["worktree", "add", "-b", &branch, "--", destination],
            )?;
        }
        Action::StageHunk {
            path,
            expected,
            patch,
        } => {
            if !split_hunks(&expected).contains(&patch) {
                return Err(io::Error::other("The hunk changed. Open a fresh diff."));
            }
            stage_patch(&git, root, &path, &expected, &patch)?;
        }
        Action::Diff { path, staged } => {
            validate_path(&path)?;
            if !staged && call(&git, root, &["ls-files", "--error-unmatch", "--", &path]).is_err() {
                let target = safe_working_path(root, &path)?;
                if target.symlink_metadata()?.file_type().is_symlink() {
                    return Err(io::Error::other("Open symbolic links in the explorer."));
                }
                let bytes = crate::packages::read_bounded(&target, LIMIT)?;
                let text = String::from_utf8(bytes)
                    .map_err(|_| io::Error::other("Binary file: stage it as a complete file."))?;
                if text.contains('\0') {
                    return Err(io::Error::other(
                        "Binary file: stage it as a complete file.",
                    ));
                }
                let lines: Vec<_> = text.lines().collect();
                let mut body = String::new();
                for line in &lines {
                    body.push('+');
                    body.push_str(line);
                    body.push('\n');
                }
                return Ok(Outcome::Diff(format!(
                    "diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n{body}",
                    lines.len()
                )));
            }
            let mut arguments = vec!["diff", "--no-ext-diff", "--no-textconv", "--no-color"];
            if staged {
                arguments.push("--cached");
            }
            arguments.extend(["--", &path]);
            let bytes = call(&git, root, &arguments)?;
            return Ok(Outcome::Diff(String::from_utf8_lossy(&bytes).into_owned()));
        }
        Action::Stage(path) => {
            validate_path(&path)?;
            call(&git, root, &["add", "--", &path])?;
        }
        Action::Unstage(path) => {
            validate_path(&path)?;
            if call(&git, root, &["rev-parse", "--verify", "HEAD"]).is_ok() {
                call(&git, root, &["restore", "--staged", "--", &path])?;
            } else {
                call(&git, root, &["rm", "--cached", "--", &path])?;
            }
        }
        Action::Commit { message, staged } => {
            if staged.is_empty() || staged != read_staged(&git, root)? {
                return Err(io::Error::other(
                    "The staged changes changed. Refresh Git status and review before committing.",
                ));
            }
            call(&git, root, &["commit", "-m", &message])?;
        }
        Action::Status => {}
    }
    let raw = call(
        &git,
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--no-renames",
            "--untracked-files=all",
            "--ignore-submodules=dirty",
        ],
    )?;
    let staged = read_staged(&git, root)?;
    let (changes, skipped) = parse_status(&raw);
    Ok(Outcome::Status(
        changes,
        skipped,
        staged,
        Box::new(repository(&git, root)?),
    ))
}

fn is_staged(change: &Change) -> bool {
    !matches!(change.index, b' ' | b'?' | b'!')
}
fn is_conflict(change: &Change) -> bool {
    change.index == b'U'
        || change.worktree == b'U'
        || matches!((change.index, change.worktree), (b'A', b'A') | (b'D', b'D'))
}

fn side_by_side(ui: &mut egui::Ui, diff: &str) {
    let rows = diff_rows(diff);
    egui::Grid::new("git-diff-columns")
        .striped(true)
        .min_col_width(350.0)
        .show(ui, |ui| {
            ui.strong("Before");
            ui.strong("After");
            ui.end_row();
            for (before, after) in rows {
                for (line, changed) in [(before, true), (after, false)] {
                    let (text, color) = if let Some((number, text, modified)) = line {
                        (
                            format!("{number:>5}  {text}"),
                            if modified {
                                if changed {
                                    crate::chrome::theme_red()
                                } else {
                                    crate::chrome::theme_green()
                                }
                            } else {
                                crate::chrome::theme_text()
                            },
                        )
                    } else {
                        (String::new(), crate::chrome::theme_text())
                    };
                    ui.add(
                        egui::Label::new(egui::RichText::new(text).monospace().color(color))
                            .selectable(true)
                            .wrap_mode(egui::TextWrapMode::Extend),
                    );
                }
                ui.end_row();
            }
        });
    if diff.is_empty() {
        ui.label("No textual changes.");
    }
}

type DiffLine = Option<(usize, String, bool)>;
fn diff_rows(diff: &str) -> Vec<(DiffLine, DiffLine)> {
    let mut rows = Vec::new();
    let (mut old, mut new) = (0usize, 0usize);
    let (mut removed, mut added) = (Vec::new(), Vec::new());
    let flush = |rows: &mut Vec<(DiffLine, DiffLine)>,
                 removed: &mut Vec<(usize, String, bool)>,
                 added: &mut Vec<(usize, String, bool)>| {
        let count = removed.len().max(added.len());
        for index in 0..count {
            rows.push((removed.get(index).cloned(), added.get(index).cloned()));
        }
        removed.clear();
        added.clear();
    };
    for line in diff.lines() {
        if line.starts_with("@@ ") {
            flush(&mut rows, &mut removed, &mut added);
            let ranges: Vec<_> = line.split_whitespace().collect();
            old = ranges
                .get(1)
                .and_then(|part| part.trim_start_matches('-').split(',').next())
                .and_then(|number| number.parse().ok())
                .unwrap_or(0);
            new = ranges
                .get(2)
                .and_then(|part| part.trim_start_matches('+').split(',').next())
                .and_then(|number| number.parse().ok())
                .unwrap_or(0);
        } else if line.starts_with("diff --git ") {
            flush(&mut rows, &mut removed, &mut added);
            rows.push((Some((0, line.into(), false)), None));
            old = 0;
            new = 0;
        } else if line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("index ")
            || line.starts_with("new file mode")
            || line.starts_with("deleted file mode")
        {
            continue;
        } else if let Some(text) = line.strip_prefix('-') {
            removed.push((old, text.into(), true));
            old += 1;
        } else if let Some(text) = line.strip_prefix('+') {
            added.push((new, text.into(), true));
            new += 1;
        } else if let Some(text) = line.strip_prefix(' ') {
            flush(&mut rows, &mut removed, &mut added);
            rows.push((
                Some((old, text.into(), false)),
                Some((new, text.into(), false)),
            ));
            old += 1;
            new += 1;
        }
    }
    flush(&mut rows, &mut removed, &mut added);
    rows
}

fn valid_hash(hash: &str) -> bool {
    matches!(hash.len(), 40 | 64) && hash.bytes().all(|character| character.is_ascii_hexdigit())
}

fn stage_patch(git: &Path, root: &Path, path: &str, expected: &str, patch: &str) -> io::Result<()> {
    validate_path(path)?;
    let fresh = call(
        git,
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--",
            path,
        ],
    )?;
    if fresh != expected.as_bytes() {
        return Err(io::Error::other(
            "The file or index changed. Open a fresh diff before staging.",
        ));
    }
    call_with_input(
        git,
        root,
        &["apply", "--cached", "--check", "--whitespace=nowarn", "-"],
        Some(patch.as_bytes()),
    )?;
    call_with_input(
        git,
        root,
        &["apply", "--cached", "--whitespace=nowarn", "-"],
        Some(patch.as_bytes()),
    )?;
    Ok(())
}

fn selected_line_patch(patch: &str, selected: &BTreeSet<usize>) -> io::Result<String> {
    let start = patch
        .find("\n@@ ")
        .map(|offset| offset + 1)
        .ok_or_else(|| io::Error::other("A normal unified hunk is required."))?;
    let body: Vec<_> = patch[start..].split_inclusive('\n').collect();
    if selected.is_empty()
        || selected.iter().any(|line| {
            body.get(*line)
                .is_none_or(|line| !line.starts_with(['+', '-']))
        })
    {
        return Err(io::Error::other("Select added or removed lines."));
    }
    let old_start = body[0]
        .split_whitespace()
        .nth(1)
        .and_then(|range| range.strip_prefix('-'))
        .and_then(|range| range.split(',').next())
        .and_then(|number| number.parse::<usize>().ok())
        .ok_or_else(|| io::Error::other("Invalid hunk coordinates."))?;
    let mut output = String::new();
    let mut old_count = 0;
    let mut new_count = 0;
    let mut kept = false;
    for (index, line) in body.iter().enumerate().skip(1) {
        match line.as_bytes().first() {
            Some(b' ') => {
                output.push_str(line);
                old_count += 1;
                new_count += 1;
                kept = true;
            }
            Some(b'-') => {
                old_count += 1;
                if selected.contains(&index) {
                    output.push_str(line);
                } else {
                    output.push(' ');
                    output.push_str(&line[1..]);
                    new_count += 1;
                }
                kept = true;
            }
            Some(b'+') => {
                kept = selected.contains(&index);
                if kept {
                    output.push_str(line);
                    new_count += 1;
                }
            }
            Some(b'\\') => {
                if kept {
                    output.push_str(line);
                }
            }
            _ => return Err(io::Error::other("Unsupported hunk line.")),
        }
    }
    let new_start = if old_count == 0 {
        old_start + 1
    } else if new_count == 0 {
        old_start.saturating_sub(1)
    } else {
        old_start
    };
    Ok(format!(
        "{}@@ -{old_start},{old_count} +{new_start},{new_count} @@\n{output}",
        &patch[..start]
    ))
}

fn colored_diff(ui: &mut egui::Ui, diff: &str) {
    for line in diff.lines() {
        let color = match line.as_bytes().first() {
            Some(b'+') => crate::chrome::theme_green(),
            Some(b'-') => crate::chrome::theme_red(),
            Some(b'@') => crate::chrome::theme_accent(),
            _ => ui.visuals().text_color(),
        };
        ui.add(
            egui::Label::new(egui::RichText::new(line).monospace().color(color)).selectable(true),
        );
    }
}

fn split_hunks(diff: &str) -> Vec<String> {
    let starts: Vec<_> = diff
        .match_indices(
            "
@@ ",
        )
        .map(|(index, _)| index + 1)
        .collect();
    let Some(first) = starts.first() else {
        return vec![];
    };
    // Combined, binary, rename, mode and multi-file patches require a complete-file operation.
    if diff.matches("diff --git ").count() != 1
        || diff.contains(
            "
old mode ",
        )
        || diff.contains(
            "
new file mode ",
        )
        || diff.contains(
            "
deleted file mode ",
        )
        || diff.contains(
            "
rename from ",
        )
    {
        return vec![];
    }
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts.get(index + 1).copied().unwrap_or(diff.len());
            format!("{}{}", &diff[..*first], &diff[*start..end])
        })
        .collect()
}

fn read_staged(git: &Path, root: &Path) -> io::Result<Vec<u8>> {
    call(
        git,
        root,
        &[
            "diff",
            "--cached",
            "--raw",
            "--no-abbrev",
            "--no-renames",
            "--no-ext-diff",
            "--no-textconv",
            "-z",
        ],
    )
}

fn validate_path(path: &str) -> io::Result<()> {
    if !path.is_empty()
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        Ok(())
    } else {
        Err(io::Error::other("Invalid repository path."))
    }
}

fn parse_status(raw: &[u8]) -> (Vec<Change>, usize) {
    let mut changes = Vec::new();
    let mut skipped = 0;
    for record in raw
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let [index, worktree, b' ', bytes @ ..] = record else {
            skipped += 1;
            continue;
        };
        let Ok(path) = std::str::from_utf8(bytes) else {
            skipped += 1;
            continue;
        };
        if validate_path(path).is_err() {
            skipped += 1;
            continue;
        }
        changes.push(Change {
            index: *index,
            worktree: *worktree,
            path: path.to_owned(),
        });
    }
    (changes, skipped)
}

pub(crate) fn locate_git(root: &Path) -> io::Result<PathBuf> {
    let executable = if cfg!(windows) { "git.exe" } else { "git" };
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|path| path.is_absolute())
        .filter_map(|path| path.join(executable).canonicalize().ok())
        .find(|path| path.is_file() && !path.starts_with(root))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Git was not found on the system PATH.",
            )
        })
}

fn call(program: &Path, cwd: &Path, args: &[&str]) -> io::Result<Vec<u8>> {
    call_with_input(program, cwd, args, None)
}

fn call_with_input(
    program: &Path,
    cwd: &Path,
    args: &[&str],
    input: Option<&[u8]>,
) -> io::Result<Vec<u8>> {
    call_with_timeout(program, cwd, args, input, Duration::from_secs(15))
}

fn call_network(program: &Path, cwd: &Path, args: &[&str]) -> io::Result<Vec<u8>> {
    call_with_timeout(program, cwd, args, None, Duration::from_secs(120))
}

fn call_with_timeout(
    program: &Path,
    cwd: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    timeout: Duration,
) -> io::Result<Vec<u8>> {
    let cancellation = CANCELLATION.with(|flag| flag.borrow().clone());
    let cancelled = || {
        cancellation
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
    };
    if cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Git operation cancelled. Refresh before retrying.",
        ));
    }
    let mut command = Command::new(program);
    command.env("PATH", crate::project::environment_path());
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(key);
        }
    }
    if let Some(gh) = crate::accounts::credential_helper(cwd) {
        let path = if cfg!(windows) {
            gh.to_string_lossy().replace('\\', "/")
        } else {
            gh.to_string_lossy().into_owned()
        };
        let escaped = path.replace('\'', "'\"'\"'");
        command.args([
            "-c",
            "credential.https://github.com.helper=",
            "-c",
            &format!("credential.https://github.com.helper=!'{escaped}' auth git-credential"),
        ]);
    }
    if args
        .first()
        .is_some_and(|verb| matches!(*verb, "add" | "rm" | "restore" | "diff" | "ls-files"))
    {
        command.arg("--literal-pathspecs");
    }
    command.env_remove("GH_DEBUG");
    let mut child = command
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "--no-pager",
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "protocol.ext.allow=never",
        ])
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(input) = input {
        let mut stdin = child.stdin.take().expect("piped stdin");
        let bytes = input.to_owned();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }
    let output = collect(child.stdout.take().expect("piped stdout"));
    let errors = collect(child.stderr.take().expect("piped stderr"));
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            let bytes = read_result(output, deadline)?;
            let errors = read_result(errors, deadline)?;
            if !status.success() {
                return Err(io::Error::other(
                    crate::accounts::redact(&String::from_utf8_lossy(&errors))
                        .trim()
                        .to_owned(),
                ));
            }
            return Ok(bytes);
        }
        if cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Git operation cancelled. Some completed steps may remain; refresh status before retrying.",
            ));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Git operation timed out. Refresh Git status before retrying.",
            ));
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

pub(crate) fn collect(stream: impl Read + Send + 'static) -> mpsc::Receiver<io::Result<Vec<u8>>> {
    let (send, receive) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stream
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .and_then(|length| {
                if length as u64 > LIMIT {
                    Err(io::Error::other("Git output exceeded 512 KiB."))
                } else {
                    Ok(bytes)
                }
            });
        let _ = send.send(result);
    });
    receive
}

pub(crate) fn read_result(
    receiver: mpsc::Receiver<io::Result<Vec<u8>>>,
    deadline: Instant,
) -> io::Result<Vec<u8>> {
    receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(io::Error::other)?
}

fn safe_working_path(root: &Path, path: &str) -> io::Result<PathBuf> {
    validate_path(path)?;
    let target = root.join(path);
    let mut ancestor = target
        .parent()
        .ok_or_else(|| io::Error::other("Invalid working path."))?;
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| io::Error::other("Missing repository parent."))?;
    }
    if !ancestor.canonicalize()?.starts_with(root.canonicalize()?) {
        return Err(io::Error::other("The working path leaves the repository."));
    }
    if target
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(io::Error::other(
            "Symbolic links require a complete Git operation outside this text-discard flow.",
        ));
    }
    Ok(target)
}
fn saved_text(path: &Path) -> io::Result<Option<String>> {
    match crate::packages::read_bounded(path, LIMIT) {
        Ok(bytes) => {
            let text = String::from_utf8(bytes).map_err(|_| {
                io::Error::other("Only UTF-8 text can be archived by this discard flow.")
            })?;
            if text.contains('\0') {
                return Err(io::Error::other("Binary files cannot use text discard."));
            }
            Ok(Some(text))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
fn review_discard(git: &Path, root: &Path, path: String) -> io::Result<DiscardReview> {
    let target = safe_working_path(root, &path)?;
    call(git, root, &["ls-files", "--error-unmatch", "--", &path])?;
    let head = call(git, root, &["rev-parse", "--verify", "HEAD"])
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
        .unwrap_or_default();
    Ok(DiscardReview {
        path,
        before: saved_text(&target)?,
        index: read_staged(git, root)?,
        head,
    })
}
fn discard_saved(
    git: &Path,
    root: &Path,
    review: &DiscardReview,
    archive: impl FnOnce(&Path, &str) -> io::Result<()>,
) -> io::Result<()> {
    let target = safe_working_path(root, &review.path)?;
    let head = call(git, root, &["rev-parse", "--verify", "HEAD"])
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
        .unwrap_or_default();
    if head != review.head
        || read_staged(git, root)? != review.index
        || saved_text(&target)? != review.before
    {
        return Err(io::Error::other(
            "The file, index or HEAD changed. Review the discard again.",
        ));
    }
    if let Some(before) = &review.before {
        archive(&target, before)?;
    }
    call(git, root, &["restore", "--worktree", "--", &review.path])?;
    Ok(())
}

fn validate_remote_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.len() > 100
        || name.starts_with('-')
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(io::Error::other(
            "Remote names use letters, numbers, dots, underscores and hyphens.",
        ));
    }
    Ok(())
}

fn validate_remote_url(value: &str) -> io::Result<()> {
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(io::Error::other("Invalid remote URL."));
    }
    if Path::new(value).is_absolute() {
        return Ok(());
    }
    if let Ok(url) = url::Url::parse(value) {
        let host_valid = url.scheme() == "file"
            || url
                .host_str()
                .is_some_and(|host| !host.is_empty() && !host.starts_with('-'));
        if matches!(url.scheme(), "https" | "ssh" | "file")
            && host_valid
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && (url.scheme() != "https" || url.username().is_empty())
        {
            return Ok(());
        }
    }
    if !value.contains("://") && !value.contains("::") {
        if let Some((host, path)) = value.split_once(':') {
            if host.contains('@')
                && !host
                    .rsplit('@')
                    .next()
                    .is_some_and(|host| host.starts_with('-'))
                && !path.is_empty()
                && host.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'@' | b'.' | b'-' | b'_')
                })
            {
                return Ok(());
            }
        }
    }
    Err(io::Error::other(
        "Use HTTPS without embedded credentials, SSH, or an absolute local repository path.",
    ))
}

fn check_remote(git: &Path, root: &Path, name: &str) -> io::Result<()> {
    validate_remote_name(name)?;
    let url = call(git, root, &["remote", "get-url", "--", name])?;
    validate_remote_url(String::from_utf8_lossy(&url).trim())?;
    for url in String::from_utf8_lossy(&call(
        git,
        root,
        &["remote", "get-url", "--push", "--all", "--", name],
    )?)
    .lines()
    {
        validate_remote_url(url)?;
    }
    Ok(())
}

fn check_network_target(
    git: &Path,
    root: &Path,
    remote: &str,
    branch: &str,
    expected_head: &str,
) -> io::Result<()> {
    check_remote(git, root, remote)?;
    call(git, root, &["check-ref-format", "--branch", branch])?;
    let head = call(git, root, &["rev-parse", "--verify", "HEAD"])?;
    if !valid_hash(expected_head) || String::from_utf8_lossy(&head).trim() != expected_head {
        return Err(io::Error::other(
            "HEAD changed. Refresh and review the remote operation again.",
        ));
    }
    Ok(())
}

fn repository(git: &Path, root: &Path) -> io::Result<Repository> {
    let branch = call(git, root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
        .unwrap_or_else(|_| "Detached HEAD".into());
    let head = call(git, root, &["rev-parse", "--verify", "HEAD"])
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
        .unwrap_or_default();
    let upstream = call(
        git,
        root,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )
    .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
    .unwrap_or_default();
    let counts = if upstream.is_empty() {
        Vec::new()
    } else {
        call(
            git,
            root,
            &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        )
        .unwrap_or_default()
    };
    let counts: Vec<usize> = String::from_utf8_lossy(&counts)
        .split_whitespace()
        .filter_map(|value| value.parse().ok())
        .collect();
    let raw = call(git, root, &["remote"])?;
    let mut remotes = Vec::new();
    for name in String::from_utf8_lossy(&raw).lines().take(30) {
        if validate_remote_name(name).is_ok() {
            let url = call(git, root, &["remote", "get-url", "--", name])?;
            remotes.push(Remote {
                name: name.into(),
                push_urls: String::from_utf8_lossy(&call(
                    git,
                    root,
                    &["remote", "get-url", "--push", "--all", "--", name],
                )?)
                .lines()
                .map(crate::accounts::redact)
                .collect(),
                url: crate::accounts::redact(&String::from_utf8_lossy(&url))
                    .trim()
                    .into(),
            });
        }
    }
    let config_value = |key: &str| {
        call(git, root, &["config", "--get", key])
            .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
            .unwrap_or_default()
    };
    Ok(Repository {
        author_name: config_value("user.name"),
        author_email: config_value("user.email"),
        root: root.to_owned(),
        branch,
        head,
        upstream,
        ahead: counts.first().copied().unwrap_or(0),
        behind: counts.get(1).copied().unwrap_or(0),
        remotes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_keeps_literal_paths_and_excludes_invalid_entries() {
        let (changes, skipped) = parse_status(b" M src/file name.rs\0?? odd\nfile\0 M ../escape\0");
        assert_eq!(skipped, 1);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].index, b' ');
        assert_eq!(changes[0].worktree, b'M');
        assert_eq!(changes[0].path, "src/file name.rs");
        assert_eq!(changes[1].path, "odd\nfile");
    }

    #[test]
    fn stages_only_selected_hunk_and_refuses_stale_patch() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().canonicalize()?;
        let git = locate_git(&root)?;
        call(&git, &root, &["init", "--template="])?;
        use std::fmt::Write as _;
        let mut before = String::new();
        for line in 0..40 {
            writeln!(&mut before, "line {line}").unwrap();
        }
        std::fs::write(root.join("sample.txt"), &before)?;
        execute(&root, Action::Stage("sample.txt".into()))?;
        let after = before
            .replace("line 2\n", "first change\n")
            .replace("line 35\n", "second change\n");
        std::fs::write(root.join("sample.txt"), after)?;
        let Outcome::Diff(expected) = execute(
            &root,
            Action::Diff {
                path: "sample.txt".into(),
                staged: false,
            },
        )?
        else {
            panic!("diff expected")
        };
        let hunks = split_hunks(&expected);
        assert_eq!(hunks.len(), 2);
        execute(
            &root,
            Action::StageHunk {
                path: "sample.txt".into(),
                expected: expected.clone(),
                patch: hunks[0].clone(),
            },
        )?;
        let index = String::from_utf8(call(&git, &root, &["show", ":sample.txt"])?).unwrap();
        assert!(index.contains("first change"));
        assert!(!index.contains("second change"));
        assert!(
            execute(
                &root,
                Action::StageHunk {
                    path: "sample.txt".into(),
                    expected,
                    patch: hunks[1].clone()
                }
            )
            .is_err()
        );
        let Outcome::Diff(fresh) = execute(
            &root,
            Action::Diff {
                path: "sample.txt".into(),
                staged: false,
            },
        )?
        else {
            panic!("diff expected")
        };
        let hunk = split_hunks(&fresh).remove(0);
        let body = &hunk[hunk.find("@@ ").unwrap()..];
        let selected = body
            .lines()
            .position(|line| line == "+second change")
            .unwrap();
        execute(
            &root,
            Action::StageLines {
                path: "sample.txt".into(),
                expected: fresh,
                hunk: 0,
                lines: BTreeSet::from([selected]),
            },
        )?;
        let index = String::from_utf8(call(&git, &root, &["show", ":sample.txt"])?).unwrap();
        assert!(index.contains("line 35\nsecond change\n"));
        assert!(index.contains("first change"));
        Ok(())
    }

    #[test]
    fn stages_literal_path_and_commits_only_that_file() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let root = folder.path().canonicalize()?;
        let git = locate_git(&root)?;
        call(&git, &root, &["init", "--template="])?;
        for (key, value) in [
            ("user.name", "ForgeIDE Fixture"),
            ("user.email", "fixture@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", ".git/disabled-hooks"),
        ] {
            call(&git, &root, &["config", "--local", key, value])?;
        }
        std::fs::write(root.join("source one.rs"), "first")?;
        std::fs::write(root.join("leave.txt"), "second")?;
        execute(&root, Action::Stage("source one.rs".into()))?;
        execute(&root, Action::Unstage("source one.rs".into()))?;
        let Outcome::Status(_, _, staged, _) =
            execute(&root, Action::Stage("source one.rs".into()))?
        else {
            panic!("expected status");
        };
        let Outcome::Status(changes, _, _, _) = execute(
            &root,
            Action::Commit {
                message: "fixture commit".into(),
                staged,
            },
        )?
        else {
            panic!("expected status");
        };
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "leave.txt");
        assert_eq!(
            call(&git, &root, &["show", "HEAD:source one.rs"])?,
            b"first"
        );
        let base = String::from_utf8(call(&git, &root, &["rev-parse", "HEAD"])?)
            .unwrap()
            .trim()
            .to_owned();
        std::fs::write(root.join("source one.rs"), "first improved")?;
        let Outcome::Status(_, _, staged, _) =
            execute(&root, Action::Stage("source one.rs".into()))?
        else {
            panic!("status expected")
        };
        execute(
            &root,
            Action::Commit {
                message: "improve".into(),
                staged,
            },
        )?;
        let head = String::from_utf8(call(&git, &root, &["rev-parse", "HEAD"])?)
            .unwrap()
            .trim()
            .to_owned();
        let Outcome::Diff(diff) = execute(&root, Action::Compare { base, head })? else {
            panic!("comparison expected")
        };
        assert!(diff.contains("+first improved"));
        Ok(())
    }
}

#[cfg(test)]
mod professional_tests {
    use super::*;

    fn initialize(root: &Path) -> io::Result<PathBuf> {
        std::fs::create_dir_all(root)?;
        let git = locate_git(root)?;
        execute(root, Action::Init)?;
        call(&git, root, &["config", "user.name", "Forge Test"])?;
        call(
            &git,
            root,
            &["config", "user.email", "forge@example.invalid"],
        )?;
        call(&git, root, &["config", "commit.gpgsign", "false"])?;
        Ok(git)
    }
    fn snapshot(root: &Path) -> io::Result<Repository> {
        match execute(root, Action::Status)? {
            Outcome::Status(_, _, _, repository) => Ok(*repository),
            _ => Err(io::Error::other("Expected repository state.")),
        }
    }
    fn commit(root: &Path, text: &str) -> io::Result<String> {
        std::fs::write(root.join("file.txt"), text)?;
        execute(root, Action::StageAll(vec!["file.txt".into()]))?;
        let git = locate_git(root)?;
        execute(
            root,
            Action::Commit {
                message: format!("Save {text}"),
                staged: read_staged(&git, root)?,
            },
        )?;
        Ok(snapshot(root)?.head)
    }

    #[test]
    fn local_remote_publish_fetch_pull_sync_and_stale_head_are_checked() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("local");
        let git = initialize(&root)?;
        let bare = directory.path().join("origin.git");
        call(
            &git,
            directory.path(),
            &["init", "--bare", "--template=", bare.to_str().unwrap()],
        )?;
        let first = commit(&root, "first\n")?;
        execute(
            &root,
            Action::RenameBranch {
                old: snapshot(&root)?.branch,
                new: "main".into(),
            },
        )?;
        execute(
            &root,
            Action::AddRemote {
                name: "origin".into(),
                url: bare.to_str().unwrap().into(),
            },
        )?;
        execute(
            &root,
            Action::Push {
                remote: "origin".into(),
                branch: "main".into(),
                head: first,
                publish: true,
            },
        )?;
        call(&git, &bare, &["symbolic-ref", "HEAD", "refs/heads/main"])?;
        assert_eq!(snapshot(&root)?.upstream, "origin/main");
        let other = directory.path().join("other");
        call(
            &git,
            directory.path(),
            &[
                "clone",
                "--template=",
                bare.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        )?;
        call(&git, &other, &["config", "user.name", "Other Test"])?;
        call(
            &git,
            &other,
            &["config", "user.email", "other@example.invalid"],
        )?;
        call(&git, &other, &["config", "commit.gpgsign", "false"])?;
        let second = commit(&other, "second\n")?;
        execute(
            &other,
            Action::Push {
                remote: "origin".into(),
                branch: "main".into(),
                head: second,
                publish: false,
            },
        )?;
        execute(&root, Action::Fetch("origin".into()))?;
        assert_eq!(snapshot(&root)?.behind, 1);
        let before = snapshot(&root)?.head;
        execute(
            &root,
            Action::Pull {
                remote: "origin".into(),
                branch: "main".into(),
                head: before.clone(),
            },
        )?;
        assert_eq!(std::fs::read_to_string(root.join("file.txt"))?, "second\n");
        assert!(
            execute(
                &root,
                Action::Push {
                    remote: "origin".into(),
                    branch: "main".into(),
                    head: before,
                    publish: false
                }
            )
            .is_err()
        );
        let third = commit(&root, "third\n")?;
        assert_eq!(snapshot(&root)?.ahead, 1);
        execute(
            &root,
            Action::Sync {
                remote: "origin".into(),
                branch: "main".into(),
                head: third.clone(),
            },
        )?;
        assert_eq!(
            String::from_utf8_lossy(&call(&git, &bare, &["rev-parse", "refs/heads/main"])?).trim(),
            third
        );
        assert_eq!(snapshot(&root)?.ahead, 0);
        Ok(())
    }

    #[test]
    fn fast_forward_pull_refuses_divergence_and_keeps_local_content() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("local");
        let git = initialize(&root)?;
        let base = commit(&root, "base\n")?;
        let branch = snapshot(&root)?.branch;
        let bare = directory.path().join("origin.git");
        call(
            &git,
            directory.path(),
            &["init", "--bare", "--template=", bare.to_str().unwrap()],
        )?;
        execute(
            &root,
            Action::AddRemote {
                name: "origin".into(),
                url: bare.to_str().unwrap().into(),
            },
        )?;
        execute(
            &root,
            Action::Push {
                remote: "origin".into(),
                branch: branch.clone(),
                head: base,
                publish: true,
            },
        )?;
        let local = commit(&root, "local change\n")?;
        // A separate remote history advances from the shared parent.
        let other = directory.path().join("other");
        call(
            &git,
            &bare,
            &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")],
        )?;
        call(
            &git,
            directory.path(),
            &[
                "clone",
                "--template=",
                bare.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        )?;
        call(&git, &other, &["config", "user.name", "Other"])?;
        call(
            &git,
            &other,
            &["config", "user.email", "other@example.invalid"],
        )?;
        call(&git, &other, &["config", "commit.gpgsign", "false"])?;
        let remote = commit(&other, "remote change\n")?;
        execute(
            &other,
            Action::Push {
                remote: "origin".into(),
                branch: branch.clone(),
                head: remote,
                publish: false,
            },
        )?;
        let result = execute(
            &root,
            Action::Pull {
                remote: "origin".into(),
                branch,
                head: local.clone(),
            },
        );
        assert!(result.is_err());
        assert_eq!(snapshot(&root)?.head, local);
        assert_eq!(
            std::fs::read_to_string(root.join("file.txt"))?,
            "local change\n"
        );
        Ok(())
    }

    #[test]
    fn stashes_include_untracked_files_and_apply_without_removing_the_backup() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let git = initialize(root)?;
        commit(root, "base\n")?;
        std::fs::write(root.join("file.txt"), "modified\n")?;
        std::fs::write(root.join("new.txt"), "untracked\n")?;
        execute(root, Action::StashSave("Work in progress".into()))?;
        assert_eq!(std::fs::read_to_string(root.join("file.txt"))?, "base\n");
        assert!(!root.join("new.txt").exists());
        let Outcome::Stashes(stashes) = execute(root, Action::Stashes)? else {
            panic!("stash list");
        };
        assert_eq!(stashes.len(), 1);
        execute(root, Action::StashApply(stashes[0].oid.clone()))?;
        assert_eq!(
            std::fs::read_to_string(root.join("file.txt"))?,
            "modified\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("new.txt"))?,
            "untracked\n"
        );
        assert!(!call(&git, root, &["stash", "list"])?.is_empty());
        Ok(())
    }

    #[test]
    fn finds_containing_repository_and_handles_unborn_index_operations() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        initialize(root)?;
        assert!(snapshot(root)?.head.is_empty());
        std::fs::create_dir(root.join("nested"))?;
        assert_eq!(snapshot(&root.join("nested"))?.root, root.canonicalize()?);
        std::fs::write(root.join("file.txt"), "new\n")?;
        execute(root, Action::StageAll(vec!["file.txt".into()]))?;
        execute(root, Action::UnstageAll)?;
        let Outcome::Status(changes, _, staged, _) = execute(root, Action::Status)? else {
            panic!("status");
        };
        assert!(staged.is_empty());
        assert_eq!(changes[0].index, b'?');
        Ok(())
    }

    #[test]
    fn untracked_diff_is_visible_and_unsafe_remote_schemes_are_rejected() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        initialize(root)?;
        std::fs::write(root.join("new file.txt"), "hello\nworld\n")?;
        let Outcome::Diff(diff) = execute(
            root,
            Action::Diff {
                path: "new file.txt".into(),
                staged: false,
            },
        )?
        else {
            panic!("diff");
        };
        assert!(diff.contains("+hello\n+world"));
        assert!(split_hunks(&diff).is_empty());
        for value in [
            "ext::sh -c evil",
            "http://github.com/a/b",
            "https://user:token@github.com/a/b",
            "git://host/repo",
            "\nssh://host/repo",
        ] {
            assert!(validate_remote_url(value).is_err(), "{value}");
        }
        for value in [
            "https://github.com/a/b.git",
            "git@github.com:a/b.git",
            "ssh://git@github.com/a/b.git",
        ] {
            validate_remote_url(value)?;
        }
        assert!(validate_remote_name("--upload-pack=evil").is_err());
        Ok(())
    }

    #[test]
    fn side_by_side_rows_keep_line_numbers_and_align_changed_blocks() {
        let rows = diff_rows(
            "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -2,2 +2,3 @@\n-old\n+new\n+extra\n same\n",
        );
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[1].0, Some((2, "old".into(), true)));
        assert_eq!(rows[1].1, Some((2, "new".into(), true)));
        assert!(rows[2].0.is_none());
        assert_eq!(rows[2].1, Some((3, "extra".into(), true)));
        assert_eq!(rows[3].0, Some((3, "same".into(), false)));
        assert_eq!(rows[3].1, Some((4, "same".into(), false)));
    }

    #[test]
    fn history_is_selectable_and_branch_deletion_refuses_unmerged_work() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let git = initialize(root)?;
        let first = commit(root, "base\n")?;
        let original = snapshot(root)?.branch;
        execute(root, Action::CreateBranch("feature/work".into()))?;
        commit(root, "feature\n")?;
        execute(root, Action::Switch(original.clone()))?;
        assert!(execute(root, Action::DeleteBranch("feature/work".into())).is_err());
        execute(
            root,
            Action::RenameBranch {
                old: original,
                new: "main".into(),
            },
        )?;
        assert_eq!(snapshot(root)?.branch, "main");
        let Outcome::History(commits) = execute(root, Action::History)? else {
            panic!("history");
        };
        assert_eq!(commits[0].oid, first);
        assert_eq!(commits[0].author, "Forge Test");
        assert_eq!(
            String::from_utf8_lossy(&call(&git, root, &["show", "HEAD:file.txt"])?),
            "base\n"
        );
        Ok(())
    }
    #[test]
    fn discard_archives_saved_text_and_refuses_stale_or_failed_backups() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let git = initialize(root)?;
        commit(root, "base\n")?;
        std::fs::write(root.join("file.txt"), "saved changes\n")?;
        let review = review_discard(&git, root, "file.txt".into())?;
        let mut backup = String::new();
        discard_saved(&git, root, &review, |_, before| {
            backup = before.to_owned();
            Ok(())
        })?;
        assert_eq!(backup, "saved changes\n");
        assert_eq!(std::fs::read_to_string(root.join("file.txt"))?, "base\n");
        std::fs::write(root.join("file.txt"), "first\n")?;
        let review = review_discard(&git, root, "file.txt".into())?;
        std::fs::write(root.join("file.txt"), "newer\n")?;
        assert!(
            discard_saved(&git, root, &review, |_, _| panic!(
                "stale review must not archive"
            ))
            .is_err()
        );
        let review = review_discard(&git, root, "file.txt".into())?;
        assert!(
            discard_saved(&git, root, &review, |_, _| Err(io::Error::other(
                "storage unavailable"
            )))
            .is_err()
        );
        assert_eq!(std::fs::read_to_string(root.join("file.txt"))?, "newer\n");
        Ok(())
    }

    #[test]
    fn clone_identity_and_distinct_push_destinations_are_integrated() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("source");
        let git = initialize(&root)?;
        commit(&root, "base\n")?;
        execute(
            &root,
            Action::SetIdentity {
                name: "Repository Author".into(),
                email: "author@example.invalid".into(),
            },
        )?;
        assert_eq!(snapshot(&root)?.author_email, "author@example.invalid");
        let clone = directory.path().join("clone");
        let Outcome::Cloned(cloned) = execute(
            directory.path(),
            Action::Clone {
                url: root.to_str().unwrap().into(),
                destination: clone.clone(),
            },
        )?
        else {
            panic!("clone");
        };
        assert_eq!(cloned, clone.canonicalize()?);
        assert_eq!(std::fs::read_to_string(clone.join("file.txt"))?, "base\n");
        assert!(
            execute(
                directory.path(),
                Action::Clone {
                    url: root.to_str().unwrap().into(),
                    destination: clone.clone()
                }
            )
            .is_err()
        );
        call(
            &git,
            &clone,
            &["config", "remote.origin.pushurl", "ext::must-not-execute"],
        )?;
        let state = snapshot(&clone)?;
        assert_eq!(state.remotes[0].push_urls, vec!["ext::must-not-execute"]);
        assert!(check_remote(&git, &clone, "origin").is_err());
        Ok(())
    }

    #[test]
    fn rejects_stale_publication_before_contacting_github() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        initialize(root)?;
        let first = commit(root, "first\n")?;
        let branch = snapshot(root)?.branch;
        commit(root, "newer\n")?;
        assert!(
            execute(
                root,
                Action::PublishRepository {
                    name: "octocat/forge-fixture".into(),
                    private: true,
                    branch,
                    head: first
                }
            )
            .is_err()
        );
        assert!(snapshot(root)?.remotes.is_empty());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_interrupts_the_running_git_process() -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("fake-git");
        std::fs::write(&program, "#!/bin/sh\nexec sleep 10\n")?;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700))?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = cancelled.clone();
        let cwd = directory.path().to_owned();
        let started = Instant::now();
        let worker = std::thread::spawn(move || {
            CANCELLATION.with(|flag| *flag.borrow_mut() = Some(signal));
            call_with_timeout(&program, &cwd, &["status"], None, Duration::from_secs(15))
        });
        std::thread::sleep(Duration::from_millis(100));
        cancelled.store(true, Ordering::Relaxed);
        assert_eq!(
            worker.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        assert!(started.elapsed() < Duration::from_secs(3));
        Ok(())
    }
    #[test]
    fn branch_switch_at_the_same_commit_invalidates_a_reviewed_operation() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        initialize(root)?;
        let head = commit(root, "base\n")?;
        let branch = snapshot(root)?.branch;
        execute(root, Action::CreateBranch("another-branch".into()))?;
        assert_eq!(snapshot(root)?.head, head);
        let result = execute(
            root,
            Action::CheckedBranch {
                branch,
                action: Box::new(Action::Push {
                    remote: "missing".into(),
                    branch: "main".into(),
                    head,
                    publish: false,
                }),
            },
        );
        let Err(error) = result else {
            panic!("stale branch must be rejected");
        };
        assert!(error.to_string().contains("active branch changed"));
        Ok(())
    }
}
