use super::manifest::{
    inspect_theme_archive, parse_theme_manifest, validate_archive_entry, ArchiveLimits,
    ThemeManifest,
};
use super::ThemeStoreError;
use semver::Version;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const BUILTIN_THEME_ID: &str = "builtin.default";
const REGISTRY_FILE: &str = "registry.json";
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackage {
    pub manifest: ThemeManifest,
    pub development: bool,
    pub location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStoreState {
    pub active_theme_id: String,
    pub last_healthy_theme_id: String,
    pub pending_theme_id: Option<String>,
    #[serde(default)]
    packages: Vec<ThemePackage>,
    #[serde(default, deserialize_with = "deserialize_trusted_packages")]
    trusted_packages: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredTrustedPackages {
    Current(BTreeMap<String, BTreeSet<String>>),
    Legacy(BTreeMap<String, String>),
}

fn deserialize_trusted_packages<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, BTreeSet<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(match StoredTrustedPackages::deserialize(deserializer)? {
        StoredTrustedPackages::Current(packages) => packages,
        StoredTrustedPackages::Legacy(packages) => packages
            .into_iter()
            .map(|(theme_id, version)| (theme_id, BTreeSet::from([version])))
            .collect(),
    })
}

impl Default for ThemeStoreState {
    fn default() -> Self {
        Self {
            active_theme_id: BUILTIN_THEME_ID.into(),
            last_healthy_theme_id: BUILTIN_THEME_ID.into(),
            pending_theme_id: None,
            packages: Vec::new(),
            trusted_packages: BTreeMap::new(),
        }
    }
}

pub struct ThemeStore {
    root: PathBuf,
    state: ThemeStoreState,
}

impl ThemeStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, ThemeStoreError> {
        let root = root.into();
        fs::create_dir_all(root.join("packages")).map_err(store_io)?;
        fs::create_dir_all(root.join(".staging")).map_err(store_io)?;
        let registry_path = root.join(REGISTRY_FILE);
        let registry_backup = root.join("registry.json.bak");
        if !registry_path.exists() && registry_backup.exists() {
            fs::rename(&registry_backup, &registry_path).map_err(store_io)?;
        }
        let state = if registry_path.exists() {
            let content = fs::read(&registry_path).map_err(store_io)?;
            serde_json::from_slice(&content).map_err(|error| {
                ThemeStoreError::new(
                    "THEME_STORE_INVALID",
                    format!("Cannot parse Theme registry: {error}"),
                )
            })?
        } else {
            ThemeStoreState::default()
        };
        let mut store = Self { root, state };
        if store.state.pending_theme_id.is_some() {
            store.state.active_theme_id = store.state.last_healthy_theme_id.clone();
            store.state.pending_theme_id = None;
        }
        store.save()?;
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn state(&self) -> &ThemeStoreState {
        &self.state
    }

    pub fn list(&self) -> Result<Vec<ThemePackage>, ThemeStoreError> {
        let mut packages = self.state.packages.clone();
        packages.sort_by(|left, right| {
            left.manifest
                .id
                .cmp(&right.manifest.id)
                .then_with(|| right.development.cmp(&left.development))
                .then_with(|| compare_versions(&right.manifest.version, &left.manifest.version))
        });
        Ok(packages)
    }

    pub fn install_archive(
        &mut self,
        archive_path: &Path,
    ) -> Result<ThemePackage, ThemeStoreError> {
        let mut file = fs::File::open(archive_path).map_err(store_io)?;
        let inspected = inspect_theme_archive(&mut file, ArchiveLimits::default())?;
        let manifest = inspected.manifest;
        let target = self
            .root
            .join("packages")
            .join(&manifest.id)
            .join(&manifest.version);
        if target.exists() {
            return Err(ThemeStoreError::new(
                "THEME_ALREADY_INSTALLED",
                format!(
                    "Theme {} {} is already installed.",
                    manifest.id, manifest.version
                ),
            ));
        }

        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging = self.root.join(".staging").join(format!(
            "{}-{}-{}-{sequence}",
            manifest.id,
            manifest.version,
            std::process::id()
        ));
        fs::create_dir(&staging).map_err(store_io)?;
        let extraction = file
            .rewind()
            .map_err(store_io)
            .and_then(|()| extract_archive(&mut file, &staging));
        if let Err(error) = extraction {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }

        let parent = target.parent().ok_or_else(|| {
            ThemeStoreError::new("THEME_STORE_IO", "Theme install path has no parent.")
        })?;
        fs::create_dir_all(parent).map_err(store_io)?;
        if let Err(error) = fs::rename(&staging, &target) {
            let _ = fs::remove_dir_all(&staging);
            return Err(store_io(error));
        }

        let package = ThemePackage {
            manifest,
            development: false,
            location: relative_location(&self.root, &target)?,
        };
        self.state.packages.push(package.clone());
        if let Err(error) = self.save() {
            self.state.packages.pop();
            let _ = fs::remove_dir_all(&target);
            return Err(error);
        }
        Ok(package)
    }

    pub fn register_dev_directory(
        &mut self,
        directory: &Path,
    ) -> Result<ThemePackage, ThemeStoreError> {
        let canonical = fs::canonicalize(directory).map_err(store_io)?;
        if !canonical.is_dir() {
            return Err(ThemeStoreError::new(
                "THEME_DEV_DIRECTORY_INVALID",
                "Development Theme path is not a directory.",
            ));
        }
        let manifest = parse_theme_manifest(
            &fs::read(canonical.join("manifest.json")).map_err(store_io)?,
            false,
        )?;
        verify_dev_entry(&canonical, &manifest.entry.script)?;
        if let Some(style) = &manifest.entry.style {
            verify_dev_entry(&canonical, style)?;
        }
        let package = ThemePackage {
            manifest,
            development: true,
            location: canonical.to_string_lossy().into_owned(),
        };
        self.state
            .packages
            .retain(|current| !(current.development && current.manifest.id == package.manifest.id));
        self.state.packages.push(package.clone());
        self.save()?;
        Ok(package)
    }

    pub fn read_binary(&self, theme_id: &str, path: &str) -> Result<Vec<u8>, ThemeStoreError> {
        let package = self.resolve_package(theme_id)?;
        let package_root = self.package_root(package);
        let relative = validate_archive_entry(path).map_err(|_| {
            ThemeStoreError::new("THEME_PATH_INVALID", "Theme file path is not safe.")
        })?;
        let canonical_root = fs::canonicalize(&package_root).map_err(store_io)?;
        let file = fs::canonicalize(package_root.join(relative)).map_err(store_io)?;
        if !file.starts_with(&canonical_root) || !file.is_file() {
            return Err(ThemeStoreError::new(
                "THEME_PATH_INVALID",
                "Theme file leaves the package directory.",
            ));
        }
        fs::read(file).map_err(store_io)
    }

    pub fn read_text(&self, theme_id: &str, path: &str) -> Result<String, ThemeStoreError> {
        String::from_utf8(self.read_binary(theme_id, path)?).map_err(|error| {
            ThemeStoreError::new(
                "THEME_TEXT_INVALID",
                format!("Theme file is not UTF-8 text: {error}"),
            )
        })
    }

    pub fn remove(&mut self, theme_id: &str, version: Option<&str>) -> Result<(), ThemeStoreError> {
        if theme_id == BUILTIN_THEME_ID {
            return Err(ThemeStoreError::new(
                "THEME_BUILTIN_PROTECTED",
                "The built-in Theme cannot be removed.",
            ));
        }
        let mut removed = Vec::new();
        self.state.packages.retain(|package| {
            let matches = package.manifest.id == theme_id
                && version.is_none_or(|value| package.manifest.version == value);
            if matches {
                removed.push(package.clone());
            }
            !matches
        });
        if removed.is_empty() {
            return Err(ThemeStoreError::new(
                "THEME_NOT_FOUND",
                format!("Theme is not installed: {theme_id}"),
            ));
        }
        for package in &removed {
            if !package.development {
                let path = self.package_root(package);
                if path.is_dir() {
                    fs::remove_dir_all(path).map_err(store_io)?;
                }
            }
        }
        if self.state.active_theme_id == theme_id {
            self.state.active_theme_id = BUILTIN_THEME_ID.into();
            self.state.last_healthy_theme_id = BUILTIN_THEME_ID.into();
            self.state.pending_theme_id = None;
        }
        let remove_trust_entry =
            if let Some(trusted_versions) = self.state.trusted_packages.get_mut(theme_id) {
                for package in &removed {
                    trusted_versions.remove(&package.manifest.version);
                }
                trusted_versions.is_empty()
            } else {
                false
            };
        if remove_trust_entry {
            self.state.trusted_packages.remove(theme_id);
        }
        self.save()
    }

    pub fn set_active(&mut self, theme_id: &str) -> Result<(), ThemeStoreError> {
        if theme_id != BUILTIN_THEME_ID {
            self.resolve_package(theme_id)?;
        }
        self.state.active_theme_id = theme_id.into();
        self.state.pending_theme_id = Some(theme_id.into());
        self.save()
    }

    pub fn mark_healthy(&mut self, theme_id: &str) -> Result<(), ThemeStoreError> {
        if self.state.active_theme_id != theme_id
            || self.state.pending_theme_id.as_deref() != Some(theme_id)
        {
            return Err(ThemeStoreError::new(
                "THEME_ACTIVATION_MISMATCH",
                "Theme health confirmation does not match the pending activation.",
            ));
        }
        self.state.last_healthy_theme_id = theme_id.into();
        self.state.pending_theme_id = None;
        self.save()
    }

    pub fn is_trusted(&self, theme_id: &str, version: &str) -> bool {
        self.state
            .trusted_packages
            .get(theme_id)
            .is_some_and(|trusted_versions| trusted_versions.contains(version))
    }

    pub fn set_trusted(
        &mut self,
        theme_id: &str,
        version: &str,
        trusted: bool,
    ) -> Result<(), ThemeStoreError> {
        if trusted
            && !self.state.packages.iter().any(|package| {
                package.manifest.id == theme_id && package.manifest.version == version
            })
        {
            return Err(ThemeStoreError::new(
                "THEME_NOT_FOUND",
                format!("Theme is not installed: {theme_id} {version}"),
            ));
        }
        if trusted {
            self.state
                .trusted_packages
                .entry(theme_id.into())
                .or_default()
                .insert(version.into());
        } else {
            let remove_trust_entry =
                self.state
                    .trusted_packages
                    .get_mut(theme_id)
                    .is_some_and(|trusted_versions| {
                        trusted_versions.remove(version);
                        trusted_versions.is_empty()
                    });
            if remove_trust_entry {
                self.state.trusted_packages.remove(theme_id);
            }
        }
        self.save()
    }

    fn resolve_package(&self, theme_id: &str) -> Result<&ThemePackage, ThemeStoreError> {
        self.state
            .packages
            .iter()
            .filter(|package| package.manifest.id == theme_id)
            .max_by(|left, right| {
                left.development
                    .cmp(&right.development)
                    .then_with(|| compare_versions(&left.manifest.version, &right.manifest.version))
            })
            .ok_or_else(|| {
                ThemeStoreError::new(
                    "THEME_NOT_FOUND",
                    format!("Theme is not installed: {theme_id}"),
                )
            })
    }

    fn package_root(&self, package: &ThemePackage) -> PathBuf {
        if package.development {
            PathBuf::from(&package.location)
        } else {
            self.root.join(&package.location)
        }
    }

    fn save(&self) -> Result<(), ThemeStoreError> {
        let content = serde_json::to_vec_pretty(&self.state).map_err(|error| {
            ThemeStoreError::new(
                "THEME_STORE_INVALID",
                format!("Cannot serialize Theme registry: {error}"),
            )
        })?;
        let temporary = self.root.join("registry.json.tmp");
        let registry = self.root.join(REGISTRY_FILE);
        let backup = self.root.join("registry.json.bak");
        fs::write(&temporary, content).map_err(store_io)?;
        if registry.exists() {
            if backup.exists() {
                fs::remove_file(&backup).map_err(store_io)?;
            }
            fs::rename(&registry, &backup).map_err(store_io)?;
        }
        if let Err(error) = fs::rename(&temporary, &registry) {
            if backup.exists() {
                let _ = fs::rename(&backup, &registry);
            }
            return Err(store_io(error));
        }
        if backup.exists() {
            fs::remove_file(backup).map_err(store_io)?;
        }
        Ok(())
    }
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    match (Version::parse(left), Version::parse(right)) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

fn relative_location(root: &Path, target: &Path) -> Result<String, ThemeStoreError> {
    target
        .strip_prefix(root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .map_err(|_| ThemeStoreError::new("THEME_STORE_IO", "Theme path leaves the store."))
}

fn verify_dev_entry(root: &Path, path: &str) -> Result<(), ThemeStoreError> {
    let relative = validate_archive_entry(path).map_err(|_| {
        ThemeStoreError::new("THEME_PATH_INVALID", "Development entry path is not safe.")
    })?;
    let entry = fs::canonicalize(root.join(relative)).map_err(store_io)?;
    if !entry.starts_with(root) || !entry.is_file() {
        return Err(ThemeStoreError::new(
            "THEME_PATH_INVALID",
            "Development entry leaves the Theme directory.",
        ));
    }
    Ok(())
}

fn extract_archive<R: Read + Seek>(reader: R, target: &Path) -> Result<(), ThemeStoreError> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|error| {
        ThemeStoreError::new(
            "THEME_ARCHIVE_INVALID",
            format!("Cannot open Theme archive: {error}"),
        )
    })?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            ThemeStoreError::new(
                "THEME_ARCHIVE_INVALID",
                format!("Cannot read Theme archive entry: {error}"),
            )
        })?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(ThemeStoreError::new(
                "THEME_ARCHIVE_PATH_INVALID",
                "Theme archives cannot contain symbolic links.",
            ));
        }
        let relative = validate_archive_entry(entry.name())?;
        let output = target.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(output).map_err(store_io)?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(store_io)?;
        }
        let mut file = fs::File::create(output).map_err(store_io)?;
        std::io::copy(&mut entry, &mut file).map_err(store_io)?;
    }
    Ok(())
}

