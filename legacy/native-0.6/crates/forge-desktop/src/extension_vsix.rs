use crate::packages::{Bundle, Manifest, Metadata, Prepared, Snippet, Theme};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    io::{self, Cursor, Read},
    path::{Component, Path},
};

const EXPANDED_LIMIT: u64 = 64 * 1024 * 1024;

struct Archive {
    zip: zip::ZipArchive<Cursor<Vec<u8>>>,
}

impl Archive {
    fn new(bytes: Vec<u8>) -> io::Result<Self> {
        if bytes.len() as u64 > crate::extension_registry::DOWNLOAD_LIMIT {
            return Err(io::Error::other("VSIX exceeds 32 MiB."));
        }
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(io::Error::other)?;
        if zip.len() > 5000 {
            return Err(io::Error::other("VSIX contains too many entries."));
        }
        let mut names = HashSet::new();
        let mut total = 0u64;
        for index in 0..zip.len() {
            let file = zip.by_index(index).map_err(io::Error::other)?;
            checked_path(file.name())?;
            if !names.insert(file.name().to_owned())
                || file
                    .unix_mode()
                    .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                return Err(io::Error::other(
                    "VSIX contains duplicate entries or symbolic links.",
                ));
            }
            total = total
                .checked_add(file.size())
                .ok_or_else(|| io::Error::other("Invalid VSIX size."))?;
            if total > EXPANDED_LIMIT {
                return Err(io::Error::other("Expanded VSIX exceeds 64 MiB."));
            }
        }
        Ok(Self { zip })
    }

    fn read(&mut self, path: &str, limit: u64) -> io::Result<Vec<u8>> {
        checked_path(path)?;
        let file = self.zip.by_name(path).map_err(io::Error::other)?;
        if file.is_dir() || file.size() > limit {
            return Err(io::Error::other("Contribution file exceeds its limit."));
        }
        let mut bytes = Vec::new();
        file.take(limit + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(io::Error::other("Contribution grew past its size limit."));
        }
        Ok(bytes)
    }

    fn json(&mut self, path: &str) -> io::Result<Value> {
        jsonc(&self.read(path, 2 * 1024 * 1024)?)
    }

    fn optional_text(&mut self, paths: &[&str]) -> String {
        paths
            .iter()
            .find_map(|path| {
                self.read(path, 256 * 1024)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
            })
            .unwrap_or_default()
    }
}

fn checked_path(path: &str) -> io::Result<()> {
    if path.is_empty()
        || path.contains(['\\', ':', '\0'])
        || Path::new(path)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(io::Error::other("VSIX contains an unsafe entry path."));
    }
    Ok(())
}

fn contribution_path(path: &str) -> io::Result<String> {
    let path = path.strip_prefix("./").unwrap_or(path);
    checked_path(path)?;
    Ok(format!("extension/{path}"))
}

