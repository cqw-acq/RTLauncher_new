use super::ThemeStoreError;
use regex::Regex;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek};
use std::path::Component;
use std::path::PathBuf;
use std::sync::OnceLock;

pub const THEME_SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeManifest {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: ThemeAuthor,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub icon: Option<String>,
    #[serde(default)]
    pub previews: Vec<String>,
    pub engines: ThemeEngines,
    pub entry: ThemeEntry,
    pub supports: ThemeSupports,
    pub contributes: Option<serde_json::Value>,
    #[serde(default)]
    pub disclosures: Vec<String>,
    pub integrity: Option<ThemeIntegrity>,
    pub extensions: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeAuthor {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeEngines {
    pub rtlauncher: String,
    pub theme_api: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeEntry {
    pub script: String,
    pub style: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSupports {
    pub color_schemes: Vec<String>,
    #[serde(default)]
    pub locales: Vec<String>,
    #[serde(default)]
    pub user_overrides: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeIntegrity {
    pub algorithm: String,
    pub files: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ArchiveLimits {
    pub max_files: usize,
    pub max_file_size: u64,
    pub max_total_size: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_files: 1_024,
            max_file_size: 16 * 1024 * 1024,
            max_total_size: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct InspectedThemeArchive {
    pub manifest: ThemeManifest,
    pub file_count: usize,
    pub total_uncompressed_size: u64,
}

fn archive_error(code: &str, message: impl Into<String>) -> ThemeStoreError {
    ThemeStoreError::new(code, message)
}

pub fn validate_archive_entry(name: &str) -> Result<PathBuf, ThemeStoreError> {
    if name.is_empty()
        || name.starts_with('/')
        || name.starts_with('\\')
        || name.contains('\\')
        || name.contains("//")
        || name.as_bytes().get(1) == Some(&b':')
    {
        return Err(archive_error(
            "THEME_ARCHIVE_PATH_INVALID",
            format!("Archive entry has an unsafe path: {name}"),
        ));
    }

    let normalized = name.trim_end_matches('/');
    if normalized.is_empty() {
        return Err(archive_error(
            "THEME_ARCHIVE_PATH_INVALID",
            "Archive entry path is empty.",
        ));
    }
    let path = PathBuf::from(normalized);
    if !path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(archive_error(
            "THEME_ARCHIVE_PATH_INVALID",
            format!("Archive entry leaves the install directory: {name}"),
        ));
    }
    Ok(path)
}

pub fn inspect_theme_archive<R: Read + Seek>(
    reader: R,
    limits: ArchiveLimits,
) -> Result<InspectedThemeArchive, ThemeStoreError> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|error| {
        archive_error(
            "THEME_ARCHIVE_INVALID",
            format!("Cannot open Theme archive: {error}"),
        )
    })?;
    let mut seen = HashSet::new();
    let mut files = HashMap::<String, Vec<u8>>::new();
    let mut file_count = 0usize;
    let mut total_uncompressed_size = 0u64;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            archive_error(
                "THEME_ARCHIVE_INVALID",
                format!("Cannot read Theme archive entry: {error}"),
            )
        })?;
        let path = validate_archive_entry(entry.name())?;
        let normalized = path.to_string_lossy().replace('\\', "/");
        if !seen.insert(normalized.clone()) {
            return Err(archive_error(
                "THEME_ARCHIVE_DUPLICATE_FILE",
                format!("Archive entry is duplicated: {normalized}"),
            ));
        }
        if entry.is_dir() {
            continue;
        }

        file_count += 1;
        if file_count > limits.max_files || entry.size() > limits.max_file_size {
            return Err(archive_error(
                "THEME_ARCHIVE_LIMIT_EXCEEDED",
                format!("Archive entry exceeds a package limit: {normalized}"),
            ));
        }
        total_uncompressed_size = total_uncompressed_size
            .checked_add(entry.size())
            .ok_or_else(|| {
                archive_error("THEME_ARCHIVE_LIMIT_EXCEEDED", "Archive size overflowed.")
            })?;
        if total_uncompressed_size > limits.max_total_size {
            return Err(archive_error(
                "THEME_ARCHIVE_LIMIT_EXCEEDED",
                "Archive total size exceeds the package limit.",
            ));
        }

        let mut content = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut content).map_err(|error| {
            archive_error(
                "THEME_ARCHIVE_INVALID",
                format!("Cannot read {normalized}: {error}"),
            )
        })?;
        if content.len() as u64 != entry.size() {
            return Err(archive_error(
                "THEME_ARCHIVE_INVALID",
                format!("Archive entry size changed while reading: {normalized}"),
            ));
        }
        files.insert(normalized, content);
    }

    let manifest_bytes = files.get("manifest.json").ok_or_else(|| {
        archive_error(
            "THEME_MANIFEST_MISSING",
            "Theme archive does not contain manifest.json.",
        )
    })?;
    let manifest = parse_theme_manifest(manifest_bytes, true)?;
    verify_integrity(&manifest, &files)?;

    Ok(InspectedThemeArchive {
        manifest,
        file_count,
        total_uncompressed_size,
    })
}