fn store_io(error: std::io::Error) -> ThemeStoreError {
    ThemeStoreError::new("THEME_STORE_IO", format!("Theme store I/O failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;
    use zip::write::FileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn sha256(value: &[u8]) -> String {
        format!("sha256-{}", hex::encode(Sha256::digest(value)))
    }

    fn manifest(version: &str, script: &[u8], integrity: bool) -> Value {
        let mut value = json!({
            "schemaVersion": "1.0",
            "id": "com.example.nebula",
            "name": "Nebula",
            "version": version,
            "author": { "name": "Example" },
            "engines": {
                "rtlauncher": ">=0.2.0 <2.0.0",
                "themeApi": "^1.0.0"
            },
            "entry": { "script": "dist/theme.js", "style": "dist/theme.css" },
            "supports": { "colorSchemes": ["light", "dark"] }
        });
        if integrity {
            value["integrity"] = json!({
                "algorithm": "sha256",
                "files": {
                    "dist/theme.js": sha256(script),
                    "dist/theme.css": sha256(b"body{}")
                }
            });
        }
        value
    }

    fn write_archive(directory: &Path, version: &str, script: &[u8]) -> PathBuf {
        let archive_path = directory.join(format!("nebula-{version}.rtltheme"));
        let file = fs::File::create(&archive_path).expect("create archive");
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, content) in [
            (
                "manifest.json",
                serde_json::to_vec(&manifest(version, script, true)).expect("serialize manifest"),
            ),
            ("dist/theme.js", script.to_vec()),
            ("dist/theme.css", b"body{}".to_vec()),
        ] {
            writer
                .start_file(name, options)
                .expect("start archive file");
            writer.write_all(&content).expect("write archive file");
        }
        writer.finish().expect("finish archive");
        archive_path
    }

    fn write_dev_theme(directory: &Path) -> PathBuf {
        let theme_dir = directory.join("nebula-dev");
        fs::create_dir_all(theme_dir.join("dist")).expect("create development Theme");
        fs::write(
            theme_dir.join("manifest.json"),
            serde_json::to_vec(&manifest("2.0.0-dev.1", b"dev", false))
                .expect("serialize development manifest"),
        )
        .expect("write development manifest");
        fs::write(theme_dir.join("dist/theme.js"), b"dev").expect("write development script");
        fs::write(theme_dir.join("dist/theme.css"), b"body{}").expect("write development style");
        theme_dir
    }

    #[test]
    fn archive_install_uses_staging_and_keeps_versions() {
        let directory = TempDir::new().expect("create temporary directory");
        let first = write_archive(directory.path(), "1.0.0", b"one");
        let second = write_archive(directory.path(), "1.1.0", b"two");
        let mut store = ThemeStore::open(directory.path().join("store")).unwrap();

        let first_package = store.install_archive(&first).unwrap();
        let second_package = store.install_archive(&second).unwrap();

        assert_eq!(first_package.manifest.version, "1.0.0");
        assert_eq!(second_package.manifest.version, "1.1.0");
        assert!(store
            .root()
            .join("packages/com.example.nebula/1.0.0")
            .is_dir());
        assert!(store
            .root()
            .join("packages/com.example.nebula/1.1.0")
            .is_dir());
        assert_eq!(
            fs::read_dir(store.root().join(".staging")).unwrap().count(),
            0
        );
        assert_eq!(store.list().unwrap().len(), 2);
    }

    #[test]
    fn development_directory_is_registered_without_integrity_hashes() {
        let directory = TempDir::new().expect("create temporary directory");
        let dev_theme = write_dev_theme(directory.path());
        let mut store = ThemeStore::open(directory.path().join("store")).unwrap();

        let package = store.register_dev_directory(&dev_theme).unwrap();

        assert!(package.development);
        assert_eq!(package.manifest.version, "2.0.0-dev.1");
        assert_eq!(
            store
                .read_text("com.example.nebula", "dist/theme.js")
                .unwrap(),
            "dev"
        );
    }

    #[test]
    fn package_reads_cannot_leave_the_theme_directory() {
        let directory = TempDir::new().expect("create temporary directory");
        let archive = write_archive(directory.path(), "1.0.0", b"safe");
        let mut store = ThemeStore::open(directory.path().join("store")).unwrap();
        store.install_archive(&archive).unwrap();

        assert_eq!(
            store
                .read_binary("com.example.nebula", "dist/theme.js")
                .unwrap(),
            b"safe"
        );
        assert_eq!(
            store
                .read_text("com.example.nebula", "../registry.json")
                .unwrap_err()
                .code,
            "THEME_PATH_INVALID"
        );
    }

    #[test]
    fn built_in_theme_cannot_be_removed() {
        let directory = TempDir::new().expect("create temporary directory");
        let mut store = ThemeStore::open(directory.path().join("store")).unwrap();

        assert_eq!(
            store.remove("builtin.default", None).unwrap_err().code,
            "THEME_BUILTIN_PROTECTED"
        );
    }

    #[test]
    fn unconfirmed_activation_recovers_to_the_last_healthy_theme() {
        let directory = TempDir::new().expect("create temporary directory");
        let archive = write_archive(directory.path(), "1.0.0", b"safe");
        let store_path = directory.path().join("store");
        {
            let mut store = ThemeStore::open(&store_path).unwrap();
            store.install_archive(&archive).unwrap();
            store.set_active("com.example.nebula").unwrap();
            assert_eq!(store.state().active_theme_id, "com.example.nebula");
            assert_eq!(
                store.state().pending_theme_id.as_deref(),
                Some("com.example.nebula")
            );
        }

        let recovered = ThemeStore::open(&store_path).unwrap();
        assert_eq!(recovered.state().active_theme_id, "builtin.default");
        assert!(recovered.state().pending_theme_id.is_none());
    }

    #[test]
    fn healthy_activation_becomes_the_recovery_target() {
        let directory = TempDir::new().expect("create temporary directory");
        let archive = write_archive(directory.path(), "1.0.0", b"safe");
        let store_path = directory.path().join("store");
        {
            let mut store = ThemeStore::open(&store_path).unwrap();
            store.install_archive(&archive).unwrap();
            store.set_active("com.example.nebula").unwrap();
            store.mark_healthy("com.example.nebula").unwrap();
        }

        let reopened = ThemeStore::open(&store_path).unwrap();
        assert_eq!(reopened.state().active_theme_id, "com.example.nebula");
        assert_eq!(reopened.state().last_healthy_theme_id, "com.example.nebula");
    }

    #[test]
    fn trust_is_stored_for_one_installed_theme_version() {
        let directory = TempDir::new().expect("create temporary directory");
        let archive = write_archive(directory.path(), "1.0.0", b"safe");
        let store_path = directory.path().join("store");
        {
            let mut store = ThemeStore::open(&store_path).unwrap();
            store.install_archive(&archive).unwrap();
            assert!(!store.is_trusted("com.example.nebula", "1.0.0"));
            store
                .set_trusted("com.example.nebula", "1.0.0", true)
                .unwrap();
            assert!(store.is_trusted("com.example.nebula", "1.0.0"));
            assert!(!store.is_trusted("com.example.nebula", "1.1.0"));
        }

        let mut reopened = ThemeStore::open(&store_path).unwrap();
        assert!(reopened.is_trusted("com.example.nebula", "1.0.0"));
        reopened
            .set_trusted("com.example.nebula", "1.0.0", false)
            .unwrap();
        assert!(!reopened.is_trusted("com.example.nebula", "1.0.0"));
    }

    #[test]
    fn trust_is_kept_for_each_installed_theme_version() {
        let directory = TempDir::new().expect("create temporary directory");
        let first = write_archive(directory.path(), "1.0.0", b"one");
        let second = write_archive(directory.path(), "1.1.0", b"two");
        let store_path = directory.path().join("store");
        let mut store = ThemeStore::open(&store_path).unwrap();
        store.install_archive(&first).unwrap();
        store.install_archive(&second).unwrap();

        store
            .set_trusted("com.example.nebula", "1.0.0", true)
            .unwrap();
        store
            .set_trusted("com.example.nebula", "1.1.0", true)
            .unwrap();

        drop(store);
        let mut store = ThemeStore::open(&store_path).unwrap();

        assert!(store.is_trusted("com.example.nebula", "1.0.0"));
        assert!(store.is_trusted("com.example.nebula", "1.1.0"));

        store.remove("com.example.nebula", Some("1.0.0")).unwrap();

        assert!(!store.is_trusted("com.example.nebula", "1.0.0"));
        assert!(store.is_trusted("com.example.nebula", "1.1.0"));
    }

    #[test]
    fn legacy_trust_record_is_migrated_to_a_version_set() {
        let directory = TempDir::new().expect("create temporary directory");
        let store_path = directory.path().join("store");
        fs::create_dir_all(&store_path).expect("create Theme store directory");
        fs::write(
            store_path.join(REGISTRY_FILE),
            serde_json::to_vec(&json!({
                "activeThemeId": "builtin.default",
                "lastHealthyThemeId": "builtin.default",
                "pendingThemeId": null,
                "packages": [],
                "trustedPackages": {
                    "com.example.nebula": "1.0.0"
                }
            }))
            .expect("serialize legacy registry"),
        )
        .expect("write legacy registry");

        let store = ThemeStore::open(&store_path).expect("open legacy Theme store");

        assert!(store.is_trusted("com.example.nebula", "1.0.0"));
        let saved: Value = serde_json::from_slice(
            &fs::read(store_path.join(REGISTRY_FILE)).expect("read migrated registry"),
        )
        .expect("parse migrated registry");
        assert_eq!(
            saved["trustedPackages"]["com.example.nebula"],
            json!(["1.0.0"])
        );
    }

    #[test]
    fn removing_a_theme_removes_its_trust_record() {
        let directory = TempDir::new().expect("create temporary directory");
        let archive = write_archive(directory.path(), "1.0.0", b"safe");
        let mut store = ThemeStore::open(directory.path().join("store")).unwrap();
        store.install_archive(&archive).unwrap();
        store
            .set_trusted("com.example.nebula", "1.0.0", true)
            .unwrap();

        store.remove("com.example.nebula", None).unwrap();
        store.install_archive(&archive).unwrap();

        assert!(!store.is_trusted("com.example.nebula", "1.0.0"));
    }
}