pub fn jsonc(bytes: &[u8]) -> io::Result<Value> {
    let mut data = bytes
        .strip_prefix(&[0xef, 0xbb, 0xbf])
        .unwrap_or(bytes)
        .to_vec();
    let mut index = 0;
    let mut string = false;
    while index < data.len() {
        match data[index] {
            b'\\' if string => index += 2,
            b'"' => {
                string = !string;
                index += 1;
            }
            b'/' if !string && data.get(index + 1) == Some(&b'/') => {
                while index < data.len() && data[index] != b'\n' {
                    data[index] = b' ';
                    index += 1;
                }
            }
            b'/' if !string && data.get(index + 1) == Some(&b'*') => {
                data[index..index + 2].fill(b' ');
                index += 2;
                loop {
                    if index + 1 >= data.len() {
                        return Err(io::Error::other("Unclosed JSON comment."));
                    }
                    if data[index] == b'*' && data[index + 1] == b'/' {
                        data[index..index + 2].fill(b' ');
                        index += 2;
                        break;
                    }
                    if data[index] != b'\n' {
                        data[index] = b' ';
                    }
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }
    string = false;
    index = 0;
    while index < data.len() {
        match data[index] {
            b'\\' if string => index += 2,
            b'"' => {
                string = !string;
                index += 1;
            }
            b',' if !string => {
                let next = data[index + 1..]
                    .iter()
                    .find(|byte| !byte.is_ascii_whitespace());
                if matches!(next, Some(b'}' | b']')) {
                    data[index] = b' ';
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    serde_json::from_slice(&data).map_err(io::Error::other)
}

fn rgb(value: &Value) -> Option<[u8; 3]> {
    let value = value.as_str()?.strip_prefix('#')?;
    if !matches!(value.len(), 6 | 8) || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&value[..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
}

fn read_theme(
    archive: &mut Archive,
    path: &str,
    name: &str,
    dark: bool,
    stack: &mut HashSet<String>,
) -> io::Result<Theme> {
    if stack.len() >= 8 || !stack.insert(path.to_owned()) {
        return Err(io::Error::other(
            "Theme includes contain a cycle or exceed eight files.",
        ));
    }
    let value = archive.json(path)?;
    let mut theme = if let Some(include) = value["include"].as_str() {
        let parent = Path::new(path)
            .parent()
            .ok_or_else(|| io::Error::other("Invalid theme include."))?;
        // Normalize relative includes while keeping them inside extension/.
        let mut parts: Vec<_> = parent
            .to_str()
            .ok_or_else(|| io::Error::other("Invalid theme path."))?
            .split('/')
            .collect();
        for part in include.split('/') {
            match part {
                "." | "" => {}
                ".." => {
                    if parts.len() <= 1 {
                        return Err(io::Error::other("Theme include leaves the extension."));
                    }
                    parts.pop();
                }
                _ => parts.push(part),
            }
        }
        read_theme(archive, &parts.join("/"), name, dark, stack)?
    } else {
        Theme {
            name: name.to_owned(),
            dark,
            colors: BTreeMap::new(),
            syntax: BTreeMap::new(),
        }
    };
    const COLORS: &[(&str, &str)] = &[
        ("activityBar.background", "rail"),
        ("sideBar.background", "panel"),
        ("editor.background", "editor"),
        ("editorWidget.background", "elevated"),
        ("panel.border", "border"),
        ("editor.foreground", "text"),
        ("descriptionForeground", "muted"),
        ("editorLineNumber.foreground", "dim"),
        ("textLink.foreground", "accent"),
        ("editor.selectionBackground", "active"),
        ("gitDecoration.addedResourceForeground", "green"),
        ("errorForeground", "red"),
    ];
    for (vscode, forge) in COLORS {
        if let Some(color) = rgb(&value["colors"][vscode]) {
            theme.colors.insert((*forge).into(), color);
        }
    }
    for token in value["tokenColors"].as_array().into_iter().flatten() {
        let Some(color) = rgb(&token["settings"]["foreground"]) else {
            continue;
        };
        let scopes: Vec<_> = match &token["scope"] {
            Value::String(scope) => scope.split(',').map(str::trim).collect(),
            Value::Array(scopes) => scopes.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        for scope in scopes {
            let category = if scope.starts_with("comment") {
                Some("comment")
            } else if scope.starts_with("string") {
                Some("string")
            } else if scope.starts_with("constant") {
                Some("constant")
            } else if scope.starts_with("keyword.operator") {
                Some("operator")
            } else if scope.starts_with("keyword") || scope.starts_with("storage") {
                Some("keyword")
            } else if scope.contains("entity.name.function") || scope.contains("support.function") {
                Some("function")
            } else if scope.contains("entity.name.type")
                || scope.contains("support.type")
                || scope.contains("support.class")
            {
                Some("type")
            } else if scope.contains("variable.other.property") {
                Some("property")
            } else {
                None
            };
            if let Some(category) = category {
                theme.syntax.insert(category.into(), color);
            }
        }
    }
    theme.name = name.into();
    theme.dark = dark;
    stack.remove(path);
    Ok(theme)
}

fn extensions(language: &str, package: &Value) -> Vec<String> {
    if let Some(item) = package["contributes"]["languages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|item| item["id"] == language)
    {
        let result: Vec<_> = item["extensions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|extension| extension.trim_start_matches('.').to_owned())
            .collect();
        if !result.is_empty() {
            return result;
        }
    }
    match language {
        "rust" => vec!["rs"],
        "typescript" => vec!["ts", "mts", "cts"],
        "typescriptreact" => vec!["tsx"],
        "javascript" => vec!["js", "mjs", "cjs"],
        "javascriptreact" => vec!["jsx"],
        "python" => vec!["py"],
        "go" => vec!["go"],
        "c" => vec!["c", "h"],
        "cpp" => vec!["cpp", "cc", "hpp"],
        "json" | "jsonc" => vec!["json"],
        "html" => vec!["html", "htm"],
        "css" => vec!["css"],
        "scss" => vec!["scss"],
        "shellscript" => vec!["sh", "bash"],
        "markdown" => vec!["md"],
        "yaml" => vec!["yaml", "yml"],
        "java" => vec!["java"],
        "php" => vec!["php"],
        "ruby" => vec!["rb"],
        "swift" => vec!["swift"],
        "csharp" => vec!["cs"],
        _ => vec![],
    }
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn prepare(bytes: Vec<u8>) -> io::Result<Prepared> {
    let digest = crate::extension_registry::digest(&bytes);
    let mut archive = Archive::new(bytes)?;
    let package = archive.json("extension/package.json")?;
    let publisher = package["publisher"]
        .as_str()
        .ok_or_else(|| io::Error::other("VSIX publisher is missing."))?;
    let name = package["name"]
        .as_str()
        .ok_or_else(|| io::Error::other("VSIX name is missing."))?;
    let id = format!("{publisher}.{name}").to_lowercase();
    let mut metadata = Metadata {
        publisher: publisher.into(),
        description: package["description"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(4000)
            .collect(),
        license: package["license"].as_str().unwrap_or("Unspecified").into(),
        readme: archive.optional_text(&[
            "extension/README.md",
            "extension/readme.md",
            "extension/Readme.md",
        ]),
        changelog: archive.optional_text(&["extension/CHANGELOG.md", "extension/changelog.md"]),
        sha256: digest,
        imported: true,
        javascript: !package["main"].is_null() || !package["browser"].is_null(),
        categories: package["categories"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .take(20)
            .map(str::to_owned)
            .collect(),
        ..Metadata::default()
    };
    if metadata.javascript {
        metadata.notes.push("JavaScript extension host required for executable features. Only the contributions listed below are imported.".into());
    }
    for (key, _) in package["contributes"].as_object().into_iter().flatten() {
        if !matches!(key.as_str(), "themes" | "snippets" | "languages") {
            metadata
                .notes
                .push(format!("Unsupported VS Code contribution — {key}"));
        }
    }
    let mut manifest = Manifest {
        schema_version: 1,
        id,
        name: package["displayName"]
            .as_str()
            .unwrap_or(name)
            .chars()
            .take(120)
            .collect(),
        version: package["version"]
            .as_str()
            .ok_or_else(|| io::Error::other("VSIX version is missing."))?
            .into(),
        snippets: vec![],
        themes: vec![],
        transform: None,
        commands: vec![],
        dependencies: vec![],
        extension_pack: vec![],
        engine: None,
        description: metadata.description.clone(),
        license: metadata.license.clone(),
        publisher: metadata.publisher.clone(),
    };
    for item in package["contributes"]["themes"]
        .as_array()
        .into_iter()
        .flatten()
        .take(20)
    {
        let path = contribution_path(
            item["path"]
                .as_str()
                .ok_or_else(|| io::Error::other("Missing theme path."))?,
        )?;
        let name = item["label"].as_str().unwrap_or(&manifest.name);
        let dark = !matches!(item["uiTheme"].as_str(), Some("vs" | "hc-light"));
        manifest.themes.push(read_theme(
            &mut archive,
            &path,
            name,
            dark,
            &mut HashSet::new(),
        )?);
    }
    let mut skipped = 0;
    for item in package["contributes"]["snippets"]
        .as_array()
        .into_iter()
        .flatten()
        .take(100)
    {
        let language = item["language"].as_str().unwrap_or("");
        let path = contribution_path(
            item["path"]
                .as_str()
                .ok_or_else(|| io::Error::other("Missing snippets path."))?,
        )?;
        let snippets = archive.json(&path)?;
        for snippet in snippets
            .as_object()
            .into_iter()
            .flat_map(|map| map.values())
        {
            let body = match &snippet["body"] {
                Value::String(text) => text.clone(),
                Value::Array(lines) => lines
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => {
                    skipped += 1;
                    continue;
                }
            };
            let prefixes: Vec<_> = match &snippet["prefix"] {
                Value::String(prefix) => vec![prefix.as_str()],
                Value::Array(prefixes) => prefixes.iter().filter_map(Value::as_str).collect(),
                _ => vec![],
            };
            let scope = snippet["scope"].as_str().unwrap_or(language);
            let extensions: Vec<_> = scope
                .split(',')
                .flat_map(|language| extensions(language.trim(), &package))
                .collect();
            for prefix in prefixes {
                let contribution = Snippet {
                    prefix: prefix.to_owned(),
                    extensions: extensions.clone(),
                    body: body.clone(),
                };
                if manifest.snippets.len() >= 100 {
                    skipped += 1;
                    continue;
                }
                let mut check = manifest.clone();
                check.snippets = vec![contribution.clone()];
                if crate::packages::validate(&check).is_ok() {
                    manifest.snippets.push(contribution);
                } else {
                    skipped += 1;
                }
            }
        }
    }
    if skipped > 0 {
        metadata.notes.push(format!(
            "{skipped} snippets require unsupported syntax, scopes or exceed contribution limits."
        ));
    }
    if let Some(pack) = package["extensionPack"].as_array() {
        manifest.extension_pack = pack
            .iter()
            .filter_map(Value::as_str)
            .map(|id| id.to_lowercase())
            .collect();
    }
    if let Some(dependencies) = package["extensionDependencies"].as_array() {
        // Runtime dependencies are reported. They cannot supply an executable VS Code host.
        if !dependencies.is_empty() {
            metadata.notes.push(
                "Runtime extension dependencies are not activated by the declarative importer."
                    .into(),
            );
        }
    }
    if manifest.themes.is_empty()
        && manifest.snippets.is_empty()
        && manifest.extension_pack.is_empty()
    {
        return Err(io::Error::other(
            "This VSIX needs unsupported extension APIs and contains no importable themes, snippets or extension pack.",
        ));
    }
    crate::packages::validate(&manifest)?;
    Ok(Prepared {
        bundle: Bundle {
            manifest,
            enabled: true,
            module: None,
            command_modules: BTreeMap::new(),
            metadata,
            pinned: false,
            disabled_workspaces: vec![],
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn vsix(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (path, bytes) in files {
            writer
                .start_file(*path, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn manifest(contributes: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "publisher":"studio", "name":"toolbox", "displayName":"Toolbox",
            "version":"1.2.3", "license":"MIT", "contributes":contributes
        }))
        .unwrap()
    }

    #[test]
    fn converts_vsix_themes_snippets_and_reports_unsupported_features() {
        let mut package: Value = serde_json::from_slice(&manifest(json!({
            "themes":[{"label":"Midnight","uiTheme":"vs-dark","path":"themes/night.json"}],
            "snippets":[{"language":"rust","path":"snippets/rust.json"}],
            "commands":[{"command":"studio.run","title":"Run"}]
        })))
        .unwrap();
        package["main"] = json!("./extension.js");
        let theme = br##"{
            // Theme comments and trailing commas are accepted.
            "include":"../base.json",
            "colors":{"editor.background":"#101828ff",},
            "tokenColors":[{"scope":"keyword, storage.type","settings":{"foreground":"#ffaa88"}}],
        }"##;
        let base = br##"{"colors":{"sideBar.background":"#202838"}}"##;
        let snippets = json!({"answer":{"prefix":["answer","ans"],"body":["fn answer() {","    return 42;","}"]}});
        let result = prepare(vsix(&[
            (
                "extension/package.json",
                &serde_json::to_vec(&package).unwrap(),
            ),
            ("extension/themes/night.json", theme),
            ("extension/base.json", base),
            (
                "extension/snippets/rust.json",
                &serde_json::to_vec(&snippets).unwrap(),
            ),
            ("extension/README.md", b"# Toolbox"),
        ]))
        .unwrap();
        let bundle = result.bundle;
        assert_eq!(bundle.manifest.id, "studio.toolbox");
        assert_eq!(bundle.manifest.themes[0].colors["editor"], [16, 24, 40]);
        assert_eq!(bundle.manifest.themes[0].colors["panel"], [32, 40, 56]);
        assert_eq!(bundle.manifest.themes[0].syntax["keyword"], [255, 170, 136]);
        assert_eq!(bundle.manifest.snippets.len(), 2);
        assert_eq!(bundle.manifest.snippets[0].extensions, vec!["rs"]);
        assert!(bundle.metadata.javascript);
        assert!(
            bundle
                .metadata
                .notes
                .iter()
                .any(|note| note.contains("commands"))
        );
        assert_eq!(bundle.metadata.readme, "# Toolbox");
        assert_eq!(bundle.metadata.sha256.len(), 64);
        assert!(bundle.module.is_none() && bundle.command_modules.is_empty());
    }

    #[test]
    fn rejects_executable_only_extensions_without_running_them() {
        let package = json!({"publisher":"studio","name":"unsafe","version":"1.0.0","main":"extension.js","contributes":{"commands":[{"command":"unsafe.run"}]}});
        assert!(
            prepare(vsix(&[
                (
                    "extension/package.json",
                    &serde_json::to_vec(&package).unwrap()
                ),
                (
                    "extension/extension.js",
                    b"throw new Error('must not execute');"
                ),
            ]))
            .is_err()
        );
    }

    #[test]
    fn rejects_archive_traversal_and_absolute_paths() {
        for path in [
            "../escape",
            "/tmp/escape",
            "extension/../escape",
            "C:/escape",
            "extension\\escape",
        ] {
            assert!(prepare(vsix(&[(path, b"x")])).is_err(), "{path}");
        }
    }

    #[test]
    fn rejects_theme_include_cycles_and_escapes() {
        let package = manifest(json!({"themes":[{"path":"themes/night.json","label":"Night"}]}));
        for theme in [
            br#"{"include":"night.json"}"#.as_slice(),
            br#"{"include":"../../outside.json"}"#,
        ] {
            assert!(
                prepare(vsix(&[
                    ("extension/package.json", &package),
                    ("extension/themes/night.json", theme),
                ]))
                .is_err()
            );
        }
    }

    #[test]
    fn skips_unsupported_snippet_variables_without_claiming_compatibility() {
        let package = manifest(json!({"snippets":[{"language":"rust","path":"rust.json"}]}));
        let snippets = json!({"ok":{"prefix":"ok","body":"return 42;"},"unsupported":{"prefix":"bad","body":concat!("$", "{TM_FILENAME}")}});
        let bundle = prepare(vsix(&[
            ("extension/package.json", &package),
            (
                "extension/rust.json",
                &serde_json::to_vec(&snippets).unwrap(),
            ),
        ]))
        .unwrap()
        .bundle;
        assert_eq!(bundle.manifest.snippets.len(), 1);
        assert!(
            bundle
                .metadata
                .notes
                .iter()
                .any(|note| note.contains("1 snippets"))
        );
    }

    #[test]
    fn jsonc_preserves_comments_inside_strings_and_rejects_unclosed_comments() {
        let value = jsonc(br#"{"url":"https://example.test/a/*b*/","text":"a,\"b\"",/* comment */"list":[1,2,],}"#).unwrap();
        assert_eq!(value["url"], "https://example.test/a/*b*/");
        assert_eq!(value["text"], "a,\"b\"");
        assert_eq!(value["list"], json!([1, 2]));
        assert!(jsonc(b"{ /* unfinished").is_err());
        assert!(jsonc(b"{\"a\": 1,,}").is_err());
    }

    #[test]
    fn pack_imports_members_without_executing_them() {
        let package = json!({"publisher":"studio","name":"pack","version":"1.0.0","extensionPack":["studio.theme","studio.snippets"]});
        let bundle = prepare(vsix(&[(
            "extension/package.json",
            &serde_json::to_vec(&package).unwrap(),
        )]))
        .unwrap()
        .bundle;
        assert_eq!(
            bundle.manifest.extension_pack,
            vec!["studio.theme", "studio.snippets"]
        );
    }

    #[test]
    fn rejects_symlinks_and_excessive_expanded_size() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_symlink(
                "extension/linked",
                "../../outside",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        assert!(prepare(writer.finish().unwrap().into_inner()).is_err());
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "oversized",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        let block = [0u8; 64 * 1024];
        for _ in 0..1025 {
            writer.write_all(&block).unwrap();
        }
        assert!(prepare(writer.finish().unwrap().into_inner()).is_err());
    }
}
