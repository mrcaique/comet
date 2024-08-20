use async_zip::base::read::mem::ZipFileReader;
use derive_getters::Getters;
use futures_util::io;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf};
use tokio::fs;
use tokio_util::compat::TokioAsyncWriteCompatExt;

use reqwest::Client;

#[allow(dead_code)]
pub enum Platform {
    Windows,
    Mac,
}

impl Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Windows => f.write_str("windows"),
            Self::Mac => f.write_str("osx"),
        }
    }
}

#[derive(Serialize, Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
struct ComponentManifest {
    application_type: String,
    #[serde(rename = "baseURI")]
    base_uri: String,
    files: Vec<ComponentFile>,
    force_update: bool,
    project_name: String,
    symlinks: Vec<ComponentSymlink>,
    timestamp: String,
    version: String,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
struct ComponentFile {
    hash: String,
    path: String,
    resource: String,
    sha256: String,
    size: u32,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
struct ComponentSymlink {
    path: String,
    target: String,
}

#[derive(Debug)]
pub enum Component {
    Peer,
    Overlay,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct ComponentManifestLocal {
    pub time: i64,
    pub version: String,
}

impl Display for Component {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Peer => f.write_str("desktop-galaxy-peer"),
            Self::Overlay => f.write_str("desktop-galaxy-overlay"),
        }
    }
}

pub async fn get_component(
    reqwest_client: &Client,
    dest_path: PathBuf,
    platform: Platform,
    component: Component,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manifest_path = dest_path.join(format!(".{}-{}.toml", component, platform));
    let local_manifest: ComponentManifestLocal = match fs::read_to_string(&manifest_path).await {
        Ok(manifest_str) => toml::from_str(&manifest_str).unwrap_or_default(),
        Err(err) => {
            log::debug!("Failed to read component manifest {err:?}");
            ComponentManifestLocal::default()
        }
    };
    if local_manifest.time + (24 * 3600) > chrono::Utc::now().timestamp() {
        return Ok(());
    }
    log::debug!("Checking for peer updates");
    let url = format!(
        "https://cfg.gog.com/{}/7/master/files-{}.json",
        component, platform
    );

    let manifest_res = reqwest_client.get(url).send().await?;
    let manifest: ComponentManifest = manifest_res.json().await?;

    if dest_path.exists() {
        if local_manifest.version == manifest.version && !manifest.force_update {
            return Ok(());
        }
    } else {
        fs::create_dir_all(&dest_path).await?;
    }

    // Download
    let n_of_files = manifest.files().len();
    for (i, file) in manifest.files().iter().enumerate() {
        log::info!("Downloading {} file {} of {}", component, i + 1, n_of_files);
        let url = format!("{}/{}", manifest.base_uri(), file.resource());
        let response = reqwest_client.get(url).send().await?;
        let data = response.bytes().await?;

        let zip = ZipFileReader::new(data.to_vec()).await?;

        let file_path = dest_path.join(file.path());
        let parent = file_path.parent().unwrap();
        if !parent.exists() {
            fs::create_dir_all(parent).await?;
        }

        let mut reader = zip.reader_with_entry(0).await?;
        let file_handle = fs::File::create(&file_path).await?;
        io::copy(&mut reader, &mut file_handle.compat_write()).await?;
        #[cfg(unix)]
        if let Some(permissions) = reader.entry().unix_permissions() {
            use std::{fs::Permissions, os::unix::fs::PermissionsExt};
            let permissions = Permissions::from_mode(permissions as u32);
            fs::set_permissions(file_path, permissions).await?;
        }
    }

    #[cfg(unix)]
    for symlink in manifest.symlinks() {
        fs::symlink(symlink.target(), dest_path.join(symlink.path())).await?;
    }

    let new_manifest = ComponentManifestLocal {
        version: manifest.version().clone(),
        time: chrono::Utc::now().timestamp(),
    };
    let data = toml::to_string(&new_manifest).expect("Failed to serialize local manifest");
    fs::write(manifest_path, data).await?;

    Ok(())
}