pub fn parse_theme_manifest(
    content: &[u8],
    require_integrity: bool,
) -> Result<ThemeManifest, ThemeStoreError> {
    let manifest: ThemeManifest = serde_json::from_slice(content).map_err(|error| {
        archive_error(
            "THEME_MANIFEST_INVALID",
            format!("Cannot parse manifest.json: {error}"),
        )
    })?;
    validate_manifest(&manifest, require_integrity)?;
    Ok(manifest)
}

fn validate_manifest(
    manifest: &ThemeManifest,
    require_integrity: bool,
) -> Result<(), ThemeStoreError> {
    let schema_major = manifest
        .schema_version
        .split('.')
        .next()
        .and_then(|value| value.parse::<u64>().ok());
    let supported_major = THEME_SCHEMA_VERSION
        .split('.')
        .next()
        .and_then(|value| value.parse::<u64>().ok());
    if schema_major.is_none() || schema_major != supported_major {
        return Err(archive_error(
            "THEME_SCHEMA_INCOMPATIBLE",
            format!("Unsupported Theme schema: {}", manifest.schema_version),
        ));
    }

    static THEME_ID: OnceLock<Regex> = OnceLock::new();
    let id_pattern = THEME_ID.get_or_init(|| {
        Regex::new(r"^[a-z0-9]+(?:[.-][a-z0-9]+)+$").expect("Theme ID regular expression is valid")
    });
    if manifest.id.starts_with("builtin.") || !id_pattern.is_match(&manifest.id) {
        return Err(archive_error(
            "THEME_MANIFEST_INVALID",
            "Theme ID must be a lowercase reverse-domain ID and cannot use builtin.*.",
        ));
    }
    if manifest.name.trim().is_empty()
        || manifest.author.name.trim().is_empty()
        || Version::parse(&manifest.version).is_err()
    {
        return Err(archive_error(
            "THEME_MANIFEST_INVALID",
            "Theme name, author, or SemVer version is invalid.",
        ));
    }
    if !is_version_requirement(&manifest.engines.rtlauncher)
        || !is_version_requirement(&manifest.engines.theme_api)
    {
        return Err(archive_error(
            "THEME_MANIFEST_INVALID",
            "Theme engine requirements are invalid.",
        ));
    }
    validate_package_path(&manifest.entry.script, "entry.script")?;
    if let Some(style) = &manifest.entry.style {
        validate_package_path(style, "entry.style")?;
    }
    if let Some(icon) = &manifest.icon {
        validate_package_path(icon, "icon")?;
    }
    for preview in &manifest.previews {
        validate_package_path(preview, "previews")?;
    }
    if manifest.supports.color_schemes.is_empty()
        || manifest
            .supports
            .color_schemes
            .iter()
            .any(|scheme| scheme != "light" && scheme != "dark")
    {
        return Err(archive_error(
            "THEME_MANIFEST_INVALID",
            "Theme color schemes must contain light or dark.",
        ));
    }
    match &manifest.integrity {
        Some(integrity) if integrity.algorithm == "sha256" && !integrity.files.is_empty() => {}
        None if !require_integrity => {}
        _ => {
            return Err(archive_error(
                "THEME_MANIFEST_INVALID",
                "Theme integrity must contain SHA-256 hashes.",
            ));
        }
    }
    Ok(())
}

