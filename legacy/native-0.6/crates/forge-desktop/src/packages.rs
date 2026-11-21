use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub(crate) const MANIFEST_LIMIT: u64 = 256 * 1024;
pub(crate) const MODULE_LIMIT: u64 = 256 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snippet {
    pub prefix: String,
    pub extensions: Vec<String>,
    pub body: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub name: String,
    pub dark: bool,
    pub colors: BTreeMap<String, [u8; 3]>,
    #[serde(default)]
    pub syntax: BTreeMap<String, [u8; 3]>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionCommand {
    pub id: String,
    pub title: String,
    pub module: PathBuf,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Metadata {
    pub publisher: String,
    pub description: String,
    pub license: String,
    pub readme: String,
    pub changelog: String,
    pub categories: Vec<String>,
    pub source: Option<String>,
    pub sha256: String,
    pub notes: Vec<String>,
    pub javascript: bool,
    pub imported: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
    #[serde(default)]
    pub themes: Vec<Theme>,
    #[serde(default)]
    pub transform: Option<PathBuf>,
    #[serde(default)]
    pub commands: Vec<ExtensionCommand>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub extension_pack: Vec<String>,
    #[serde(default)]
    pub engine: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub publisher: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Bundle {
    pub manifest: Manifest,
    pub enabled: bool,
    pub module: Option<Vec<u8>>,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(default)]
    pub command_modules: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub disabled_workspaces: Vec<PathBuf>,
}

pub(crate) struct Prepared {
    pub bundle: Bundle,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortablePackage {
    schema_version: u32,
    bundle: Bundle,
}

pub(crate) fn prepare_file(path: &Path) -> io::Result<Prepared> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("vsix") => crate::extension_vsix::prepare(read_bounded(
            path,
            crate::extension_registry::DOWNLOAD_LIMIT,
        )?),
        Some("forge-extension") => {
            let bytes = read_bounded(path, 32 * 1024 * 1024)?;
            let mut package: PortablePackage = serde_json::from_slice(&bytes)?;
            package.bundle.metadata.sha256 = crate::extension_registry::digest(&bytes);
            if package.schema_version != 1 {
                return Err(io::Error::other("Unsupported portable package version."));
            }
            validate_bundle(&package.bundle)?;
            package.bundle.enabled = true;
            package.bundle.pinned = false;
            package.bundle.disabled_workspaces.clear();
            // A local file cannot opt into registry updates.
            package.bundle.metadata.source = None;
            Ok(Prepared {
                bundle: package.bundle,
            })
        }
        _ => Library::prepare_native(path),
    }
}

pub(crate) fn package_native(source: &Path, output: &Path) -> io::Result<()> {
    let Prepared { bundle } = Library::prepare_native(source)?;
    let bytes = serde_json::to_vec_pretty(&PortablePackage {
        schema_version: 1,
        bundle,
    })?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(io::Error::other("Portable package exceeds 32 MiB."));
    }
    if output.exists() {
        return Err(io::Error::other(
            "The output package already exists. Choose another path.",
        ));
    }
    let mut temporary = tempfile::NamedTempFile::new_in(
        output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(output)
        .map_err(|error| error.error)?;
    Ok(())
}

pub(crate) fn inspect_file(path: &Path) -> io::Result<serde_json::Value> {
    let Prepared { bundle } = prepare_file(path)?;
    Ok(serde_json::json!({
        "id": bundle.manifest.id, "name":bundle.manifest.name, "version":bundle.manifest.version,
        "themes":bundle.manifest.themes.iter().map(|theme| &theme.name).collect::<Vec<_>>(),
        "snippets":bundle.manifest.snippets.len(),
        "commands":bundle.manifest.commands,
        "extension_pack":bundle.manifest.extension_pack,
        "javascript_required":bundle.metadata.javascript,
        "notes":bundle.metadata.notes,
        "sha256":bundle.metadata.sha256,
        "permissions":if bundle.module.is_some() || !bundle.command_modules.is_empty() { vec!["active-document-on-explicit-command"] } else { vec![] }
    }))
}

#[derive(Default)]
pub struct Library {
    pub(crate) installed: Vec<Bundle>,
    pub(crate) notice: String,
    pub(crate) manager: crate::extension_manager::Manager,
    workspace: Option<PathBuf>,
    storage: Option<PathBuf>,
}

pub enum Action {
    Theme(Theme),
    Transform(String, Vec<u8>),
    RunCommand(String),
}

pub(crate) fn directory() -> io::Result<PathBuf> {
    let directory = crate::recovery::directory()?
        .parent()
        .expect("data directory")
        .join("Extensions");
    std::fs::create_dir_all(&directory)?;
    Ok(directory)
}

pub(crate) fn read_bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > limit {
        return Err(io::Error::other("Extension file exceeds its size limit."));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::other("Extension file grew past its size limit."));
    }
    Ok(bytes)
}

