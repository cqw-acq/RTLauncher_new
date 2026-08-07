/*
RTLauncher, a third-party Minecraft launcher built with the newest
technology and provides innovative funtionalities
Copyright (C) 2025 lutouna

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use log::{error, info};
use sha2::{Digest, Sha256};
use std::fs;
use std::fs::File;
use std::io::Write;

const FOLDER_PATH: &str = "./.minecraft/versions";

/// 获取或下载 authlib-injector，返回其文件路径
/// 如果下载失败则返回空字符串
pub fn get_or_download_authlib_injector() -> String {
    // 初始化
    let URL_BMCL = "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json";
    let URL_YUSHI = "https://authlib-injector.yushi.moe/artifact/latest.json";
    let httpClient = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());

    // 先检查现有版本目录中是否已有 authlib-injector jar
    if let Ok(entries) = fs::read_dir(FOLDER_PATH) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if name.starts_with("authlib-injector") && name.ends_with(".jar") {
                    let path = format!("{}/{}", FOLDER_PATH, name);
                    eprintln!("[AuthlibInjector] 找到现有文件: {}", path);
                    return path;
                }
            }
        }
    }

    // 尝试bmcl源
    let mut jsonResponse = httpClient.get(URL_BMCL).send();
    let jsonData;
    
    // 尝试yushi源
    if jsonResponse.is_err() {
        eprintln!(
            "[AuthlibInjector] BMCL源失败，尝试Yushi源: {}",
            jsonResponse.err().unwrap()
        );
        jsonResponse = httpClient.get(URL_YUSHI).send();
        if jsonResponse.is_err() {
            error!("两个 authlib-injector 下载源都连接失败了");
            return String::new();
        }
    }
    
    // 解析JSON
    let jsonText = jsonResponse.unwrap().text();
    if jsonText.is_err() {
        error!("获取 authlib-injector JSON失败");
        return String::new();
    }
    
    let parse_result: Result<serde_json::Value, _> = serde_json::from_str(&jsonText.unwrap());
    match parse_result {
        Ok(data) => jsonData = data,
        Err(e) => {
            error!("authlib-injector JSON格式错误: {}", e);
            return String::new();
        }
    }
    
    // 获取下载地址
    let downloadUrl = jsonData.get("download_url").and_then(|u| u.as_str());
    if downloadUrl.is_none() {
        error!("JSON中没有下载地址");
        return String::new();
    }
    let downloadUrl = downloadUrl.unwrap();
    
    // 获取文件名
    let urlParts: Vec<&str> = downloadUrl.split('/').collect();
    let fileName = urlParts.last().unwrap_or(&"authlib-injector.jar");
    
    // 构造文件路径
    let filePath = format!("{}/{}", FOLDER_PATH, fileName);
    
    // 检查文件是否已存在
    if fs::metadata(&filePath).is_ok() {
        if let Ok(fileContent) = fs::read(&filePath) {
            let fileSha256 = hex::encode(Sha256::digest(&fileContent));
        
            // 获取校验和
            if let Some(checksumValue) = jsonData
                .get("checksums")
                .and_then(|c| c.get("sha256"))
                .and_then(|s| s.as_str()) 
            {
                if fileSha256 == checksumValue {
                    info!("[AuthlibInjector] 文件已存在且校验成功: {}", filePath);
                    return filePath;
                }
            }
        }
    }
    
    // 下载文件
    eprintln!("[AuthlibInjector] 开始下载: {}", downloadUrl);
    let downloadResponse = httpClient.get(downloadUrl).send();
    if downloadResponse.is_err() {
        error!(
            "[AuthlibInjector] 下载文件失败: {}",
            downloadResponse.err().unwrap()
        );
        return String::new();
    }
    
    let fileContent = downloadResponse.unwrap().bytes();
    if fileContent.is_err() {
        error!("[AuthlibInjector] 读取下载内容失败");
        return String::new();
    }
    let bytes = fileContent.unwrap();
    eprintln!("[AuthlibInjector] 下载完成，大小: {} bytes", bytes.len());

    // 创建目录
    if let Err(err) = fs::create_dir_all(FOLDER_PATH) {
        error!("创建目录失败: {}", err);
        return String::new();
    }

    // 保存文件
    if let Err(err) = fs::write(&filePath, &bytes) {
        error!("保存文件失败: {}", err);
        return String::new();
    }
    
    // 验证文件
    if let Some(checksumValue) = jsonData
        .get("checksums")
        .and_then(|c| c.get("sha256"))
        .and_then(|s| s.as_str())
    {
        let fileSha256 = hex::encode(Sha256::digest(&bytes));
        if fileSha256 == checksumValue {
            info!("[AuthlibInjector] 文件下载成功，校验成功: {}", filePath);
        } else {
            error!("[AuthlibInjector] 校验失败，但仍返回路径供尝试使用");
        }
    }
    
    filePath
}

pub fn downloadInjecter() {
    let _ = get_or_download_authlib_injector();
}

#[tauri::command]
pub fn thirdPartyLogin(url: String) -> String {
    // 发送get请求
    let response = reqwest::blocking::get(url);
    if response.is_err() {
        error!("无法发送get请求: {}", response.err().unwrap());
        return String::new();
    }

    // 获取响应体
    let body = response.unwrap().text();
    if body.is_err() {
        error!("无法获取响应体: {}", body.err().unwrap());
        return String::new();
    }
    let bodyText = body.unwrap();

    // 查找返回的json中是否有signaturePublickey
    let jsonResult = serde_json::from_str::<serde_json::Value>(&bodyText);
    if jsonResult.is_err() {
        error!(
            "JSON解析失败: {}，收到的内容: {}",
            jsonResult.err().unwrap(),
            bodyText
        );
        return String::new();
    }
    
    let json = jsonResult.unwrap();
    let signaturePublicKey = json.get("signaturePublickey");
    if signaturePublicKey.is_none() {
        error!("返回的json中没有signaturePublickey");
        return String::new();
    }

    // 返回base64编码的整个响应的json
    let base64Json = BASE64.encode(&bodyText);
    info!("{}", base64Json);
    base64Json
}

// 返回一个字符串和两个字符串数组
#[tauri::command]
pub fn getAccountList(
    url: String,
    user: String,
    pwd: String,
) -> Result<super::ThirdPartyAccountList, String> {
    // 初始化
    let fullUrl = format!("{}/{}", url, "authserver/authenticate");
    let client = reqwest::blocking::Client::new();

    // 发送post请求
    let requestBody = format!(
        r#"{{"username":"{}","password":"{}","clientToken":"","requestUser":true,"agent":{{"name":"Minecraft","version":1}}}}"#,
        user, pwd
    );
    
    let response = client
        .post(fullUrl)
        .header("Content-Type", "application/json")
        .body(requestBody)
        .send()
        .map_err(|e| format!("发送POST请求失败: {}", e))?;

    let bodyText = response
        .text()
        .map_err(|e| format!("获取响应体失败: {}", e))?;

    let json: serde_json::Value =
        serde_json::from_str(&bodyText).map_err(|e| format!("JSON解析失败: {}", e))?;

    let accessToken = json
        .get("accessToken")
        .and_then(|v| v.as_str())
        .ok_or("JSON中没有accessToken".to_string())?
        .to_string();

    let availableProfiles = json
        .get("availableProfiles")
        .and_then(|v| v.as_array())
        .ok_or("JSON中没有availableProfiles".to_string())?;

    let mut profiles = Vec::new();
    for profile in availableProfiles {
        if let (Some(id), Some(name)) = (
            profile.get("id").and_then(|v| v.as_str()),
            profile.get("name").and_then(|v| v.as_str()),
        ) {
            profiles.push(super::ThirdPartyProfile {
                id: id.to_string(),
                name: name.to_string(),
            });
        }
    }

    info!("accessToken: {}", accessToken);
    info!("profiles: {:?}", profiles);

    Ok(super::ThirdPartyAccountList {
        access_token: accessToken,
        profiles,
    })
}

#[tauri::command]
pub fn getPlayerSkin(url: String, uuid: String) -> String {
    info!("开始获取玩家皮肤...");
    
    // 去掉可能存在的引号
    let uuid = uuid.trim_matches('"');
    
    // 初始化
    let fullUrl = format!("{}/sessionserver/session/minecraft/profile/{}", url, uuid);
    info!("请求URL: {}", fullUrl);
    let httpClient = reqwest::blocking::Client::new();

    // 发送get请求
    let response = httpClient.get(fullUrl).send();
    if response.is_err() {
        error!("发送get请求失败: {}", response.err().unwrap());
        return String::new();
    }
    
    // 获取响应体
    let body = response.unwrap().text();
    if body.is_err() {
        error!("获取响应体失败: {}", body.err().unwrap());
        return String::new();
    }
    let bodyText = body.unwrap();

    // 解析JSON
    let jsonResult = serde_json::from_str::<serde_json::Value>(&bodyText);
    if jsonResult.is_err() {
        error!("JSON解析失败: {}", jsonResult.err().unwrap());
        return String::new();
    }
    let json = jsonResult.unwrap();

    // 获取 properties 中 value 的值
    let properties = json.get("properties");
    if properties.is_none() {
        error!("JSON中没有properties");
        return String::new();
    }
    let properties = properties.unwrap().as_array().unwrap();
    let value = properties
        .get(0)
        .unwrap()
        .get("value")
        .unwrap()
        .as_str()
        .unwrap();
    info!("获取到的base64编码值: {}", value);

    // base64解码
    let decoded = BASE64.decode(value);
    if decoded.is_err() {
        error!("base64解码失败: {}", decoded.err().unwrap());
        return String::new();
    }

    // 解析解码后的json
    let jsonResult = serde_json::from_slice(&decoded.unwrap());
    if jsonResult.is_err() {
        error!("JSON解析失败: {}", jsonResult.err().unwrap());
        return String::new();
    }
    let json: serde_json::Value = jsonResult.unwrap();

    // 获取textures中的SKIN中的url
    let textures = json.get("textures");
    if textures.is_none() {
        error!("JSON中没有textures");
        return String::new();
    }
    let textures = textures.unwrap().as_object().unwrap();
    let skin = textures
        .get("SKIN")
        .unwrap()
        .get("url")
        .unwrap()
        .as_str()
        .unwrap();
    
    info!("获取到的皮肤URL: {}", skin);
    
    // 下载皮肤
    let response = httpClient.get(skin).send();
    if response.is_err() {
        error!("下载皮肤失败: {}", response.err().unwrap());
        return String::new();
    }

    // 创建 skins 目录
    let skins_dir = format!("{}/skins", super::config_dir());
    if let Err(err) = fs::create_dir_all(&skins_dir) {
        error!("创建皮肤目录失败: {}", err);
        return String::new();
    }

    // 保存皮肤
    let skinPath = format!("{}/{}.png", skins_dir, uuid);
    let mut file = match File::create(&skinPath) {
        Ok(file) => file,
        Err(err) => {
            error!("创建文件失败: {}", err);
            return String::new();
        }
    };
    file.write_all(&response.unwrap().bytes().unwrap()).unwrap();
    file.flush().unwrap();

    // 返回皮肤路径
    info!("皮肤路径: {}", skinPath);
    skinPath
}