fn validate_package_path(path: &str, field: &str) -> Result<(), ThemeStoreError> {
    validate_archive_entry(path).map(|_| ()).map_err(|_| {
        archive_error(
            "THEME_MANIFEST_INVALID",
            format!("{field} is not a safe package path."),
        )
    })
}

fn is_version_requirement(value: &str) -> bool {
    if value.trim().is_empty() {
        return false;
    }
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(", ");
    VersionReq::parse(&normalized).is_ok()
}

fn verify_integrity(
    manifest: &ThemeManifest,
    files: &HashMap<String, Vec<u8>>,
) -> Result<(), ThemeStoreError> {
    let integrity = manifest.integrity.as_ref().ok_or_else(|| {
        archive_error(
            "THEME_MANIFEST_INVALID",
            "Theme archive does not contain integrity hashes.",
        )
    })?;
    for (path, expected) in &integrity.files {
        validate_package_path(path, "integrity.files")?;
        let content = files.get(path).ok_or_else(|| {
            archive_error(
                "THEME_INTEGRITY_FAILED",
                format!("Integrity file is missing: {path}"),
            )
        })?;
        let expected = expected.strip_prefix("sha256-").unwrap_or(expected);
        if expected.len() != 64 || !expected.bytes().all(|value| value.is_ascii_hexdigit()) {
            return Err(archive_error(
                "THEME_MANIFEST_INVALID",
                format!("Integrity hash is invalid: {path}"),
            ));
        }
        let actual = hex::encode(Sha256::digest(content));
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(archive_error(
                "THEME_INTEGRITY_FAILED",
                format!("Integrity check failed: {path}"),
            ));
        }
    }

    if !manifest
        .integrity
        .as_ref()
        .is_some_and(|integrity| integrity.files.contains_key(&manifest.entry.script))
        || manifest
            .entry
            .style
            .as_ref()
            .is_some_and(|style| !integrity.files.contains_key(style))
    {
        return Err(archive_error(
            "THEME_INTEGRITY_FAILED",
            "Theme entry files must have integrity hashes.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::io::{Cursor, Write};
    use zip::write::FileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn sha256(value: &[u8]) -> String {
        format!("sha256-{}", hex::encode(Sha256::digest(value)))
    }

    fn valid_manifest(script: &[u8]) -> Value {
        json!({
            "schemaVersion": "1.0",
            "id": "com.example.nebula",
            "name": "Nebula",
            "version": "1.2.0",
            "author": { "name": "Example" },
            "engines": {
                "rtlauncher": ">=0.2.0 <1.0.0",
                "themeApi": "^1.0.0"
            },
            "entry": { "script": "dist/theme.js", "style": "dist/theme.css" },
            "supports": { "colorSchemes": ["light", "dark"] },
            "integrity": {
                "algorithm": "sha256",
                "files": {
                    "dist/theme.js": sha256(script),
                    "dist/theme.css": sha256(b"body{}")
                }
            }
        })
    }

    fn archive(entries: Vec<(String, Vec<u8>)>) -> Cursor<Vec<u8>> {
        let mut output = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut output);
            let options = FileOptions::default().compression_method(CompressionMethod::Stored);
            for (name, content) in entries {
                writer.start_file(name, options).expect("start ZIP entry");
                writer.write_all(&content).expect("write ZIP entry");
            }
            writer.finish().expect("finish ZIP");
        }
        output.set_position(0);
        output
    }

    fn valid_archive() -> Cursor<Vec<u8>> {
        let script = b"globalThis.theme = true;";
        let manifest = serde_json::to_vec(&valid_manifest(script)).expect("serialize manifest");
        archive(vec![
            ("manifest.json".into(), manifest),
            ("dist/theme.js".into(), script.to_vec()),
            ("dist/theme.css".into(), b"body{}".to_vec()),
        ])
    }

    #[test]
    fn archive_paths_stay_inside_the_install_directory() {
        for path in ["manifest.json", "dist/theme.js", "assets/icon.png"] {
            assert_eq!(validate_archive_entry(path).unwrap(), PathBuf::from(path));
        }
        for path in [
            "",
            "/theme.js",
            "../theme.js",
            "dist/../theme.js",
            "dist\\theme.js",
            "C:/theme.js",
            "dist//theme.js",
        ] {
            let error = validate_archive_entry(path).unwrap_err();
            assert_eq!(error.code, "THEME_ARCHIVE_PATH_INVALID", "path: {path}");
        }
    }

    #[test]
    fn valid_archive_returns_manifest_and_size_summary() {
        let inspected = inspect_theme_archive(valid_archive(), ArchiveLimits::default()).unwrap();

        assert_eq!(inspected.manifest.id, "com.example.nebula");
        assert_eq!(inspected.manifest.version, "1.2.0");
        assert_eq!(inspected.file_count, 3);
        assert!(inspected.total_uncompressed_size > 20);
    }

    #[test]
    fn archive_rejects_unsafe_and_duplicate_entries() {
        let unsafe_archive = archive(vec![("../theme.js".into(), b"x".to_vec())]);
        assert_eq!(
            inspect_theme_archive(unsafe_archive, ArchiveLimits::default())
                .unwrap_err()
                .code,
            "THEME_ARCHIVE_PATH_INVALID"
        );

        let duplicate_archive = archive(vec![
            ("manifest.json".into(), b"{}".to_vec()),
            ("manifest.json".into(), b"{}".to_vec()),
        ]);
        assert_eq!(
            inspect_theme_archive(duplicate_archive, ArchiveLimits::default())
                .unwrap_err()
                .code,
            "THEME_ARCHIVE_DUPLICATE_FILE"
        );
    }

    #[test]
    fn archive_enforces_file_count_and_size_limits() {
        let too_many = inspect_theme_archive(
            valid_archive(),
            ArchiveLimits {
                max_files: 2,
                ..ArchiveLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(too_many.code, "THEME_ARCHIVE_LIMIT_EXCEEDED");

        let too_large = inspect_theme_archive(
            valid_archive(),
            ArchiveLimits {
                max_file_size: 8,
                ..ArchiveLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(too_large.code, "THEME_ARCHIVE_LIMIT_EXCEEDED");
    }

    #[test]
    fn archive_rejects_invalid_manifest_fields() {
        let script = b"theme";
        for (field, value) in [
            ("id", json!("Nebula")),
            ("id", json!("builtin.default")),
            ("version", json!("latest")),
            ("schemaVersion", json!("2.0")),
        ] {
            let mut manifest = valid_manifest(script);
            manifest[field] = value;
            let archive = archive(vec![
                (
                    "manifest.json".into(),
                    serde_json::to_vec(&manifest).expect("serialize manifest"),
                ),
                ("dist/theme.js".into(), script.to_vec()),
                ("dist/theme.css".into(), b"body{}".to_vec()),
            ]);

            let error = inspect_theme_archive(archive, ArchiveLimits::default()).unwrap_err();
            assert!(
                ["THEME_MANIFEST_INVALID", "THEME_SCHEMA_INCOMPATIBLE"]
                    .contains(&error.code.as_str()),
                "field: {field}, error: {error}"
            );
        }
    }

    #[test]
    fn archive_rejects_missing_or_mismatched_integrity_files() {
        let script = b"tampered";
        let manifest =
            serde_json::to_vec(&valid_manifest(b"expected")).expect("serialize manifest");
        let mismatched = archive(vec![
            ("manifest.json".into(), manifest),
            ("dist/theme.js".into(), script.to_vec()),
            ("dist/theme.css".into(), b"body{}".to_vec()),
        ]);

        assert_eq!(
            inspect_theme_archive(mismatched, ArchiveLimits::default())
                .unwrap_err()
                .code,
            "THEME_INTEGRITY_FAILED"
        );
    }
}