pub(crate) fn validate(manifest: &Manifest) -> io::Result<()> {
    if manifest.schema_version != 1
        || !valid_id(&manifest.id)
        || manifest.name.trim().is_empty()
        || manifest.name.len() > 120
    {
        return Err(io::Error::other(
            "Use schema version 1, a lowercase extension ID and a display name.",
        ));
    }
    semver::Version::parse(&manifest.version).map_err(io::Error::other)?;
    if let Some(engine) = &manifest.engine {
        let requirement = semver::VersionReq::parse(engine).map_err(io::Error::other)?;
        if !requirement.matches(&semver::Version::new(1, 0, 0)) {
            return Err(io::Error::other(
                "The extension requires another Forge extension API version.",
            ));
        }
    }
    if manifest.commands.len() > 100
        || manifest.dependencies.len() > 30
        || manifest.extension_pack.len() > 100
    {
        return Err(io::Error::other(
            "Too many commands, dependencies or extension pack entries.",
        ));
    }
    for dependency in manifest.dependencies.iter().chain(&manifest.extension_pack) {
        if dependency == &manifest.id || !valid_id(dependency) {
            return Err(io::Error::other("Invalid dependency ID."));
        }
    }
    let mut commands = std::collections::HashSet::new();
    for command in &manifest.commands {
        if !valid_id(&command.id)
            || command.title.trim().is_empty()
            || command.title.len() > 120
            || !commands.insert(&command.id)
        {
            return Err(io::Error::other(
                "Command IDs must be unique and titles must be nonempty.",
            ));
        }
    }
    if manifest.snippets.len() > 100 || manifest.themes.len() > 20 {
        return Err(io::Error::other(
            "A package supports up to 100 snippets and 20 themes.",
        ));
    }
    for snippet in &manifest.snippets {
        if snippet.prefix.is_empty()
            || snippet.prefix.len() > 64
            || !snippet
                .prefix
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
            || snippet.extensions.is_empty()
            || snippet.extensions.len() > 20
            || snippet.extensions.iter().any(|extension| {
                extension.is_empty()
                    || extension.len() > 16
                    || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
            || snippet.body.len() > 8192
        {
            return Err(io::Error::other(
                "Snippet prefixes, extensions or body are invalid.",
            ));
        }
        crate::snippets::expand(&snippet.body)?;
    }
    for theme in &manifest.themes {
        const KEYS: &[&str] = &[
            "rail", "panel", "editor", "elevated", "border", "text", "muted", "dim", "accent",
            "active", "green", "red",
        ];
        if theme.syntax.keys().any(|key| {
            ![
                "comment", "string", "constant", "keyword", "function", "type", "property",
                "operator",
            ]
            .contains(&key.as_str())
        }) {
            return Err(io::Error::other("Unsupported syntax color category."));
        }
        if theme.name.trim().is_empty()
            || theme.name.len() > 100
            || theme.colors.keys().any(|key| !KEYS.contains(&key.as_str()))
        {
            return Err(io::Error::other("Theme name or palette keys are invalid."));
        }
    }
    Ok(())
}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}

impl Library {
    pub fn load() -> Self {
        match directory().and_then(|directory| Self::load_at(&directory)) {
            Ok(mut library) => {
                library.manager.load_preferences();
                library
            }
            Err(error) => Self {
                notice: error.to_string(),
                ..Self::default()
            },
        }
    }

    fn load_at(directory: &Path) -> io::Result<Self> {
        let mut library = Self {
            storage: Some(directory.to_owned()),
            ..Self::default()
        };
        for entry in std::fs::read_dir(directory)?.take(1000) {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let result = (|| -> io::Result<Bundle> {
                let bundle: Bundle =
                    serde_json::from_slice(&read_bounded(&path, 32 * 1024 * 1024)?)?;
                validate(&bundle.manifest)?;
                if path
                    .file_stem()
                    .is_none_or(|name| name != bundle.manifest.id.as_str())
                {
                    return Err(io::Error::other(
                        "Installed extension ID does not match its file.",
                    ));
                }
                validate_bundle(&bundle)?;
                Ok(bundle)
            })();
            match result {
                Ok(bundle) if library.installed.len() < 100 => library.installed.push(bundle),
                Ok(_) => library.notice = "Only the first 100 valid extensions are loaded.".into(),
                Err(error) => library.notice = format!("{} — {error}", path.display()),
            }
        }
        library
            .installed
            .sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));
        Ok(library)
    }

    pub(crate) fn prepare_native(source: &Path) -> io::Result<Prepared> {
        let source = source.canonicalize()?;
        let manifest: Manifest = serde_json::from_slice(&read_bounded(&source, MANIFEST_LIMIT)?)?;
        validate(&manifest)?;
        let parent = source.parent().expect("manifest parent");
        let module = manifest
            .transform
            .as_deref()
            .map(|module| load_module(parent, module))
            .transpose()?;
        let mut command_modules = BTreeMap::new();
        for command in &manifest.commands {
            command_modules.insert(command.id.clone(), load_module(parent, &command.module)?);
        }
        validate_module(module.as_deref())?;
        let bundle = Bundle {
            manifest: manifest.clone(),
            module,
            command_modules,
            enabled: true,
            pinned: false,
            disabled_workspaces: vec![],
            metadata: Metadata {
                description: manifest.description.clone(),
                license: manifest.license.clone(),
                publisher: manifest.publisher.clone(),
                readme: read_bounded(&parent.join("README.md"), 256 * 1024)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .unwrap_or_default(),
                sha256: crate::extension_registry::digest(&read_bounded(&source, MANIFEST_LIMIT)?),
                ..Metadata::default()
            },
        };
        validate_bundle(&bundle)?;
        Ok(Prepared { bundle })
    }

    #[cfg(test)]
    fn install_at(&mut self, source: &Path, directory: &Path) -> io::Result<()> {
        self.install_prepared_at(Self::prepare_native(source)?, directory)
    }

    pub(crate) fn install_prepared_at(
        &mut self,
        prepared: Prepared,
        directory: &Path,
    ) -> io::Result<()> {
        let mut bundle = prepared.bundle;
        validate_bundle(&bundle)?;
        let existing = self
            .installed
            .iter()
            .position(|item| item.manifest.id == bundle.manifest.id);
        if existing.is_none() && self.installed.len() >= 100 {
            return Err(io::Error::other("Up to 100 extensions can be installed."));
        }
        let _lock = extension_lock(directory, &bundle.manifest.id)?;
        let path = directory.join(format!("{}.json", bundle.manifest.id));
        let current = match read_bounded(&path, 32 * 1024 * 1024) {
            Ok(bytes) => Some((serde_json::from_slice::<Bundle>(&bytes)?, bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        if current
            .as_ref()
            .map(|(item, _)| serde_json::to_value(item))
            .transpose()?
            != existing
                .map(|index| serde_json::to_value(&self.installed[index]))
                .transpose()?
        {
            return Err(io::Error::other(
                "The extension changed in another window. Reload installed extensions before updating.",
            ));
        }
        let previous_bytes = current.map(|(previous, bytes)| {
            bundle.enabled = previous.enabled;
            bundle.pinned = previous.pinned;
            bundle.disabled_workspaces = previous.disabled_workspaces;
            bytes
        });
        let bytes = serde_json::to_vec(&bundle)?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err(io::Error::other("Installed package exceeds 32 MiB."));
        }
        if let Some(previous_bytes) = previous_bytes {
            atomic_write(
                &directory.join(format!("{}.previous", bundle.manifest.id)),
                &previous_bytes,
            )?;
        }
        atomic_write(&path, &bytes)?;
        self.storage = Some(directory.to_owned());
        if let Some(index) = existing {
            self.installed[index] = bundle;
        } else {
            self.installed.push(bundle);
        }
        self.installed
            .sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));
        Ok(())
    }

    pub(crate) fn set_enabled(
        &mut self,
        id: &str,
        enabled: bool,
        workspace: Option<&Path>,
    ) -> io::Result<()> {
        let index = self
            .installed
            .iter()
            .position(|item| item.manifest.id == id)
            .ok_or_else(|| io::Error::other("Extension is missing."))?;
        let mut next = self.installed[index].clone();
        if let Some(workspace) = workspace {
            next.disabled_workspaces.retain(|root| root != workspace);
            if !enabled {
                next.disabled_workspaces.push(workspace.to_owned());
            }
        } else {
            next.enabled = enabled;
        }
        persist(&self.installed[index], &next, &self.storage_directory()?)?;
        self.installed[index] = next;
        Ok(())
    }

    pub(crate) fn pin(&mut self, id: &str, pinned: bool) -> io::Result<()> {
        let index = self
            .installed
            .iter()
            .position(|item| item.manifest.id == id)
            .ok_or_else(|| io::Error::other("Extension is missing."))?;
        let mut next = self.installed[index].clone();
        next.pinned = pinned;
        persist(&self.installed[index], &next, &self.storage_directory()?)?;
        self.installed[index] = next;
        Ok(())
    }

    fn storage_directory(&self) -> io::Result<PathBuf> {
        self.storage.clone().map_or_else(directory, Ok)
    }

    pub(crate) fn rollback(&mut self, id: &str) -> io::Result<()> {
        if !valid_id(id) {
            return Err(io::Error::other("Invalid extension ID."));
        }
        let directory = self.storage_directory()?;
        let bundle: Bundle = serde_json::from_slice(&read_bounded(
            &directory.join(format!("{id}.previous")),
            32 * 1024 * 1024,
        )?)?;
        if bundle.manifest.id != id {
            return Err(io::Error::other("Rollback identifier does not match."));
        }
        self.install_prepared_at(Prepared { bundle }, &directory)
    }

    pub(crate) fn uninstall(&mut self, id: &str) -> io::Result<()> {
        if !valid_id(id) {
            return Err(io::Error::other("Invalid extension ID."));
        }
        let directory = self.storage_directory()?;
        let _lock = extension_lock(&directory, id)?;
        let path = directory.join(format!("{id}.json"));
        let bytes = read_bounded(&path, 32 * 1024 * 1024)?;
        let disk: Bundle = serde_json::from_slice(&bytes)?;
        let expected = self
            .installed
            .iter()
            .find(|bundle| bundle.manifest.id == id)
            .ok_or_else(|| io::Error::other("Extension is missing. Reload first."))?;
        if serde_json::to_value(disk)? != serde_json::to_value(expected)? {
            return Err(io::Error::other(
                "Another window changed this extension. Reload before uninstalling.",
            ));
        }
        atomic_write(&directory.join(format!("{id}.removed")), &bytes)?;
        std::fs::remove_file(path)?;
        self.installed.retain(|item| item.manifest.id != id);
        Ok(())
    }

    pub(crate) fn reload(&mut self) -> io::Result<()> {
        let library = Self::load_at(&self.storage_directory()?)?;
        self.installed = library.installed;
        self.notice = library.notice;
        Ok(())
    }

    pub fn set_workspace(&mut self, workspace: Option<PathBuf>) {
        self.workspace = workspace;
    }
    pub(crate) fn workspace(&self) -> Option<&Path> {
        self.workspace.as_deref()
    }

    fn available(&self, bundle: &Bundle) -> bool {
        self.is_available_at(&bundle.manifest.id, self.workspace.as_deref())
    }

    fn available_at(
        &self,
        id: &str,
        root: Option<&Path>,
        visiting: &mut Vec<String>,
        cache: &mut BTreeMap<String, bool>,
    ) -> bool {
        if let Some(available) = cache.get(id) {
            return *available;
        }
        if !self.is_enabled_at(id, root)
            || visiting.iter().any(|item| item == id)
            || visiting.len() >= 100
        {
            return false;
        }
        let Some(bundle) = self.installed.iter().find(|item| item.manifest.id == id) else {
            return false;
        };
        visiting.push(id.into());
        let available = bundle
            .manifest
            .dependencies
            .iter()
            .all(|dependency| self.available_at(dependency, root, visiting, cache));
        visiting.pop();
        cache.insert(id.to_owned(), available);
        available
    }

    pub fn is_available_at(&self, id: &str, root: Option<&Path>) -> bool {
        self.available_at(id, root, &mut Vec::new(), &mut BTreeMap::new())
    }

    pub(crate) fn removed(&self) -> io::Result<Vec<String>> {
        let mut result = Vec::new();
        for entry in std::fs::read_dir(self.storage_directory()?)?.take(1000) {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "removed")
            {
                if let Some(id) = path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .filter(|id| valid_id(id))
                {
                    if !self.installed.iter().any(|item| item.manifest.id == id) {
                        result.push(id.to_owned());
                    }
                }
            }
        }
        result.sort();
        Ok(result)
    }

    pub(crate) fn restore_removed(&mut self, id: &str) -> io::Result<()> {
        if !valid_id(id) {
            return Err(io::Error::other("Invalid extension ID."));
        }
        let directory = self.storage_directory()?;
        let bundle: Bundle = serde_json::from_slice(&read_bounded(
            &directory.join(format!("{id}.removed")),
            32 * 1024 * 1024,
        )?)?;
        if bundle.manifest.id != id {
            return Err(io::Error::other("Restore identifier does not match."));
        }
        self.install_prepared_at(Prepared { bundle }, &directory)
    }

    pub fn update_count(&self) -> usize {
        self.installed
            .iter()
            .filter(|bundle| !bundle.pinned && self.manager.has_update(&bundle.manifest.id))
            .count()
    }

    pub fn commands(&self) -> Vec<(String, String)> {
        self.installed
            .iter()
            .filter(|bundle| self.available(bundle))
            .flat_map(|bundle| {
                bundle.manifest.commands.iter().map(|command| {
                    (
                        format!("{}/{}", bundle.manifest.id, command.id),
                        command.title.clone(),
                    )
                })
            })
            .collect()
    }

    pub fn command(&self, id: &str) -> Option<(String, Vec<u8>)> {
        let (extension, command) = id.split_once('/')?;
        let bundle = self
            .installed
            .iter()
            .find(|bundle| bundle.manifest.id == extension && self.available(bundle))?;
        Some((
            extension.into(),
            bundle.command_modules.get(command)?.clone(),
        ))
    }

    pub fn snippet(&self, extension: &str, prefix: &str) -> Option<&str> {
        self.installed
            .iter()
            .filter(|bundle| self.available(bundle))
            .flat_map(|bundle| &bundle.manifest.snippets)
            .find(|snippet| {
                snippet.prefix == prefix && snippet.extensions.iter().any(|item| item == extension)
            })
            .map(|snippet| snippet.body.as_str())
    }

    pub fn themes(&self) -> impl Iterator<Item = (&str, &Theme)> {
        self.installed
            .iter()
            .filter(|bundle| self.available(bundle))
            .flat_map(|bundle| {
                bundle
                    .manifest
                    .themes
                    .iter()
                    .map(|theme| (bundle.manifest.id.as_str(), theme))
            })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut manager = std::mem::take(&mut self.manager);
        let action = manager.show(self, ui);
        self.manager = manager;
        action
    }

    pub fn poll(&mut self, context: &egui::Context) {
        let mut manager = std::mem::take(&mut self.manager);
        manager.poll(self, context);
        self.manager = manager;
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.is_enabled_at(id, self.workspace.as_deref())
    }
    pub fn is_enabled_at(&self, id: &str, workspace: Option<&Path>) -> bool {
        self.installed.iter().any(|bundle| {
            bundle.manifest.id == id
                && bundle.enabled
                && workspace.is_none_or(|root| {
                    !bundle
                        .disabled_workspaces
                        .iter()
                        .any(|disabled| disabled == root)
                })
        })
    }
}

