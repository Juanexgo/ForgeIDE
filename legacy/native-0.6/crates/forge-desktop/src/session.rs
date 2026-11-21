use crate::recovery;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

const LIMIT: u64 = 1024 * 1024;

#[derive(Default, Serialize, Deserialize)]
pub struct WorkspaceSession {
    pub root: Option<PathBuf>,
    #[serde(default)]
    pub extra_roots: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
    pub active: Option<PathBuf>,
    #[serde(default)]
    pub split: Option<SplitSession>,
}

#[derive(Serialize, Deserialize)]
pub struct SplitSession {
    pub companion: PathBuf,
    pub active_right: bool,
    pub ratio: f32,
}

pub fn load() -> io::Result<WorkspaceSession> {
    let path = session_path()?;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(WorkspaceSession::default());
        }
        Err(error) => return Err(error),
    };
    deserialize(file)
}

pub fn save(session: &WorkspaceSession) -> io::Result<()> {
    let path = session_path()?;
    write_at(session, &path)
}

fn write_at(session: &WorkspaceSession, path: &Path) -> io::Result<()> {
    let data = serde_json::to_vec(session)?;
    if data.len() as u64 > LIMIT {
        return Err(io::Error::other("Workspace session exceeds 1 MiB."));
    }
    let parent = path.parent().expect("session path has a parent");
    let mut temp = NamedTempFile::new_in(parent)?;
    io::Write::write_all(&mut temp, &data)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn deserialize(file: File) -> io::Result<WorkspaceSession> {
    if file.metadata()?.len() > LIMIT {
        return Err(io::Error::other("Workspace session exceeds 1 MiB."));
    }
    let state: WorkspaceSession = serde_json::from_reader(file.take(LIMIT))?;
    if state.files.len() > 500 || state.extra_roots.len() > 15 {
        return Err(io::Error::other(
            "Workspace session contains too many tabs.",
        ));
    }
    Ok(state)
}

fn session_path() -> io::Result<PathBuf> {
    let recovery = recovery::directory()?;
    Ok(recovery
        .parent()
        .expect("ForgeIDE data directory")
        .join("session.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_workspace_tabs_and_active_file() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("session.json");
        let file = dir.path().join("file with space.rs");
        let second = dir.path().join("companion.rs");
        let state = WorkspaceSession {
            root: Some(dir.path().to_owned()),
            extra_roots: vec![dir.path().join("second-root")],
            files: vec![file.clone(), second.clone()],
            active: Some(file),
            split: Some(SplitSession {
                companion: second.clone(),
                active_right: true,
                ratio: 0.6,
            }),
        };
        write_at(&state, &path)?;
        let read = deserialize(File::open(path)?)?;
        assert_eq!(read.root, state.root);
        assert_eq!(read.extra_roots, state.extra_roots);
        assert_eq!(read.files, state.files);
        assert_eq!(read.active, state.active);
        let split = read.split.expect("split restored");
        assert_eq!(split.companion, second);
        assert!(split.active_right);
        assert_eq!(split.ratio, 0.6);
        Ok(())
    }
}
