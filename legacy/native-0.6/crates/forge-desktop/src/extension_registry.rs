use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    time::Duration,
};

pub const REGISTRY: &str = "https://open-vsx.org";
pub const DOWNLOAD_LIMIT: u64 = 32 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Remote {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub publisher: String,
    pub license: String,
    pub downloads: u64,
    pub verified_namespace: bool,
    pub versions: Vec<String>,
    pub download: String,
    pub checksum: Option<String>,
    pub readme_url: Option<String>,
    pub categories: Vec<String>,
}

pub struct Search {
    pub items: Vec<Remote>,
    pub total: usize,
    pub offset: usize,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn fetch(url: &str, limit: u64) -> io::Result<Vec<u8>> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(20))
        .timeout(Duration::from_secs(45))
        .redirects(0)
        .build();
    let mut url = url::Url::parse(url).map_err(io::Error::other)?;
    for _ in 0..6 {
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(io::Error::other(
                "Extension downloads require HTTPS without credentials in the URL.",
            ));
        }
        let response = agent
            .get(url.as_str())
            .set(
                "User-Agent",
                concat!("ForgeIDE/", env!("CARGO_PKG_VERSION")),
            )
            .call()
            .map_err(io::Error::other)?;
        if matches!(response.status(), 301 | 302 | 303 | 307 | 308) {
            let location = response
                .header("Location")
                .ok_or_else(|| io::Error::other("Missing download redirect."))?;
            url = url.join(location).map_err(io::Error::other)?;
            continue;
        }
        if response.status() != 200 {
            return Err(io::Error::other(format!(
                "Registry returned HTTP {}.",
                response.status()
            )));
        }
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(limit + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(io::Error::other(
                "Registry response exceeds the download limit.",
            ));
        }
        return Ok(bytes);
    }
    Err(io::Error::other("Too many registry redirects."))
}

fn remote(value: &Value) -> io::Result<Remote> {
    let namespace = value["namespace"].as_str().unwrap_or("");
    let extension = value["name"].as_str().unwrap_or("");
    let id = format!("{namespace}.{extension}").to_lowercase();
    if namespace.is_empty() || extension.is_empty() || !crate::packages::valid_id(&id) {
        return Err(io::Error::other("Invalid registry extension identifier."));
    }
    let version = value["version"]
        .as_str()
        .ok_or_else(|| io::Error::other("Missing extension version."))?
        .to_owned();
    semver::Version::parse(&version).map_err(io::Error::other)?;
    let mut versions: Vec<_> = value["allVersions"]
        .as_object()
        .into_iter()
        .flat_map(|map| map.keys())
        .filter_map(|value| {
            semver::Version::parse(value)
                .ok()
                .map(|version| (version, value.clone()))
        })
        .collect();
    versions.sort_by(|left, right| right.0.cmp(&left.0));
    Ok(Remote {
        id,
        name: value["displayName"]
            .as_str()
            .unwrap_or(extension)
            .chars()
            .take(120)
            .collect(),
        version,
        description: value["description"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(4000)
            .collect(),
        publisher: namespace.chars().take(100).collect(),
        license: value["license"]
            .as_str()
            .unwrap_or("Unspecified")
            .chars()
            .take(200)
            .collect(),
        downloads: value["downloadCount"].as_u64().unwrap_or(0),
        verified_namespace: value["verified"].as_bool().unwrap_or(false),
        versions: versions.into_iter().map(|(_, text)| text).collect(),
        download: value["files"]["download"].as_str().unwrap_or("").to_owned(),
        checksum: value["files"]["sha256"].as_str().map(str::to_owned),
        readme_url: value["files"]["readme"].as_str().map(str::to_owned),
        categories: value["categories"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .take(20)
            .map(str::to_owned)
            .collect(),
    })
}

pub fn search(query: &str, offset: usize) -> io::Result<Search> {
    let mut url = url::Url::parse(&format!("{REGISTRY}/api/-/search")).map_err(io::Error::other)?;
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("size", "30")
        .append_pair("offset", &offset.to_string());
    let value: Value = serde_json::from_slice(&fetch(url.as_str(), 2 * 1024 * 1024)?)?;
    let items = value["extensions"]
        .as_array()
        .ok_or_else(|| io::Error::other("Invalid registry search response."))?
        .iter()
        .take(30)
        .map(remote)
        .collect::<io::Result<Vec<_>>>()?;
    Ok(Search {
        items,
        total: value["totalSize"].as_u64().unwrap_or(0) as usize,
        offset,
    })
}

pub fn details(id: &str, version: Option<&str>) -> io::Result<Remote> {
    if !crate::packages::valid_id(&id.to_lowercase()) {
        return Err(io::Error::other("Use a publisher.extension identifier."));
    }
    let (publisher, name) = id
        .split_once('.')
        .ok_or_else(|| io::Error::other("Use publisher.extension."))?;
    if let Some(version) = version {
        semver::Version::parse(version).map_err(io::Error::other)?;
    }
    let mut url = url::Url::parse(REGISTRY).map_err(io::Error::other)?;
    url.path_segments_mut()
        .map_err(|_| io::Error::other("Invalid registry URL."))?
        .extend(["api", publisher, name, version.unwrap_or("latest")]);
    let value: Value = serde_json::from_slice(&fetch(url.as_str(), 2 * 1024 * 1024)?)?;
    let result = remote(&value)?;
    if result.id != id.to_lowercase() {
        return Err(io::Error::other(
            "The registry returned another extension identifier.",
        ));
    }
    Ok(result)
}

pub fn download(remote: &Remote) -> io::Result<Vec<u8>> {
    let bytes = fetch(&remote.download, DOWNLOAD_LIMIT)?;
    if let Some(url) = &remote.checksum {
        let expected = String::from_utf8(fetch(url, 4096)?).map_err(io::Error::other)?;
        let expected = expected.split_whitespace().next().unwrap_or("");
        if expected.len() != 64 || !expected.eq_ignore_ascii_case(&digest(&bytes)) {
            return Err(io::Error::other(
                "The extension checksum does not match the registry. Installation stopped.",
            ));
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_insecure_downloads_and_invalid_identifiers_before_network() {
        for url in [
            "http://example.com/file",
            "file:///etc/passwd",
            "https://user:secret@example.com/file",
        ] {
            assert!(fetch(url, 1024).is_err());
        }
        for id in ["../escape", "publisher/name", "missingnamespace"] {
            assert!(details(id, None).is_err());
        }
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    #[ignore = "downloads a real Open VSX package over HTTPS"]
    fn open_vsx_search_download_checksum_and_import() -> io::Result<()> {
        let search = search("gruvbox", 0)?;
        assert!(search.total > 0 && !search.items.is_empty());
        let remote = details("jdinhlife.gruvbox", Some("1.29.2"))?;
        let bytes = download(&remote)?;
        let prepared = crate::extension_vsix::prepare(bytes)?;
        assert_eq!(prepared.bundle.manifest.id, remote.id);
        assert_eq!(prepared.bundle.manifest.version, remote.version);
        assert!(!prepared.bundle.manifest.themes.is_empty());
        assert!(!prepared.bundle.metadata.readme.is_empty());
        eprintln!(
            "Open VSX verified: {}@{} · {} themes",
            remote.id,
            remote.version,
            prepared.bundle.manifest.themes.len()
        );
        Ok(())
    }
}
