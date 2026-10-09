pub mod view;
pub use view::PackageManagerView;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tar::Archive;
use typst_kit::packages::FsPackages;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageIndexItem {
    pub name: String,
    pub version: String,
    pub entrypoint: String,
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub compiler: Option<String>,
    #[serde(default)]
    pub updated_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageLocationStatus {
    InstalledLocal, // Package resides in the `@local` data directory
    CachedPreview,  // Package resides in the `@preview` cache directory
    NotDownloaded,  // Found in remote registry but missing on disk
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPackageInfo {
    pub namespace: String, // "preview" or "local"
    pub name: String,
    pub version: String,
    pub path: PathBuf,
}

pub struct PackageManagerStore {
    pub cache_packages: Option<FsPackages>,
    pub data_packages: Option<FsPackages>,
    pub cache_dir: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub local_packages: Vec<LocalPackageInfo>,
    pub remote_index: Option<Vec<PackageIndexItem>>,
    pub is_fetching: bool,
    pub last_error: Option<String>,
}

impl Default for PackageManagerStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManagerStore {
    pub fn new() -> Self {
        let cache_packages = FsPackages::system_cache();
        let data_packages = FsPackages::system_data();
        let cache_dir = cache_packages.as_ref().map(|p| p.path().to_path_buf());
        let data_dir = data_packages.as_ref().map(|p| p.path().to_path_buf());

        let mut store = Self {
            cache_packages,
            data_packages,
            cache_dir,
            data_dir,
            local_packages: Vec::new(),
            remote_index: None,
            is_fetching: false,
            last_error: None,
        };

        store.scan_local_packages();
        store
    }

    /// Explicit local disk scanner inspecting local cache and data paths
    pub fn scan_local_packages(&mut self) {
        let mut found = Vec::new();

        let scan_targets = [(&self.cache_dir, "preview"), (&self.data_dir, "local")];

        for (dir_opt, default_ns) in scan_targets {
            if let Some(dir_path) = dir_opt {
                if !dir_path.exists() {
                    continue;
                }

                if let Ok(ns_entries) = fs::read_dir(dir_path) {
                    for ns_entry in ns_entries.flatten() {
                        if ns_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            let ns_path = ns_entry.path();
                            let namespace = ns_entry.file_name().to_string_lossy().to_string();

                            if let Ok(pkg_entries) = fs::read_dir(&ns_path) {
                                for pkg_entry in pkg_entries.flatten() {
                                    if pkg_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                        let pkg_path = pkg_entry.path();
                                        let name =
                                            pkg_entry.file_name().to_string_lossy().to_string();

                                        if let Ok(ver_entries) = fs::read_dir(&pkg_path) {
                                            for ver_entry in ver_entries.flatten() {
                                                if ver_entry
                                                    .file_type()
                                                    .map(|t| t.is_dir())
                                                    .unwrap_or(false)
                                                {
                                                    let ver_path = ver_entry.path();
                                                    let version = ver_entry
                                                        .file_name()
                                                        .to_string_lossy()
                                                        .to_string();

                                                    let has_toml =
                                                        ver_path.join("typst.toml").exists();
                                                    if has_toml
                                                        || Self::directory_has_contents(&ver_path)
                                                    {
                                                        found.push(LocalPackageInfo {
                                                            namespace: namespace.clone(),
                                                            name: name.clone(),
                                                            version: version.clone(),
                                                            path: ver_path,
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        self.local_packages = found;
    }

    /// Opt-In Sync: Fetches registry JSON over network using internal `ureq` client
    pub fn fetch_remote_index(&mut self) -> Result<Vec<PackageIndexItem>, String> {
        self.is_fetching = true;
        self.last_error = None;

        let url = "https://packages.typst.org/preview/index.json";

        let mut response = ureq::get(url)
            .header("User-Agent", "typforge-package-manager/0.1.0")
            .call()
            .map_err(|e| {
                let err_msg = format!("Offline or connection error: {}", e);
                self.last_error = Some(err_msg.clone());
                self.is_fetching = false;
                err_msg
            })?;

        let index_items: Vec<PackageIndexItem> = response.body_mut().read_json().map_err(|e| {
            let err_msg = format!("Failed to parse index JSON: {}", e);
            self.last_error = Some(err_msg.clone());
            self.is_fetching = false;
            err_msg
        })?;

        // Deduplicate to latest version per package upon fetch
        let mut latest_map: std::collections::BTreeMap<String, PackageIndexItem> =
            std::collections::BTreeMap::new();
        for item in index_items {
            latest_map
                .entry(item.name.clone())
                .and_modify(|existing| {
                    if item.version > existing.version {
                        *existing = item.clone();
                    }
                })
                .or_insert(item);
        }

        let deduplicated: Vec<PackageIndexItem> = latest_map.into_values().collect();
        self.remote_index = Some(deduplicated.clone());
        self.is_fetching = false;
        Ok(deduplicated)
    }

    /// Evaluates the download status of a namespace / name / version configuration
    pub fn get_status(&self, namespace: &str, name: &str, version: &str) -> PackageLocationStatus {
        let is_local_installed = self
            .local_packages
            .iter()
            .any(|pkg| pkg.namespace == namespace && pkg.name == name && pkg.version == version);

        if is_local_installed {
            if namespace == "local" {
                PackageLocationStatus::InstalledLocal
            } else {
                PackageLocationStatus::CachedPreview
            }
        } else {
            PackageLocationStatus::NotDownloaded
        }
    }

    fn directory_has_contents(path: &Path) -> bool {
        fs::read_dir(path)
            .map(|mut entry| entry.next().is_some())
            .unwrap_or(false)
    }

    /// Downloads and extracts a specific preview package into the local cache directory
    pub fn download_package(&mut self, name: &str, version: &str) -> Result<PathBuf, String> {
        let cache_dir = self
            .cache_dir
            .as_ref()
            .ok_or_else(|| "No cache directory available on this system.".to_string())?;

        let url = format!(
            "https://packages.typst.org/preview/{}-{}.tar.gz",
            name, version
        );

        let response = ureq::get(&url)
            .header("User-Agent", "typforge-package-manager/0.1.0")
            .call()
            .map_err(|e| format!("Failed to download {}:{} - {}", name, version, e))?;

        let mut bytes = Vec::new();
        std::io::copy(&mut response.into_body().into_reader(), &mut bytes)
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        let tar_gz = Cursor::new(bytes);
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);

        let target_dir = cache_dir.join("preview").join(name).join(version);
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create package directory {:?}: {}", target_dir, e))?;

        archive
            .unpack(&target_dir)
            .map_err(|e| format!("Failed to unpack package archive: {}", e))?;

        // Re-scan local disk packages to pick up the newly downloaded package
        self.scan_local_packages();
        Ok(target_dir)
    }

    /// Removes a cached preview package version from disk and cleans up empty parent directories
    pub fn remove_cached_package(&mut self, name: &str, version: &str) -> Result<(), String> {
        let cache_dir = self
            .cache_dir
            .as_ref()
            .ok_or_else(|| "No cache directory available on this system.".to_string())?;

        let package_dir = cache_dir.join("preview").join(name);
        let version_dir = package_dir.join(version);

        // 1. Remove the specific version folder
        if version_dir.exists() {
            fs::remove_dir_all(&version_dir).map_err(|e| {
                format!(
                    "Failed to delete package version at {:?}: {}",
                    version_dir, e
                )
            })?;
        }

        // 2. Clean up empty parent package directory if no other versions remain
        if package_dir.exists() && Self::is_directory_empty(&package_dir) {
            let _ = fs::remove_dir(&package_dir);
        }

        // Re-scan disk
        self.scan_local_packages();
        Ok(())
    }

    fn is_directory_empty(path: &Path) -> bool {
        fs::read_dir(path)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
    }
}
