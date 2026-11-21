use crate::file::OpenFile;
use eframe::egui::{self, TextEdit};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
    sync::mpsc,
};

#[derive(Clone)]
enum Operation {
    Create { folder: bool },
    Rename { source: PathBuf },
    Remove { source: PathBuf },
    Restore { record: PathBuf },
}
struct Form {
    root: PathBuf,
    operation: Operation,
    value: String,
    error: String,
}
pub enum Outcome {
    Created(PathBuf, bool),
    Renamed(PathBuf, PathBuf),
    Removed(PathBuf),
    Restored(PathBuf),
}
#[derive(Default)]
pub struct Controller {
    form: Option<Form>,
    job: Option<mpsc::Receiver<io::Result<Outcome>>>,
    pub notice: String,
    recoveries: Vec<(PathBuf, PathBuf)>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    root: PathBuf,
    relative: PathBuf,
}

fn recovery_directory() -> io::Result<PathBuf> {
    let path = crate::recovery::directory()?
        .parent()
        .expect("app data")
        .join("ExplorerRecovery");
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&path)?;
    Ok(path)
}

fn relative_path(value: &Path) -> io::Result<()> {
    if value.as_os_str().is_empty()
        || value
            .components()
            .any(|part| !matches!(part,Component::Normal(name) if name != ".git"))
    {
        return Err(io::Error::other(
            "Use a relative workspace path without '..' or '.git'.",
        ));
    }
    if value.to_string_lossy().chars().any(char::is_control) {
        return Err(io::Error::other(
            "Control characters are not valid in a file name.",
        ));
    }
    Ok(())
}

fn destination(root: &Path, relative: &Path) -> io::Result<PathBuf> {
    relative_path(relative)?;
    let root = root.canonicalize()?;
    let path = root.join(relative);
    let mut parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing parent."))?;
    while !parent.exists() {
        parent = parent
            .parent()
            .ok_or_else(|| io::Error::other("Missing workspace parent."))?;
    }
    if !parent.canonicalize()?.starts_with(&root) {
        return Err(io::Error::other("This path leaves the workspace."));
    }
    Ok(path)
}
fn source(root: &Path, path: &Path) -> io::Result<PathBuf> {
    let original_root = root;
    let root = root.canonicalize()?;
    let relative = path
        .strip_prefix(original_root)
        .or_else(|_| path.strip_prefix(&root))
        .map_err(io::Error::other)?;
    let path = destination(&root, relative)?;
    if path.symlink_metadata()?.file_type().is_symlink() {
        return Err(io::Error::other(
            "Use the system file manager for symbolic links.",
        ));
    }
    if !path.canonicalize()?.starts_with(&root) {
        return Err(io::Error::other("This item leaves the workspace."));
    }
    Ok(path)
}

/// The OS performs an exclusive rename, so an existing destination is never replaced.
fn rename_exclusive(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let from = std::ffi::CString::new(from.as_os_str().as_bytes()).map_err(io::Error::other)?;
        let to = std::ffi::CString::new(to.as_os_str().as_bytes()).map_err(io::Error::other)?;
        #[cfg(target_os = "macos")]
        // SAFETY: both pointers are valid NUL-terminated paths for the duration of this call.
        let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        #[cfg(target_os = "linux")]
        // SAFETY: both pointers are valid NUL-terminated paths and flags request no replacement.
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let result = return Err(io::Error::other(
            "Exclusive moves are not available on this platform.",
        ));
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileW(from: *const u16, to: *const u16) -> i32;
        }
        let mut from: Vec<u16> = from.as_os_str().encode_wide().collect();
        let mut to: Vec<u16> = to.as_os_str().encode_wide().collect();
        if from.contains(&0) || to.contains(&0) {
            return Err(io::Error::other("Invalid path."));
        }
        from.push(0);
        to.push(0);
        // SAFETY: both pointers reference live, NUL-terminated UTF-16 path buffers.
        if unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) } == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn remove_recoverably(root: &Path, path: &Path, directory: &Path) -> io::Result<PathBuf> {
    let path = source(root, path)?;
    let root = root.canonicalize()?;
    let temporary = tempfile::Builder::new()
        .prefix("removed-")
        .tempdir_in(directory)?;
    let record = Record {
        version: 1,
        root: root.clone(),
        relative: path
            .strip_prefix(&root)
            .map_err(io::Error::other)?
            .to_owned(),
    };
    crate::packages::atomic_write(
        &temporary.path().join("record.json"),
        &serde_json::to_vec(&record)?,
    )?;
    rename_exclusive(&path, &temporary.path().join("payload"))?;
    Ok(temporary.keep())
}
fn restore(record_directory: &Path, storage: &Path) -> io::Result<PathBuf> {
    let directory = record_directory.canonicalize()?;
    if directory.parent() != Some(storage.canonicalize()?.as_path())
        || directory.symlink_metadata()?.file_type().is_symlink()
    {
        return Err(io::Error::other("Invalid recovery entry."));
    }
    let record: Record = serde_json::from_slice(&crate::packages::read_bounded(
        &directory.join("record.json"),
        16 * 1024,
    )?)?;
    if record.version != 1 {
        return Err(io::Error::other("Unsupported recovery record."));
    }
    let target = destination(&record.root, &record.relative)?;
    let payload = directory.join("payload");
    if payload.symlink_metadata()?.file_type().is_symlink() {
        return Err(io::Error::other("Invalid recovery payload."));
    }
    rename_exclusive(&payload, &target)?;
    // Keep the metadata if cleanup fails; the restored file is already safe.
    let _ = fs::remove_file(directory.join("record.json"));
    let _ = fs::remove_dir(directory);
    Ok(target)
}

