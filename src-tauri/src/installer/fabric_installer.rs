use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json;
use std::fs;
use std::path::PathBuf;
use super::common_libraries::{LibraryItem, download_file_with_progress, download_libraries_concurrent};

#[derive(Debug, Deserialize)]
struct GameVersion {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct LoaderVersion {
    separator: String,
    build: u64,
    maven: String,
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricMetaResponse {
    game: Vec<GameVersion>,
    loader: Vec<LoaderVersion>,
}

/// 获取 Fabric Loader 版本列表
pub fn get_fabric_loader_versions(mc_version: &str, use_mirror: bool) -> Result<Vec<String>> {
    let url = if use_mirror {
        "https://bmclapi2.bangbang93.com/fabric-meta/v2/versions"
    } else {
        "https://meta.fabricmc.net/v2/versions"
    };
    let client = Client::new();
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let meta: FabricMetaResponse = resp.json()?;
    if !meta.game.iter().any(|g| g.version == mc_version) {
        return Err(anyhow!("MC版本 {} 不存在", mc_version));
    }
    let loader_versions = meta.loader.into_iter().map(|l| l.version).collect();
    Ok(loader_versions)
}

/// 获取 Fabric API 版本列表
pub fn get_fabric_api_versions(mc_version: &str) -> Result<Vec<String>> {
    use quick_xml::Reader;
    use quick_xml::events::Event;
    use quick_xml::name::QName;

    let url = "https://maven.fabricmc.net/net/fabricmc/fabric-api/fabric-api/maven-metadata.xml";
    let client = Client::new();
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let xml_text = resp.text()?;
    let mut reader = Reader::from_str(&xml_text);
    reader.config_mut().trim_text_end = true;
    reader.config_mut().trim_text_start = true;
    let mut versions = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) if e.name() == QName(b"version") => {
                let text = reader.read_text(QName(b"version"))?;
                if text.contains(mc_version) {
                    versions.push(text.into_owned());
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => continue,
            Err(e) => return Err(anyhow!("XML解析错误: {}", e)),
        }
    }
    Ok(versions)
}

/// 安装 Fabric Loader
pub fn install_fabric_loader(mc_version: &str, loader_version: &str, mc_folder_path: &str) -> Result<()> {
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
        mc_version, loader_version
    );
    let client = Client::new();
    let resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("请求失败, HTTP 状态码: {}", resp.status()));
    }
    let profile_json_text = resp.text()?;

    let version_id = format!("{}-{}-fabric", mc_version, loader_version);
    let versions_dir = PathBuf::from(mc_folder_path).join("versions").join(&version_id);
    fs::create_dir_all(&versions_dir)?;
    let profile_json_path = versions_dir.join(format!("{}.json", version_id));
    fs::write(&profile_json_path, &profile_json_text)?;

    #[derive(Debug, Deserialize)]
    struct ProfileJson {
        #[serde(default)]
        libraries: Vec<LibraryItem>,
    }
    let profile: ProfileJson = serde_json::from_str(&profile_json_text)?;

    download_libraries_concurrent(
        &profile.libraries,
        "https://maven.fabricmc.net",
        mc_folder_path,
        8,
    )?;

    println!("Fabric Loader安装完成！版本ID：{}", version_id);
    Ok(())
}

/// 安装 Fabric API
pub fn install_fabric_api(mc_version: &str, fabric_api_version: &str, mc_folder_path: &str) -> Result<()> {
    let fabric_api_url = format!(
        "https://maven.fabricmc.net/net/fabricmc/fabric-api/fabric-api/{0}/fabric-api-{0}.jar",
        fabric_api_version
    );
    let mods_dir = PathBuf::from(mc_folder_path).join("instance").join(mc_version).join("mods");
    fs::create_dir_all(&mods_dir)?;
    let fabric_api_jar = mods_dir.join(format!("fabric-api-{}.jar", fabric_api_version));

    download_file_with_progress(&fabric_api_url, &fabric_api_jar, 24)?;

    println!("Fabric API安装完成！版本：{}", fabric_api_version);
    Ok(())
}