fn load_module(parent: &Path, module: &Path) -> io::Result<Vec<u8>> {
    if module.is_absolute() {
        return Err(io::Error::other(
            "Transform paths must be relative to the extension folder.",
        ));
    }
    let module = parent.join(module).canonicalize()?;
    if !module.starts_with(parent) {
        return Err(io::Error::other(
            "A transform must stay inside its extension folder.",
        ));
    }
    let bytes = read_bounded(&module, MODULE_LIMIT)?;
    validate_module(Some(&bytes))?;
    Ok(bytes)
}

pub(crate) fn validate_bundle(bundle: &Bundle) -> io::Result<()> {
    validate(&bundle.manifest)?;
    if bundle.manifest.transform.is_some() != bundle.module.is_some() {
        return Err(io::Error::other(
            "Transform declaration does not match its packaged module.",
        ));
    }
    if bundle
        .metadata
        .source
        .as_ref()
        .is_some_and(|id| !valid_id(id) || id != &bundle.manifest.id || !bundle.metadata.imported)
    {
        return Err(io::Error::other("Invalid extension registry source."));
    }
    validate_module(bundle.module.as_deref())?;
    if bundle.command_modules.len() != bundle.manifest.commands.len() {
        return Err(io::Error::other("A command module is missing."));
    }
    for command in &bundle.manifest.commands {
        validate_module(Some(
            bundle
                .command_modules
                .get(&command.id)
                .ok_or_else(|| io::Error::other("Missing command module."))?,
        ))?;
    }
    Ok(())
}