impl Controller {
    pub fn is_open(&self) -> bool {
        self.form.is_some()
    }
    pub fn new_item(&mut self, root: &Path, parent: &Path, folder: bool) {
        let relative = parent.strip_prefix(root).unwrap_or(Path::new(""));
        self.form = Some(Form {
            root: root.to_owned(),
            operation: Operation::Create { folder },
            value: if relative.as_os_str().is_empty() {
                String::new()
            } else {
                format!("{}/", relative.display())
            },
            error: String::new(),
        });
    }
    pub fn menu(
        &mut self,
        ui: &mut egui::Ui,
        root: &Path,
        path: &Path,
        folder: bool,
        root_row: bool,
    ) {
        if folder {
            if ui.button("New file…").clicked() {
                self.new_item(root, path, false);
                ui.close_menu();
            }
            if ui.button("New folder…").clicked() {
                self.new_item(root, path, true);
                ui.close_menu();
            }
        }
        if !root_row {
            if ui.button("Rename / move…").clicked() {
                self.form = Some(Form {
                    root: root.to_owned(),
                    operation: Operation::Rename {
                        source: path.to_owned(),
                    },
                    value: path
                        .strip_prefix(root)
                        .unwrap_or(path)
                        .display()
                        .to_string(),
                    error: String::new(),
                });
                ui.close_menu();
            }
            if ui.button("Move to recovery…").clicked() {
                self.form = Some(Form {
                    root: root.to_owned(),
                    operation: Operation::Remove {
                        source: path.to_owned(),
                    },
                    value: String::new(),
                    error: String::new(),
                });
                ui.close_menu();
            }
        }
        if ui.button("Copy relative path").clicked() {
            ui.ctx().copy_text(
                path.strip_prefix(root)
                    .unwrap_or(path)
                    .display()
                    .to_string(),
            );
            ui.close_menu();
        }
        if ui.button("Copy full path").clicked() {
            ui.ctx().copy_text(path.display().to_string());
            ui.close_menu();
        }
    }
    pub fn recovery_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("Find removed items").clicked() {
            let result = recovery_directory().and_then(|storage| {
                let mut entries = Vec::new();
                for entry in fs::read_dir(storage)?.take(1000) {
                    let path = entry?.path();
                    if path.symlink_metadata()?.file_type().is_symlink()
                        || !path.is_dir()
                        || !path.join("payload").exists()
                    {
                        continue;
                    }
                    let record =
                        crate::packages::read_bounded(&path.join("record.json"), 16 * 1024)
                            .ok()
                            .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok());
                    if let Some(record) = record {
                        entries.push((path, record.root.join(record.relative)));
                    }
                }
                Ok(entries)
            });
            match result {
                Ok(entries) => self.recoveries = entries,
                Err(error) => self.notice = error.to_string(),
            }
        }
        for (record, original) in self.recoveries.clone() {
            if ui
                .button(format!("Restore {}", original.display()))
                .clicked()
            {
                self.form = Some(Form {
                    root: original.parent().unwrap_or(Path::new(".")).to_owned(),
                    operation: Operation::Restore { record },
                    value: original.display().to_string(),
                    error: String::new(),
                });
                ui.close_menu();
            }
        }
    }
    pub fn show(&mut self, context: &egui::Context, tabs: &[OpenFile]) {
        let busy = self.job.is_some();
        let mut apply = false;
        let mut cancel = false;
        if let Some(form) = &mut self.form {
            egui::Modal::new(egui::Id::new("workspace-file-operation")).show(context,|ui| {
                ui.set_width(440.0);
                ui.heading(match form.operation { Operation::Create {folder:true} => "New folder",Operation::Create {folder:false} => "New file",Operation::Rename {..} => "Rename or move",Operation::Remove {..} => "Move to recovery",Operation::Restore {..} => "Restore removed item" });
                match &form.operation {
                    Operation::Create {..} | Operation::Rename {..} => { ui.label("Path relative to this workspace folder"); ui.add_enabled(!busy,TextEdit::singleline(&mut form.value).desired_width(f32::INFINITY)); }
                    Operation::Remove {source} => { ui.label(source.display().to_string()); ui.label("Moves this item into ForgeIDE recovery. You can restore it from the Explorer menu."); }
                    Operation::Restore {..} => { ui.label(&form.value); ui.label("An existing destination will not be replaced."); }
                }
                let blocked = match &form.operation {
                    Operation::Remove {source} => tabs.iter().any(|file| file.path.starts_with(source) && file.is_dirty()),
                    Operation::Create {..} | Operation::Rename {..} => {
                        let target = form.root.join(form.value.trim());
                        tabs.iter().any(|file| file.path.starts_with(&target) && !matches!(&form.operation,Operation::Rename {source} if file.path.starts_with(source)))
                    }
                    _ => false,
                };
                if blocked { ui.label("Save or close the affected open document before this operation."); }
                if !form.error.is_empty() { ui.colored_label(crate::chrome::theme_red(),&form.error); }
                ui.horizontal(|ui| {
                    apply = ui.add_enabled(!busy && !blocked,egui::Button::new("Continue")).clicked();
                    cancel = ui.add_enabled(!busy,egui::Button::new("Cancel")).clicked();
                    if busy { ui.spinner(); }
                });
            });
        }
        if cancel {
            self.form = None;
        }
        if apply {
            let Some(form) = &self.form else {
                return;
            };
            let root = form.root.clone();
            let operation = form.operation.clone();
            let value = form.value.trim().to_owned();
            let (sender, receiver) = mpsc::sync_channel(1);
            let context = context.clone();
            self.job = Some(receiver);
            std::thread::spawn(move || {
                let result = (|| -> io::Result<Outcome> {
                    match operation {
                        Operation::Create { folder } => {
                            let target = destination(&root, Path::new(&value))?;
                            if folder {
                                fs::create_dir(&target)?;
                            } else {
                                fs::OpenOptions::new()
                                    .write(true)
                                    .create_new(true)
                                    .open(&target)?
                                    .sync_all()?;
                            }
                            Ok(Outcome::Created(target.canonicalize()?, folder))
                        }
                        Operation::Rename { source: old } => {
                            let old = source(&root, &old)?;
                            let target = destination(&root, Path::new(&value))?;
                            if target.starts_with(&old) {
                                return Err(io::Error::other(
                                    "An item cannot be moved inside itself.",
                                ));
                            }
                            rename_exclusive(&old, &target)?;
                            Ok(Outcome::Renamed(old, target.canonicalize()?))
                        }
                        Operation::Remove { source: path } => {
                            let storage = recovery_directory()?;
                            remove_recoverably(&root, &path, &storage)?;
                            Ok(Outcome::Removed(path))
                        }
                        Operation::Restore { record } => {
                            restore(&record, &recovery_directory()?).map(Outcome::Restored)
                        }
                    }
                })();
                let _ = sender.send(result);
                context.request_repaint();
            });
        }
        if busy || self.job.is_some() {
            context.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    pub fn poll(&mut self) -> Option<Outcome> {
        let result = self.job.as_ref().and_then(|job| match job.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(io::Error::other(
                "File operation stopped unexpectedly.",
            ))),
            Err(mpsc::TryRecvError::Empty) => None,
        })?;
        self.job = None;
        match result {
            Ok(outcome) => {
                self.form = None;
                self.notice = "Workspace updated.".into();
                Some(outcome)
            }
            Err(error) => {
                if let Some(form) = &mut self.form {
                    form.error = error.to_string();
                }
                self.notice = error.to_string();
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_moves_never_replace_files_or_directories() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        fs::write(&first, "first")?;
        fs::write(&second, "second")?;
        assert!(rename_exclusive(&first, &second).is_err());
        assert_eq!(fs::read_to_string(&first)?, "first");
        assert_eq!(fs::read_to_string(&second)?, "second");
        let folder = directory.path().join("folder");
        fs::create_dir(&folder)?;
        let moved = directory.path().join("moved");
        rename_exclusive(&folder, &moved)?;
        assert!(moved.is_dir() && !folder.exists());
        Ok(())
    }
    #[test]
    fn removal_and_restore_keep_contents_and_refuse_destination_collisions() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("project");
        fs::create_dir(&root)?;
        let storage = directory.path().join("recovery");
        fs::create_dir(&storage)?;
        let path = root.join("notes.txt");
        fs::write(&path, "keep me")?;
        let record = remove_recoverably(&root, &path, &storage)?;
        assert!(!path.exists());
        fs::write(&path, "new content")?;
        assert!(restore(&record, &storage).is_err());
        assert_eq!(fs::read_to_string(&path)?, "new content");
        fs::remove_file(&path)?;
        assert_eq!(
            restore(&record, &storage)?,
            root.canonicalize()?.join("notes.txt")
        );
        assert_eq!(fs::read_to_string(&path)?, "keep me");
        Ok(())
    }
    #[test]
    fn rejects_traversal_git_metadata_and_symlink_escapes() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        for value in ["../escape", ".git/config", "/absolute", ""] {
            assert!(destination(directory.path(), Path::new(value)).is_err());
        }
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir()?;
            std::os::unix::fs::symlink(outside.path(), directory.path().join("link"))?;
            assert!(destination(directory.path(), Path::new("link/escape")).is_err());
        }
        Ok(())
    }
}