fn extension_lock(directory: &Path, id: &str) -> io::Result<File> {
    use fs2::FileExt;
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(format!("{id}.lock")))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(200);
    loop {
        match lock.try_lock_exclusive() {
            Ok(()) => break,
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(5))
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Err(io::Error::other(
                    "This extension is being changed in another window. Retry after it finishes.",
                ));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(lock)
}

fn persist(previous: &Bundle, bundle: &Bundle, directory: &Path) -> io::Result<()> {
    let _lock = extension_lock(directory, &bundle.manifest.id)?;
    let path = directory.join(format!("{}.json", bundle.manifest.id));
    let disk: Bundle = serde_json::from_slice(&read_bounded(&path, 32 * 1024 * 1024)?)?;
    if serde_json::to_value(&disk)? != serde_json::to_value(previous)? {
        return Err(io::Error::other(
            "Another window changed this extension. Reload first.",
        ));
    }
    let bytes = serde_json::to_vec(bundle)?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(io::Error::other("Installed extension exceeds 32 MiB."));
    }
    atomic_write(&path, &bytes)
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temporary =
        tempfile::NamedTempFile::new_in(path.parent().expect("extension directory"))?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn validate_module(module: Option<&[u8]>) -> io::Result<()> {
    if let Some(bytes) = module {
        if bytes.len() as u64 > MODULE_LIMIT {
            return Err(io::Error::other("Transform exceeds 256 KiB."));
        }
        let module =
            wasmi::Module::new(&wasmi::Engine::default(), bytes).map_err(io::Error::other)?;
        if module.imports().next().is_some() {
            return Err(io::Error::other("Extension imports are disabled."));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn package_update_is_atomic_and_enabled_state_survives_reload() -> io::Result<()> {
        let source = tempfile::tempdir()?;
        let installed = tempfile::tempdir()?;
        let manifest = source.path().join("forge-extension.json");
        let mut value = json!({
            "schema_version": 1, "id": "studio.toolbox", "name": "Toolbox", "version": "1.0.0",
            "snippets": [{"prefix": "hello", "extensions": ["rs"], "body": concat!("fn $", "{1:name}() {\n    $", "{0}\n}")}],
            "themes": [{"name": "Midnight", "dark": true, "colors": {"editor": [12, 18, 30]}}]
        });
        std::fs::write(&manifest, serde_json::to_vec(&value)?)?;
        let mut library = Library::default();
        library.install_at(&manifest, installed.path())?;
        assert!(library.snippet("rs", "hello").is_some());
        let saved = std::fs::read(installed.path().join("studio.toolbox.json"))?;
        value["version"] = json!("1.1.0");
        value["snippets"][0]["body"] = json!(concat!("$", "{unsupported}"));
        std::fs::write(&manifest, serde_json::to_vec(&value)?)?;
        assert!(library.install_at(&manifest, installed.path()).is_err());
        assert_eq!(
            std::fs::read(installed.path().join("studio.toolbox.json"))?,
            saved
        );
        value["snippets"][0]["body"] = json!(concat!("updated $", "{1:value}"));
        std::fs::write(&manifest, serde_json::to_vec(&value)?)?;
        library.install_at(&manifest, installed.path())?;
        let mut library = Library::load_at(installed.path())?;
        assert_eq!(
            library.snippet("rs", "hello"),
            Some(concat!("updated $", "{1:value}"))
        );
        let theme = library.themes().next().expect("installed theme").1.clone();
        let context = egui::Context::default();
        crate::chrome::set_theme(Some(theme));
        crate::chrome::configure(&context, false, 15.0, false, 1.0);
        assert_eq!(
            crate::chrome::theme_editor(),
            egui::Color32::from_rgb(12, 18, 30)
        );
        crate::chrome::set_theme(None);
        crate::chrome::configure(&context, false, 15.0, false, 1.0);
        library.set_enabled("studio.toolbox", false, None)?;
        let library = Library::load_at(installed.path())?;
        assert!(!library.is_enabled("studio.toolbox"));
        assert!(library.snippet("rs", "hello").is_none());
        Ok(())
    }

    #[test]
    fn package_rejects_transform_path_escape() -> io::Result<()> {
        let source = tempfile::tempdir()?;
        let package = source.path().join("package");
        std::fs::create_dir(&package)?;
        let installed = tempfile::tempdir()?;
        let bytes =
            wat::parse_str(r#"(module (memory (export "memory") 1))"#).map_err(io::Error::other)?;
        std::fs::write(source.path().join("outside.wasm"), bytes)?;
        let manifest = package.join("forge-extension.json");
        std::fs::write(
            &manifest,
            serde_json::to_vec(&json!({
                "schema_version": 1, "id": "studio.escape", "name": "Escape", "version": "1.0.0",
                "transform": "../outside.wasm"
            }))?,
        )?;
        assert!(
            Library::default()
                .install_at(&manifest, installed.path())
                .is_err()
        );
        assert_eq!(std::fs::read_dir(installed.path())?.count(), 0);
        Ok(())
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use serde_json::json;

    fn prepared(id: &str, version: &str) -> Prepared {
        Prepared {
            bundle: serde_json::from_value(json!({
                "manifest":{"schema_version":1,"id":id,"name":"Lifecycle","version":version,
                    "snippets":[{"prefix":"hello","extensions":["rs"],"body":"hello"}]},
                "enabled":true,"module":null
            }))
            .unwrap(),
        }
    }

    #[test]
    fn updates_preserve_scope_and_pin_and_rollback_and_uninstall_are_recoverable() -> io::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let mut library = Library::default();
        library.install_prepared_at(prepared("studio.life", "1.0.0"), directory.path())?;
        let root = directory.path().join("project");
        library.set_workspace(Some(root.clone()));
        library.set_enabled("studio.life", false, Some(&root))?;
        library.pin("studio.life", true)?;
        assert!(library.snippet("rs", "hello").is_none());
        library.install_prepared_at(prepared("studio.life", "2.0.0"), directory.path())?;
        assert!(library.installed[0].pinned);
        assert!(library.snippet("rs", "hello").is_none());
        library.rollback("studio.life")?;
        assert_eq!(library.installed[0].manifest.version, "1.0.0");
        library.set_enabled("studio.life", true, Some(&root))?;
        assert_eq!(library.snippet("rs", "hello"), Some("hello"));
        library.uninstall("studio.life")?;
        assert!(library.installed.is_empty());
        assert_eq!(library.removed()?, vec!["studio.life"]);
        library.restore_removed("studio.life")?;
        assert!(library.installed[0].pinned);
        assert_eq!(
            Library::load_at(directory.path())?.installed[0]
                .manifest
                .version,
            "1.0.0"
        );
        Ok(())
    }

    #[test]
    fn failed_update_keeps_current_and_previous_versions_unchanged() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let mut library = Library::default();
        library.install_prepared_at(prepared("studio.life", "1.0.0"), directory.path())?;
        library.install_prepared_at(prepared("studio.life", "2.0.0"), directory.path())?;
        let current = std::fs::read(directory.path().join("studio.life.json"))?;
        let previous = std::fs::read(directory.path().join("studio.life.previous"))?;
        let mut oversized = prepared("studio.life", "3.0.0");
        oversized.bundle.metadata.readme = "x".repeat(32 * 1024 * 1024);
        assert!(
            library
                .install_prepared_at(oversized, directory.path())
                .is_err()
        );
        assert_eq!(
            std::fs::read(directory.path().join("studio.life.json"))?,
            current
        );
        assert_eq!(
            std::fs::read(directory.path().join("studio.life.previous"))?,
            previous
        );
        assert_eq!(library.installed[0].manifest.version, "2.0.0");
        Ok(())
    }

    #[test]
    fn stale_windows_cannot_overwrite_same_version_settings_or_updates() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let mut first = Library::default();
        first.install_prepared_at(prepared("studio.life", "1.0.0"), directory.path())?;
        let mut stale = Library::load_at(directory.path())?;
        first.pin("studio.life", true)?;
        assert!(stale.set_enabled("studio.life", false, None).is_err());
        assert!(stale.uninstall("studio.life").is_err());
        assert!(
            stale
                .install_prepared_at(prepared("studio.life", "2.0.0"), directory.path())
                .is_err()
        );
        let loaded = Library::load_at(directory.path())?;
        assert!(loaded.installed[0].pinned && loaded.installed[0].enabled);
        assert_eq!(loaded.installed[0].manifest.version, "1.0.0");
        Ok(())
    }

    #[test]
    fn transitive_dependencies_and_cycles_do_not_activate_contributions() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let mut library = Library::default();
        let mut first = prepared("studio.first", "1.0.0");
        first.bundle.manifest.dependencies = vec!["studio.second".into()];
        library.install_prepared_at(first, directory.path())?;
        assert!(library.snippet("rs", "hello").is_none());
        let mut second = prepared("studio.second", "1.0.0");
        second.bundle.manifest.dependencies = vec!["studio.third".into()];
        library.install_prepared_at(second, directory.path())?;
        assert!(!library.is_available_at("studio.first", None));
        library.install_prepared_at(prepared("studio.third", "1.0.0"), directory.path())?;
        assert!(library.is_available_at("studio.first", None));
        let mut third = prepared("studio.third", "2.0.0");
        third.bundle.manifest.dependencies = vec!["studio.first".into()];
        library.install_prepared_at(third, directory.path())?;
        assert!(!library.is_available_at("studio.first", None));
        Ok(())
    }

    #[test]
    fn native_commands_are_registered_and_scoped_without_running_on_install() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let source = tempfile::tempdir()?;
        std::fs::write(
            source.path().join("command.wasm"),
            wat::parse_str(include_str!(
                "../../../fixtures/extensions/uppercase-first.wat"
            ))
            .map_err(io::Error::other)?,
        )?;
        let path = source.path().join("forge-extension.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({
                "schema_version":1,"id":"studio.command","name":"Native command","version":"1.0.0","engine":"^1.0",
                "description":"Native command fixture","license":"MIT",
                "commands":[{"id":"uppercase","title":"Uppercase first letter","module":"command.wasm"}]
            }))?,
        )?;
        let mut library = Library::default();
        library.install_at(&path, directory.path())?;
        assert_eq!(
            library.commands(),
            vec![(
                "studio.command/uppercase".into(),
                "Uppercase first letter".into()
            )]
        );
        assert!(library.command("studio.command/uppercase").is_some());
        library.set_enabled("studio.command", false, None)?;
        assert!(
            library.commands().is_empty() && library.command("studio.command/uppercase").is_none()
        );
        Ok(())
    }
}
